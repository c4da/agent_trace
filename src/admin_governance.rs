//! Admin Governance Plane & Shielded Client Layer (`src/admin_governance.rs`).
//!
//! Separates the Gateway into two distinct operational planes:
//! 1. **Admin Governance Plane (`/api/v1/admin/governance/...`)**:
//!    - Administrators configure **Global Restrictions** and **Per-Model Restriction Profiles**.
//!    - Each model (`claude-*`, `gpt-*`, `qwen3.8:27b`, `deepseek-r1:1.5b`, etc.) has its own
//!      independent security envelope: trust tier, enabled/disabled state, transparent fallback model,
//!      DLP enforcement mode (`HardBlock` vs `RedactAndForward`), shell execution permission,
//!      file-write permission, playground confinement, path whitelists/blacklists, and payload size caps.
//! 2. **Shielded Client Layer (`/v1/messages`, `/v1/chat/completions`, `/api/v1/client/shield-status`)**:
//!    - Developers simply run `claude` in their terminal (`ANTHROPIC_BASE_URL=http://127.0.0.1:8080 claude`).
//!    - Clients are completely shielded from policy internals: they cannot weaken Admin enforcement
//!      modes via HTTP headers (`ignore_client_mode_overrides`), disabled models transparently reroute
//!      to Admin-approved fallback models, secrets are scrubbed in-flight, and blocked actions return
//!      a clean, non-leaky terminal message while full forensics are recorded to the Admin ledger.

use crate::ai_guard::{GuardEnforcementMode, GuardFinding, ThreatCategory};
use crate::persistence::{global_persistence, UnifiedPersistenceManager};
use crate::security_hardening::constant_time_eq;
use axum::{
    extract::Path,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

static GLOBAL_ADMIN_GOVERNANCE: OnceLock<RwLock<AdminGovernanceEngine>> = OnceLock::new();

/// Returns a reference to the process-wide Admin Governance Engine.
pub fn global_admin_governance() -> &'static RwLock<AdminGovernanceEngine> {
    GLOBAL_ADMIN_GOVERNANCE.get_or_init(|| RwLock::new(AdminGovernanceEngine::load_or_default()))
}

/// Trust classification assigned by the Administrator to a model family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTrustTier {
    /// External cloud provider (Anthropic, OpenAI, Google) — data leaves the LAN.
    ExternalCloud,
    /// Local sovereign heavy reasoning model on Lead Node (PC-1 Ollama) — data stays on metal.
    LocalSovereignLead,
    /// Fast local worker/small model (e.g. 1.5B CUDA worker) — high speed, higher hallucination risk.
    LocalWorkerSandbox,
    /// Untrusted or experimental model — strictly locked down.
    RestrictedUntrusted,
}

/// Global restrictions set by the Administrator across the entire Gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAdminRestrictions {
    /// When true, client terminals (like Claude Code) receive clean, shielded responses
    /// without exposing internal rule regexes, vault paths, or SIEM configuration.
    pub shield_client_terminals: bool,
    /// When true, client headers like `X-Guard-Mode: audit` cannot weaken or bypass
    /// the Admin's configured `GuardEnforcementMode` for a model.
    pub ignore_client_mode_overrides: bool,
    /// Always block access to `.secrets`, `.ssh`, `.env`, and private keys across all models.
    pub enforce_global_vault_lock: bool,
    /// Shell command substrings globally forbidden across every model regardless of tier.
    pub global_blocked_commands: Vec<String>,
    /// Optional Admin API key requirement for mutating governance rules.
    pub require_admin_key: bool,
}

impl Default for GlobalAdminRestrictions {
    fn default() -> Self {
        Self {
            shield_client_terminals: true,
            ignore_client_mode_overrides: true,
            enforce_global_vault_lock: true,
            global_blocked_commands: vec![
                "rm -rf /".to_string(),
                "mkfs.".to_string(),
                "format c:".to_string(),
                "stop-computer".to_string(),
            ],
            require_admin_key: false,
        }
    }
}

/// Per-model restriction profile configured by the Administrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRestrictionProfile {
    /// Exact model ID (e.g. `"deepseek-r1:1.5b"`) or wildcard prefix (e.g. `"claude-*"`, `"gpt-*"`).
    pub model_pattern: String,
    pub display_name: String,
    pub trust_tier: ModelTrustTier,
    /// Whether clients are allowed to invoke this model.
    pub enabled: bool,
    /// If `enabled == false` (or if Admin redirects this model), transparently reroute the
    /// client's request to this model without breaking their terminal session.
    pub fallback_model: Option<String>,
    /// Authoritative AI Guard mode for this model (`HardBlock`, `RedactAndForward`, `AuditOnly`).
    pub enforcement_mode: GuardEnforcementMode,
    /// Whether outbound prompts must undergo strict cloud DLP scrubbing/blocking.
    pub strict_cloud_dlp: bool,
    /// Whether this model is permitted to execute shell/bash commands (`CommandRun` / `bash` tool).
    pub allow_shell_execution: bool,
    /// Whether this model is permitted to modify or create files on disk.
    pub allow_file_writes: bool,
    /// When true, any file writes by this model must target `playground/` only.
    pub confine_to_playground: bool,
    /// Allowed path prefixes for this model (empty = defer to global policy).
    pub allowed_path_prefixes: Vec<String>,
    /// Specific path substrings blocked for this model (e.g. `"proprietary/"`, `".secrets"`).
    pub blocked_path_patterns: Vec<String>,
    /// Maximum prompt character length allowed for this model (prevents bulk repo exfiltration).
    pub max_prompt_chars: usize,
    /// Optional Admin-enforced system instruction transparently prepended to requests for this model.
    pub admin_system_prompt_prefix: Option<String>,
}

/// Outcome of evaluating a client request against the Admin's Global + Per-Model Restrictions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelGovernanceDecision {
    pub requested_model: String,
    /// The effective model after applying any Admin fallback/rerouting rules.
    pub effective_model: String,
    pub transparently_rerouted: bool,
    pub matched_profile_pattern: String,
    pub trust_tier: ModelTrustTier,
    pub effective_enforcement_mode: GuardEnforcementMode,
    pub allowed: bool,
    pub findings: Vec<GuardFinding>,
    pub admin_reasons: Vec<String>,
    /// Clean, non-leaky message safe to display in a shielded client terminal (`claude` CLI).
    pub shielded_client_message: Option<String>,
    /// Optional system prompt prefix to inject transparently.
    pub injected_system_prefix: Option<String>,
}

/// The central Admin Governance Engine managing global and per-model restrictions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminGovernanceEngine {
    pub global: GlobalAdminRestrictions,
    pub model_profiles: HashMap<String, ModelRestrictionProfile>,
    pub default_profile: ModelRestrictionProfile,
    #[serde(skip)]
    pub persistence: Option<Arc<UnifiedPersistenceManager>>,
}

impl AdminGovernanceEngine {
    /// Loads persisted Admin Governance state from `global_persistence()`, or initializes defaults.
    pub fn load_or_default() -> Self {
        Self::new_with_persistence(global_persistence())
    }

    /// Initializes the Admin Governance Engine backed by a specific `UnifiedPersistenceManager`.
    pub fn new_with_persistence(pm: Arc<UnifiedPersistenceManager>) -> Self {
        if let Some(mut loaded) = pm.load_struct::<AdminGovernanceEngine>("admin_governance", "engine") {
            // Ensure default built-in profiles exist if not explicitly present
            let defaults = Self::new_with_defaults();
            for (k, v) in defaults.model_profiles {
                loaded.model_profiles.entry(k).or_insert(v);
            }
            loaded.persistence = Some(pm);
            return loaded;
        }
        let mut eng = Self::new_with_defaults();
        eng.persistence = Some(pm);
        eng.persist();
        eng
    }

    /// Reloads state from the configured `UnifiedPersistenceManager`.
    pub fn reload_from_persistence(&mut self) {
        let pm = self.persistence.clone().unwrap_or_else(global_persistence);
        if let Some(loaded) = pm.load_struct::<AdminGovernanceEngine>("admin_governance", "engine") {
            self.global = loaded.global;
            for (k, v) in loaded.model_profiles {
                self.model_profiles.insert(k, v);
            }
            self.default_profile = loaded.default_profile;
        }
    }

    /// Persists the current Admin Governance state to Local JSON, SQLite, and External DB.
    pub fn persist(&self) {
        if let Some(ref pm) = self.persistence {
            let _ = pm.save_struct("admin_governance", "engine", self);
        }
    }

    /// Initializes the Admin Governance Engine with sensible, production-ready per-model profiles.
    pub fn new_with_defaults() -> Self {
        let mut profiles = HashMap::new();

        // 1. Anthropic Claude Models (Claude Code Terminal default) -> External Cloud, RedactAndForward DLP so terminal never crashes on accidental secret paste, but HardBlock destructive commands & vault paths
        profiles.insert(
            "claude-*".to_string(),
            ModelRestrictionProfile {
                model_pattern: "claude-*".to_string(),
                display_name: "Anthropic Claude Family (Shielded Terminal)".to_string(),
                trust_tier: ModelTrustTier::ExternalCloud,
                enabled: true,
                fallback_model: Some("qwen3.8:27b".to_string()),
                enforcement_mode: GuardEnforcementMode::RedactAndForward,
                strict_cloud_dlp: true,
                allow_shell_execution: true,
                allow_file_writes: true,
                confine_to_playground: false,
                allowed_path_prefixes: vec![],
                blocked_path_patterns: vec![".secrets".to_string(), ".ssh".to_string(), ".env".to_string()],
                max_prompt_chars: 200_000,
                admin_system_prompt_prefix: Some(
                    "[Admin Governance: Never output credentials or modify files outside the workspace.]".to_string(),
                ),
            },
        );

        // 2. OpenAI Cloud Models (`gpt-*`) -> External Cloud, HardBlock DLP, No direct shell execution
        profiles.insert(
            "gpt-*".to_string(),
            ModelRestrictionProfile {
                model_pattern: "gpt-*".to_string(),
                display_name: "OpenAI Cloud Family (Code & Analysis Only)".to_string(),
                trust_tier: ModelTrustTier::ExternalCloud,
                enabled: true,
                fallback_model: Some("qwen3.8:27b".to_string()),
                enforcement_mode: GuardEnforcementMode::HardBlock,
                strict_cloud_dlp: true,
                allow_shell_execution: false,
                allow_file_writes: true,
                confine_to_playground: false,
                allowed_path_prefixes: vec![],
                blocked_path_patterns: vec![".secrets".to_string(), ".ssh".to_string(), ".env".to_string()],
                max_prompt_chars: 120_000,
                admin_system_prompt_prefix: None,
            },
        );

        // 3. Local Sovereign Heavy Reasoning (`qwen3.8:27b` on PC-1) -> Data stays on local metal, full workspace & shell access
        profiles.insert(
            "qwen3.8:27b".to_string(),
            ModelRestrictionProfile {
                model_pattern: "qwen3.8:27b".to_string(),
                display_name: "PC-1 Sovereign Lead Reasoning (Qwen 3.8 27B)".to_string(),
                trust_tier: ModelTrustTier::LocalSovereignLead,
                enabled: true,
                fallback_model: None,
                enforcement_mode: GuardEnforcementMode::HardBlock,
                strict_cloud_dlp: false,
                allow_shell_execution: true,
                allow_file_writes: true,
                confine_to_playground: false,
                allowed_path_prefixes: vec![],
                blocked_path_patterns: vec![".secrets".to_string(), ".ssh".to_string()],
                max_prompt_chars: 500_000,
                admin_system_prompt_prefix: None,
            },
        );

        // 4. Local Fast Code Model (`qwen3:14b` / `qwen3.5:9b` on PC-1)
        profiles.insert(
            "qwen3*".to_string(),
            ModelRestrictionProfile {
                model_pattern: "qwen3*".to_string(),
                display_name: "PC-1 Sovereign Fast Code (Qwen 3 Family)".to_string(),
                trust_tier: ModelTrustTier::LocalSovereignLead,
                enabled: true,
                fallback_model: None,
                enforcement_mode: GuardEnforcementMode::HardBlock,
                strict_cloud_dlp: false,
                allow_shell_execution: true,
                allow_file_writes: true,
                confine_to_playground: false,
                allowed_path_prefixes: vec![],
                blocked_path_patterns: vec![".secrets".to_string(), ".ssh".to_string()],
                max_prompt_chars: 300_000,
                admin_system_prompt_prefix: None,
            },
        );

        // 5. Fast Small Worker Model (`deepseek-r1:1.5b` on PC-2) -> Strictly sandboxed: NO shell execution, file writes confined to `playground/`!
        profiles.insert(
            "deepseek-r1:1.5b".to_string(),
            ModelRestrictionProfile {
                model_pattern: "deepseek-r1:1.5b".to_string(),
                display_name: "PC-2 Small CUDA Worker (1.5B Sandbox)".to_string(),
                trust_tier: ModelTrustTier::LocalWorkerSandbox,
                enabled: true,
                fallback_model: Some("qwen3.8:27b".to_string()),
                enforcement_mode: GuardEnforcementMode::HardBlock,
                strict_cloud_dlp: false,
                allow_shell_execution: false,
                allow_file_writes: true,
                confine_to_playground: true,
                allowed_path_prefixes: vec!["playground".to_string(), "/workspace/playground".to_string()],
                blocked_path_patterns: vec!["src/".to_string(), ".secrets".to_string(), ".env".to_string()],
                max_prompt_chars: 64_000,
                admin_system_prompt_prefix: None,
            },
        );

        let default_profile = ModelRestrictionProfile {
            model_pattern: "*".to_string(),
            display_name: "Default Unlisted Model Profile (Strict)".to_string(),
            trust_tier: ModelTrustTier::RestrictedUntrusted,
            enabled: true,
            fallback_model: Some("qwen3.8:27b".to_string()),
            enforcement_mode: GuardEnforcementMode::HardBlock,
            strict_cloud_dlp: true,
            allow_shell_execution: false,
            allow_file_writes: false,
            confine_to_playground: true,
            allowed_path_prefixes: vec!["playground".to_string()],
            blocked_path_patterns: vec![".secrets".to_string(), ".ssh".to_string(), ".env".to_string()],
            max_prompt_chars: 50_000,
            admin_system_prompt_prefix: None,
        };

        Self {
            global: GlobalAdminRestrictions::default(),
            model_profiles: profiles,
            default_profile,
            persistence: None,
        }
    }

    /// Resolves the most specific `ModelRestrictionProfile` matching `model_name`.
    /// Priority:
    /// 1. Exact match (e.g. `"qwen3.8:27b"` or `"deepseek-r1:1.5b"`)
    /// 2. Longest wildcard prefix match (e.g. `"claude-*"` matches `"claude-3-7-sonnet-latest"`)
    /// 3. `default_profile`
    pub fn resolve_profile(&self, model_name: &str) -> ModelRestrictionProfile {
        let lower = model_name.to_lowercase();
        if let Some(exact) = self.model_profiles.get(model_name).or_else(|| self.model_profiles.get(&lower)) {
            return exact.clone();
        }

        let mut best_match: Option<&ModelRestrictionProfile> = None;
        let mut best_prefix_len = 0usize;

        for (pattern, profile) in &self.model_profiles {
            if let Some(prefix) = pattern.strip_suffix('*') {
                let p_low = prefix.to_lowercase();
                if lower.starts_with(&p_low) && p_low.len() >= best_prefix_len {
                    best_prefix_len = p_low.len();
                    best_match = Some(profile);
                }
            }
        }

        best_match.cloned().unwrap_or_else(|| self.default_profile.clone())
    }

    /// Upserts a per-model restriction profile.
    pub fn upsert_model_profile(&mut self, profile: ModelRestrictionProfile) -> ModelRestrictionProfile {
        let key = profile.model_pattern.clone();
        self.model_profiles.insert(key, profile.clone());
        self.persist();
        profile
    }

    /// Removes a per-model restriction profile by pattern.
    pub fn remove_model_profile(&mut self, pattern: &str) -> bool {
        let removed = self.model_profiles.remove(pattern).is_some();
        if removed {
            self.persist();
        }
        removed
    }

    /// Evaluates a client request against the Admin's Global + Per-Model Restrictions.
    pub fn evaluate_request(
        &self,
        requested_model: &str,
        prompt_text: Option<&str>,
        command: Option<&str>,
        target_path: Option<&str>,
        client_requested_mode: Option<GuardEnforcementMode>,
    ) -> ModelGovernanceDecision {
        let mut profile = self.resolve_profile(requested_model);
        let mut effective_model = requested_model.to_string();
        let mut transparently_rerouted = false;
        let mut findings = Vec::new();
        let mut admin_reasons = Vec::new();

        // 1. Check if the requested model is disabled by the Administrator
        if !profile.enabled {
            if let Some(ref fb) = profile.fallback_model {
                effective_model = fb.clone();
                transparently_rerouted = true;
                profile = self.resolve_profile(&effective_model);
            } else {
                findings.push(GuardFinding {
                    rule_id: "ADMIN-MODEL-001-DISABLED".to_string(),
                    category: ThreatCategory::SpatialZoneViolation,
                    severity: "HIGH".to_string(),
                    description: format!(
                        "Model '{}' is disabled by Administrator policy with no fallback model",
                        requested_model
                    ),
                    matched_snippet: requested_model.to_string(),
                });
                admin_reasons.push(format!(
                    "[ADMIN-MODEL-001-DISABLED] Model '{}' is disabled by Administrator policy",
                    requested_model
                ));
            }
        }

        // 2. Determine authoritative GuardEnforcementMode (Client cannot weaken Admin's mode!)
        let effective_enforcement_mode = if self.global.ignore_client_mode_overrides {
            match client_requested_mode {
                // Allow a client or test harness to request HardBlock even if profile is RedactAndForward,
                // EXCEPT when the Admin explicitly locked the profile or when client tries to weaken HardBlock -> Audit/Redact.
                Some(GuardEnforcementMode::HardBlock) => GuardEnforcementMode::HardBlock,
                _ => profile.enforcement_mode,
            }
        } else {
            client_requested_mode.unwrap_or(profile.enforcement_mode)
        };

        // 3. Check per-model max prompt length ceiling
        if let Some(prompt) = prompt_text {
            if prompt.len() > profile.max_prompt_chars {
                findings.push(GuardFinding {
                    rule_id: "ADMIN-MODEL-002-PROMPT-CEILING".to_string(),
                    category: ThreatCategory::CredentialExfiltration,
                    severity: "HIGH".to_string(),
                    description: format!(
                        "Prompt length ({} chars) exceeds Admin ceiling ({} chars) for model '{}'",
                        prompt.len(),
                        profile.max_prompt_chars,
                        effective_model
                    ),
                    matched_snippet: format!("len={}", prompt.len()),
                });
                admin_reasons.push(format!(
                    "[ADMIN-MODEL-002-PROMPT-CEILING] Prompt exceeds {} char limit for model '{}'",
                    profile.max_prompt_chars, effective_model
                ));
            }
        }

        // 4. Check per-model shell execution permission & global blocked commands
        if let Some(cmd) = command {
            if !cmd.trim().is_empty() {
                if !profile.allow_shell_execution {
                    findings.push(GuardFinding {
                        rule_id: "ADMIN-MODEL-003-SHELL-FORBIDDEN".to_string(),
                        category: ThreatCategory::DestructiveShellCommand,
                        severity: "CRITICAL".to_string(),
                        description: format!(
                            "Administrator policy forbids shell command execution for model '{}' (tier: {:?})",
                            effective_model, profile.trust_tier
                        ),
                        matched_snippet: cmd.chars().take(80).collect(),
                    });
                    admin_reasons.push(format!(
                        "[ADMIN-MODEL-003-SHELL-FORBIDDEN] Model '{}' is not permitted to execute shell commands",
                        effective_model
                    ));
                }

                let cmd_low = cmd.to_lowercase();
                for banned in &self.global.global_blocked_commands {
                    if cmd_low.contains(&banned.to_lowercase()) {
                        findings.push(GuardFinding {
                            rule_id: "ADMIN-GLOBAL-CMD-BLOCK".to_string(),
                            category: ThreatCategory::DestructiveShellCommand,
                            severity: "CRITICAL".to_string(),
                            description: format!("Command matches Admin global blocked pattern '{}'", banned),
                            matched_snippet: banned.clone(),
                        });
                        admin_reasons.push(format!(
                            "[ADMIN-GLOBAL-CMD-BLOCK] Command banned by Global Admin policy ('{}')",
                            banned
                        ));
                    }
                }
            }
        }

        // 5. Check per-model file-write / path restrictions & playground confinement
        if let Some(path) = target_path {
            if !path.trim().is_empty() {
                let norm_path = path.replace('\\', "/").to_lowercase();

                if self.global.enforce_global_vault_lock
                    && (norm_path.contains(".secrets")
                        || norm_path.contains(".ssh")
                        || norm_path.ends_with(".env")
                        || norm_path.contains("/.env"))
                {
                    findings.push(GuardFinding {
                        rule_id: "ADMIN-GLOBAL-VAULT-LOCK".to_string(),
                        category: ThreatCategory::SpatialZoneViolation,
                        severity: "CRITICAL".to_string(),
                        description: format!("Global Admin Vault Lock prevents access to '{}'", path),
                        matched_snippet: path.to_string(),
                    });
                    admin_reasons.push(format!(
                        "[ADMIN-GLOBAL-VAULT-LOCK] Access to '{}' is locked across all models",
                        path
                    ));
                }

                if !profile.allow_file_writes {
                    findings.push(GuardFinding {
                        rule_id: "ADMIN-MODEL-004-WRITE-FORBIDDEN".to_string(),
                        category: ThreatCategory::SpatialZoneViolation,
                        severity: "HIGH".to_string(),
                        description: format!(
                            "Administrator policy restricts model '{}' to read-only (file writes disabled)",
                            effective_model
                        ),
                        matched_snippet: path.to_string(),
                    });
                    admin_reasons.push(format!(
                        "[ADMIN-MODEL-004-WRITE-FORBIDDEN] Model '{}' is not permitted to modify files",
                        effective_model
                    ));
                }

                if profile.confine_to_playground && !norm_path.contains("playground") {
                    findings.push(GuardFinding {
                        rule_id: "ADMIN-MODEL-005-PLAYGROUND-CONFINEMENT".to_string(),
                        category: ThreatCategory::SpatialZoneViolation,
                        severity: "HIGH".to_string(),
                        description: format!(
                            "Model '{}' is confined to 'playground/' by Administrator policy (attempted: '{}')",
                            effective_model, path
                        ),
                        matched_snippet: path.to_string(),
                    });
                    admin_reasons.push(format!(
                        "[ADMIN-MODEL-005-PLAYGROUND-CONFINEMENT] Model '{}' may only write inside playground/",
                        effective_model
                    ));
                }

                for blocked_pat in &profile.blocked_path_patterns {
                    if norm_path.contains(&blocked_pat.replace('\\', "/").to_lowercase()) {
                        findings.push(GuardFinding {
                            rule_id: "ADMIN-MODEL-006-PATH-BLOCKED".to_string(),
                            category: ThreatCategory::SpatialZoneViolation,
                            severity: "HIGH".to_string(),
                            description: format!(
                                "Path '{}' matches per-model blocked pattern '{}' for model '{}'",
                                path, blocked_pat, effective_model
                            ),
                            matched_snippet: blocked_pat.clone(),
                        });
                        admin_reasons.push(format!(
                            "[ADMIN-MODEL-006-PATH-BLOCKED] Path '{}' is restricted for model '{}'",
                            path, effective_model
                        ));
                    }
                }

                if !profile.allowed_path_prefixes.is_empty() {
                    let allowed_match = profile.allowed_path_prefixes.iter().any(|prefix| {
                        let p_norm = prefix.replace('\\', "/").to_lowercase();
                        norm_path.starts_with(&p_norm) || norm_path.contains(&p_norm)
                    });
                    if !allowed_match {
                        findings.push(GuardFinding {
                            rule_id: "ADMIN-MODEL-007-PATH-NOT-WHITELISTED".to_string(),
                            category: ThreatCategory::SpatialZoneViolation,
                            severity: "HIGH".to_string(),
                            description: format!(
                                "Path '{}' is outside the Admin allowed prefixes {:?} for model '{}'",
                                path, profile.allowed_path_prefixes, effective_model
                            ),
                            matched_snippet: path.to_string(),
                        });
                        admin_reasons.push(format!(
                            "[ADMIN-MODEL-007-PATH-NOT-WHITELISTED] Path '{}' not in model whitelist",
                            path
                        ));
                    }
                }
            }
        }

        let allowed = findings.is_empty();
        let shielded_client_message = if !allowed {
            Some(format!(
                "🛡️ [AgentTrace Shielded Terminal] This action is restricted by your Administrator's policy for model `{}` ({:?} tier). Your session remains active.",
                effective_model, profile.trust_tier
            ))
        } else {
            None
        };

        ModelGovernanceDecision {
            requested_model: requested_model.to_string(),
            effective_model,
            transparently_rerouted,
            matched_profile_pattern: profile.model_pattern,
            trust_tier: profile.trust_tier,
            effective_enforcement_mode,
            allowed,
            findings,
            admin_reasons,
            shielded_client_message,
            injected_system_prefix: profile.admin_system_prompt_prefix,
        }
    }
}

/// Verifies that the caller has Administrator privileges when mutating governance rules.
/// Rejects any caller identifying as a shielded client (`x-client-role: client`) or lacking
/// the configured `AGENTTRACE_ADMIN_KEY` when `require_admin_key` is active.
pub fn verify_admin_access(headers: &HeaderMap, engine: &AdminGovernanceEngine) -> bool {
    if let Some(role) = headers.get("x-client-role").and_then(|v| v.to_str().ok()) {
        if role.eq_ignore_ascii_case("client") || role.eq_ignore_ascii_case("claude-terminal") {
            return false;
        }
    }

    if let Ok(expected_admin_key) = std::env::var("AGENTTRACE_ADMIN_KEY") {
        if !expected_admin_key.is_empty() {
            let provided = headers
                .get("x-admin-key")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            return constant_time_eq(provided, &expected_admin_key);
        }
    }

    if engine.global.require_admin_key {
        let provided = headers
            .get("x-admin-key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        return !provided.is_empty();
    }

    true
}

// ============================================================================
// HTTP Handlers for Admin Governance Plane & Shielded Client Status
// ============================================================================

/// `GET /api/v1/admin/governance` — Returns global restrictions and all per-model profiles.
pub async fn get_admin_governance_handler(
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let eng = global_admin_governance()
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !verify_admin_access(&headers, &eng) {
        return Err(StatusCode::FORBIDDEN);
    }
    let mut profiles: Vec<&ModelRestrictionProfile> = eng.model_profiles.values().collect();
    profiles.sort_by(|a, b| a.model_pattern.cmp(&b.model_pattern));
    Ok(Json(serde_json::json!({
        "global": eng.global,
        "model_profiles": profiles,
        "default_profile": eng.default_profile,
    })))
}

/// `POST /api/v1/admin/governance/global` — Updates Global Admin Restrictions.
pub async fn update_global_governance_handler(
    headers: HeaderMap,
    Json(new_global): Json<GlobalAdminRestrictions>,
) -> Result<Json<GlobalAdminRestrictions>, StatusCode> {
    let mut eng = global_admin_governance()
        .write()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !verify_admin_access(&headers, &eng) {
        return Err(StatusCode::FORBIDDEN);
    }
    eng.global = new_global.clone();
    eng.persist();
    Ok(Json(new_global))
}

/// `POST /api/v1/admin/governance/model` — Upserts a per-model restriction profile.
pub async fn upsert_model_profile_handler(
    headers: HeaderMap,
    Json(profile): Json<ModelRestrictionProfile>,
) -> Result<Json<ModelRestrictionProfile>, StatusCode> {
    if profile.model_pattern.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut eng = global_admin_governance()
        .write()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !verify_admin_access(&headers, &eng) {
        return Err(StatusCode::FORBIDDEN);
    }
    let saved = eng.upsert_model_profile(profile);
    Ok(Json(saved))
}

/// `DELETE /api/v1/admin/governance/model/:pattern` — Deletes a custom per-model profile.
pub async fn delete_model_profile_handler(
    headers: HeaderMap,
    Path(pattern): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut eng = global_admin_governance()
        .write()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !verify_admin_access(&headers, &eng) {
        return Err(StatusCode::FORBIDDEN);
    }
    let removed = eng.remove_model_profile(&pattern);
    Ok(Json(serde_json::json!({
        "model_pattern": pattern,
        "removed": removed
    })))
}

#[derive(Debug, Deserialize)]
pub struct AdminEvaluateModelRequest {
    pub model: String,
    #[serde(default)]
    pub prompt_text: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub target_path: Option<String>,
    #[serde(default)]
    pub client_requested_mode: Option<GuardEnforcementMode>,
}

/// `POST /api/v1/admin/governance/evaluate` — Dry-run simulator for per-model restrictions.
pub async fn evaluate_model_governance_handler(
    headers: HeaderMap,
    Json(req): Json<AdminEvaluateModelRequest>,
) -> Result<(StatusCode, Json<ModelGovernanceDecision>), StatusCode> {
    let eng = global_admin_governance()
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !verify_admin_access(&headers, &eng) {
        return Err(StatusCode::FORBIDDEN);
    }
    let decision = eng.evaluate_request(
        &req.model,
        req.prompt_text.as_deref(),
        req.command.as_deref(),
        req.target_path.as_deref(),
        req.client_requested_mode,
    );
    let status = if decision.allowed {
        StatusCode::OK
    } else {
        StatusCode::FORBIDDEN
    };
    Ok((status, Json(decision)))
}

/// `GET /api/v1/client/shield-status` — Safe, shielded view for the developer's Claude Terminal client.
/// Exposes zero internal regexes, blocked path patterns, or admin secrets.
pub async fn get_client_shield_status_handler() -> Result<Json<serde_json::Value>, StatusCode> {
    let eng = global_admin_governance()
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let approved_models: Vec<serde_json::Value> = eng
        .model_profiles
        .values()
        .filter(|p| p.enabled)
        .map(|p| {
            serde_json::json!({
                "model_pattern": p.model_pattern,
                "display_name": p.display_name,
                "trust_tier": p.trust_tier,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "client_mode": "shielded-claude-terminal",
        "shield_active": eng.global.shield_client_terminals,
        "zero_config_connection": "ANTHROPIC_BASE_URL=http://127.0.0.1:8080 claude",
        "approved_models": approved_models,
        "notice": "Your terminal is protected by Administrator model governance. Sensitive tokens and unauthorized tool mutations are filtered automatically."
    })))
}
