//! Autonomous AI Agent Access Whitelist, Safe Zone & Playground Containment Engine.
//!
//! Provides granular path-based and network-based whitelisting, Safe Zone vaults,
//! Guarded Read-Only repositories, and Danger Zone Agent Playgrounds.

use crate::persistence::{global_persistence, UnifiedPersistenceManager};
use crate::schema::{EventType, SecurityVerdict};
use crate::zones::{
    analyze_shell_command, default_cluster_zones, evaluate_zones_for_path,
    resolve_canonical_target, zone_applies_to_agent, ContainmentZone, ZoneKind,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Defines an access action type for policy evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessAction {
    Read,
    Write,
    Execute,
    NetworkPull,
}

impl AccessAction {
    pub fn from_event_type(ev: &EventType) -> Self {
        match ev {
            EventType::FileRead => AccessAction::Read,
            EventType::FileWrite => AccessAction::Write,
            EventType::CommandRun => AccessAction::Execute,
            EventType::WebRequest => AccessAction::NetworkPull,
            EventType::DecisionStep => AccessAction::Execute,
            EventType::LlmInference => AccessAction::NetworkPull,
        }
    }
}

/// Normalizes file and network target paths across platforms (Windows / Unix) and collapses `..` traversal.
pub fn normalize_target_path(target: &str) -> String {
    resolve_canonical_target(target)
}

/// Checks whether a normalized target matches an allowed pattern or prefix.
pub fn matches_pattern(target: &str, pattern: &str) -> bool {
    let p_norm = normalize_target_path(pattern);
    if p_norm == "*" || p_norm == "**" {
        return true;
    }

    if let Some(inner) = p_norm.strip_prefix('*').and_then(|s| s.strip_suffix('*')) {
        return !inner.is_empty() && target.contains(inner);
    }
    if let Some(prefix) = p_norm.strip_suffix("/**") {
        return target == prefix || target.starts_with(&format!("{}/", prefix));
    }
    if let Some(prefix) = p_norm.strip_suffix("/*") {
        return target == prefix || target.starts_with(&format!("{}/", prefix));
    }

    target == p_norm
        || target.starts_with(&format!("{}/", p_norm.trim_end_matches('/')))
        || target.starts_with(&p_norm)
}

/// A specific permission policy rule assigned to an agent or group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAccessPolicy {
    /// Unique agent identifier (or "*" for global baseline).
    pub agent_id: String,
    /// Whitelist: Allowed path prefixes or patterns where the agent can pull/read data.
    #[serde(default)]
    pub read_whitelist: Vec<String>,
    /// Whitelist: Allowed path prefixes or patterns where the agent can write/modify data.
    #[serde(default)]
    pub write_whitelist: Vec<String>,
    /// Whitelist: Allowed domain or URL prefixes the agent can pull network data from.
    #[serde(default)]
    pub network_whitelist: Vec<String>,
    /// Blacklist: Explicitly forbidden path or target patterns (e.g. ".env", "id_rsa", ".secrets").
    #[serde(default)]
    pub blocked_patterns: Vec<String>,
    /// Whether enforcement mode is active (true = Blocked on violation, false = Flagged audit only).
    #[serde(default = "default_enforce_true")]
    pub enforcing: bool,
    /// When true, this agent (or `*` all agents) can ONLY write or execute inside Danger Zone `AgentPlayground` directories.
    #[serde(default)]
    pub confine_to_playgrounds: bool,
}

fn default_enforce_true() -> bool {
    true
}

/// The outcome of an access permission evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub verdict: SecurityVerdict,
    pub reasons: Vec<String>,
    pub evaluated_agent: String,
    pub normalized_target: String,
    pub action: AccessAction,
    pub allowed_by_rule: Option<String>,
}

impl AgentAccessPolicy {
    /// Evaluates an access request against this agent's policy rules.
    pub fn evaluate(&self, action: AccessAction, target: &str) -> PolicyDecision {
        let norm_target = normalize_target_path(target);
        let mut reasons = Vec::new();

        // 1. Strict Blacklist Check (Priority 1: Always evaluated first)
        for blocked in &self.blocked_patterns {
            let b_norm = normalize_target_path(blocked);
            if !b_norm.is_empty() && norm_target.contains(&b_norm) {
                let reason = format!(
                    "Policy violation: Target '{}' matches blocked sensitive pattern '{}'",
                    target, blocked
                );
                reasons.push(reason);

                let verdict = if self.enforcing {
                    SecurityVerdict::Blocked
                } else {
                    SecurityVerdict::Flagged
                };

                return PolicyDecision {
                    verdict,
                    reasons,
                    evaluated_agent: self.agent_id.clone(),
                    normalized_target: norm_target,
                    action,
                    allowed_by_rule: None,
                };
            }
        }

        // 2. Whitelist Check (Priority 2: Must be within allowed locations if whitelist is set)
        let (whitelist, action_name) = match action {
            AccessAction::Read => (&self.read_whitelist, "read/pull data"),
            AccessAction::Write => (&self.write_whitelist, "write/modify data"),
            AccessAction::NetworkPull => (&self.network_whitelist, "network fetch"),
            AccessAction::Execute => (&self.read_whitelist, "execution target"),
        };

        if !whitelist.is_empty() {
            let matched_rule = whitelist.iter().find(|p| matches_pattern(&norm_target, p));

            if let Some(rule) = matched_rule {
                return PolicyDecision {
                    verdict: SecurityVerdict::Allowed,
                    reasons: vec![],
                    evaluated_agent: self.agent_id.clone(),
                    normalized_target: norm_target,
                    action,
                    allowed_by_rule: Some(rule.clone()),
                };
            } else {
                let reason = format!(
                    "Policy violation: Target '{}' is not in allowed {} whitelist (configured rules: {:?})",
                    target, action_name, whitelist
                );
                reasons.push(reason);

                let verdict = if self.enforcing {
                    SecurityVerdict::Blocked
                } else {
                    SecurityVerdict::Flagged
                };

                return PolicyDecision {
                    verdict,
                    reasons,
                    evaluated_agent: self.agent_id.clone(),
                    normalized_target: norm_target,
                    action,
                    allowed_by_rule: None,
                };
            }
        }

        PolicyDecision {
            verdict: SecurityVerdict::Allowed,
            reasons: vec![],
            evaluated_agent: self.agent_id.clone(),
            normalized_target: norm_target,
            action,
            allowed_by_rule: Some("unrestricted".to_string()),
        }
    }
}

/// Autonomous Access Policy & Zone Containment Engine store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEngine {
    policies: HashMap<String, AgentAccessPolicy>,
    zones: Vec<ContainmentZone>,
    #[serde(skip)]
    persistence: Option<Arc<UnifiedPersistenceManager>>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new_with_defaults()
    }
}

impl PolicyEngine {
    /// Loads persisted policies and zones from `global_persistence()`, or initializes defaults.
    pub fn load_or_default() -> Self {
        Self::new_with_persistence(global_persistence())
    }

    /// Initializes the Policy Engine backed by a specific `UnifiedPersistenceManager`.
    pub fn new_with_persistence(pm: Arc<UnifiedPersistenceManager>) -> Self {
        if let Some(mut loaded) = pm.load_struct::<PolicyEngine>("policies", "engine") {
            let defaults = Self::new_with_defaults();
            for (k, v) in defaults.policies {
                loaded.policies.entry(k).or_insert(v);
            }
            for dz in defaults.zones {
                if !loaded.zones.iter().any(|z| z.zone_id == dz.zone_id) {
                    loaded.zones.push(dz);
                }
            }
            loaded.persistence = Some(pm);
            return loaded;
        }
        let mut eng = Self::new_with_defaults();
        eng.persistence = Some(pm);
        eng.persist();
        eng
    }

    /// Reloads policies and zones from the configured `UnifiedPersistenceManager`.
    pub fn reload_from_persistence(&mut self) {
        let pm = self.persistence.clone().unwrap_or_else(global_persistence);
        if let Some(loaded) = pm.load_struct::<PolicyEngine>("policies", "engine") {
            for (k, v) in loaded.policies {
                self.policies.insert(k, v);
            }
            self.zones = loaded.zones;
        }
    }

    /// Persists policies and zones to Local JSON, SQLite, and External DB.
    pub fn persist(&self) {
        if let Some(ref pm) = self.persistence {
            let _ = pm.save_struct("policies", "engine", self);
        }
    }

    /// Creates a new policy engine pre-configured with secure cluster defaults and zones.
    pub fn new_with_defaults() -> Self {
        let mut engine = Self {
            policies: HashMap::new(),
            zones: default_cluster_zones(),
            persistence: None,
        };

        let read_whitelist = vec![
            "/workspace".to_string(),
            "src".to_string(),
            "tests".to_string(),
            "crates".to_string(),
            "target".to_string(),
            "docs".to_string(),
            "sdk".to_string(),
            "playground".to_string(),
        ];

        let default_policy = AgentAccessPolicy {
            agent_id: "*".to_string(),
            read_whitelist,
            write_whitelist: vec![
                "/workspace".to_string(),
                "src".to_string(),
                "tests".to_string(),
                "crates".to_string(),
                "target".to_string(),
                "docs".to_string(),
                "sdk".to_string(),
                "playground".to_string(),
                "cluster_telemetry.db".to_string(),
                "worker_telemetry.db".to_string(),
            ],
            network_whitelist: vec![
                "http://127.0.0.1".to_string(),
                "http://localhost".to_string(),
                "http://192.168.1.101".to_string(),
                "http://192.168.1.102".to_string(),
                "https://api.github.com".to_string(),
                "https://github.com".to_string(),
                "https://huggingface.co".to_string(),
            ],
            blocked_patterns: vec![
                ".env".to_string(),
                ".secrets".to_string(),
                "id_rsa".to_string(),
                "id_ed25519".to_string(),
                ".pem".to_string(),
                ".key".to_string(),
                "credentials".to_string(),
                "c:/windows/system32".to_string(),
                "/etc/shadow".to_string(),
                "/etc/passwd".to_string(),
            ],
            enforcing: true,
            confine_to_playgrounds: false,
        };

        engine.set_policy(default_policy);
        engine
    }

    /// Evaluates an access request across Global Blacklists, Safe/Danger Zones, Playground Confinement, and Whitelists.
    pub fn evaluate(&self, agent_id: &str, action: AccessAction, target: &str) -> PolicyDecision {
        let norm_target = normalize_target_path(target);
        let active_policy = self
            .policies
            .get(agent_id)
            .or_else(|| self.policies.get("*"));
        let enforcing = active_policy.map(|p| p.enforcing).unwrap_or(true);
        let confined = self
            .policies
            .get(agent_id)
            .map(|p| p.confine_to_playgrounds)
            .unwrap_or(false)
            || self
                .policies
                .get("*")
                .map(|p| p.confine_to_playgrounds)
                .unwrap_or(false);

        // 1. Global & Agent Sensitive Pattern Blacklist (Always enforced first, cannot be bypassed by per-agent overrides)
        let mut combined_blocked: Vec<&String> = Vec::new();
        if let Some(glob) = self.policies.get("*") {
            combined_blocked.extend(glob.blocked_patterns.iter());
        }
        if let Some(spec) = self.policies.get(agent_id) {
            combined_blocked.extend(spec.blocked_patterns.iter());
        }

        for blocked in combined_blocked {
            let b_norm = normalize_target_path(blocked);
            if !b_norm.is_empty() && norm_target.contains(&b_norm) {
                let verdict = if enforcing {
                    SecurityVerdict::Blocked
                } else {
                    SecurityVerdict::Flagged
                };
                return PolicyDecision {
                    verdict,
                    reasons: vec![format!(
                        "Policy violation: Target '{}' matches blocked sensitive pattern '{}'",
                        target, blocked
                    )],
                    evaluated_agent: agent_id.to_string(),
                    normalized_target: norm_target,
                    action,
                    allowed_by_rule: None,
                };
            }
        }

        // 2. Shell Command Deep Inspection (Execute actions)
        if action == AccessAction::Execute {
            let (is_destructive, cmd_paths) = analyze_shell_command(target);
            for cpath in &cmd_paths {
                let sub_action = if is_destructive {
                    AccessAction::Write
                } else {
                    AccessAction::Read
                };
                if let Some(mut z_dec) =
                    evaluate_zones_for_path(&self.zones, agent_id, sub_action, cpath, enforcing)
                {
                    if z_dec.verdict != SecurityVerdict::Allowed {
                        z_dec.action = AccessAction::Execute;
                        z_dec.reasons.insert(
                            0,
                            format!(
                                "Shell command '{}' touches protected zone path '{}'",
                                target, cpath
                            ),
                        );
                        return z_dec;
                    }
                }
            }
        }

        // 3. Spatial Containment Zones (VaultNoAccess -> GuardedReadOnly -> AgentPlayground)
        if action != AccessAction::NetworkPull {
            if let Some(zone_decision) =
                evaluate_zones_for_path(&self.zones, agent_id, action, &norm_target, enforcing)
            {
                return zone_decision;
            }

            // 4. Strict Playground Confinement ("Danger Zone Lock")
            if confined && (action == AccessAction::Write || action == AccessAction::Execute) {
                let playgrounds: Vec<String> = self
                    .zones
                    .iter()
                    .filter(|z| z.kind == ZoneKind::AgentPlayground && zone_applies_to_agent(z, agent_id))
                    .map(|z| z.path_prefix.clone())
                    .collect();

                let in_playground = if action == AccessAction::Execute {
                    let (_, cmd_paths) = analyze_shell_command(target);
                    if cmd_paths.is_empty() {
                        playgrounds.iter().any(|p| matches_pattern(&norm_target, p))
                    } else {
                        cmd_paths
                            .iter()
                            .all(|cp| playgrounds.iter().any(|p| matches_pattern(cp, p)))
                    }
                } else {
                    playgrounds.iter().any(|p| matches_pattern(&norm_target, p))
                };

                if !in_playground {
                    let verdict = if enforcing {
                        SecurityVerdict::Blocked
                    } else {
                        SecurityVerdict::Flagged
                    };
                    return PolicyDecision {
                        verdict,
                        reasons: vec![format!(
                            "PLAYGROUND CONFINEMENT VIOLATION: Agent '{}' is locked to Danger Zone Playgrounds ({:?}) and cannot write/execute outside them ('{}')",
                            agent_id, playgrounds, target
                        )],
                        evaluated_agent: agent_id.to_string(),
                        normalized_target: norm_target,
                        action,
                        allowed_by_rule: None,
                    };
                }
            }
        }

        // 5. Standard Whitelist Evaluation
        if let Some(p) = self.policies.get(agent_id) {
            p.evaluate(action, target)
        } else if let Some(global) = self.policies.get("*") {
            let mut dec = global.evaluate(action, target);
            dec.evaluated_agent = agent_id.to_string();
            dec
        } else {
            PolicyDecision {
                verdict: SecurityVerdict::Allowed,
                reasons: vec![],
                evaluated_agent: agent_id.to_string(),
                normalized_target: norm_target,
                action,
                allowed_by_rule: Some("no_policy".to_string()),
            }
        }
    }

    /// Inserts or replaces an agent access policy.
    pub fn set_policy(&mut self, policy: AgentAccessPolicy) {
        self.policies.insert(policy.agent_id.clone(), policy);
        self.persist();
    }

    /// Toggles strict Playground Confinement for a specific agent or globally (`*`).
    pub fn set_playground_confinement(&mut self, agent_id: &str, confined: bool) -> AgentAccessPolicy {
        if let Some(existing) = self.policies.get_mut(agent_id) {
            existing.confine_to_playgrounds = confined;
            let res = existing.clone();
            self.persist();
            return res;
        }
        let mut base = self
            .policies
            .get("*")
            .cloned()
            .unwrap_or_else(|| AgentAccessPolicy {
                agent_id: agent_id.to_string(),
                read_whitelist: vec![],
                write_whitelist: vec![],
                network_whitelist: vec![],
                blocked_patterns: vec![".env".to_string(), ".secrets".to_string()],
                enforcing: true,
                confine_to_playgrounds: confined,
            });
        base.agent_id = agent_id.to_string();
        base.confine_to_playgrounds = confined;
        self.policies.insert(agent_id.to_string(), base.clone());
        self.persist();
        base
    }

    /// Retrieves an agent policy by ID.
    #[allow(dead_code)]
    pub fn get_policy(&self, agent_id: &str) -> Option<AgentAccessPolicy> {
        self.policies.get(agent_id).cloned()
    }

    /// Lists all currently active agent policies.
    pub fn list_policies(&self) -> Vec<AgentAccessPolicy> {
        self.policies.values().cloned().collect()
    }

    /// Removes an agent policy by ID.
    pub fn remove_policy(&mut self, agent_id: &str) -> bool {
        let removed = self.policies.remove(agent_id).is_some();
        if removed {
            self.persist();
        }
        removed
    }

    /// Lists all configured Safe Zones and Danger Zone Playgrounds.
    pub fn list_zones(&self) -> Vec<ContainmentZone> {
        self.zones.clone()
    }

    /// Adds or updates a Containment Zone.
    pub fn upsert_zone(&mut self, mut zone: ContainmentZone) -> ContainmentZone {
        zone.path_prefix = normalize_target_path(&zone.path_prefix);
        if zone.zone_id.trim().is_empty() {
            zone.zone_id = format!("zone-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("custom"));
        }
        if let Some(existing) = self.zones.iter_mut().find(|z| z.zone_id == zone.zone_id) {
            *existing = zone.clone();
        } else {
            self.zones.push(zone.clone());
        }
        self.persist();
        zone
    }

    /// Removes a Containment Zone by `zone_id`.
    pub fn remove_zone(&mut self, zone_id: &str) -> bool {
        let before = self.zones.len();
        self.zones.retain(|z| z.zone_id != zone_id);
        let removed = self.zones.len() < before;
        if removed {
            self.persist();
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_normalization_and_traversal_collapse() {
        assert_eq!(
            normalize_target_path(r"\workspace\agent_trace_gw\.env"),
            "/workspace/agent_trace_gw/.env"
        );
        assert_eq!(
            normalize_target_path("file:///workspace/core-repo/src/main.rs"),
            "/workspace/core-repo/src/main.rs"
        );
        assert_eq!(
            normalize_target_path("./src/policy.rs"),
            "src/policy.rs"
        );
        // Critical: Verify `..` path traversal out of playground is collapsed!
        assert_eq!(
            normalize_target_path(r"/workspace/playground/../../workspace/core-repo/src/main.rs"),
            "/workspace/core-repo/src/main.rs"
        );
    }

    #[test]
    fn test_whitelist_and_blacklist_enforcement() {
        let engine = PolicyEngine::new_with_defaults();

        // 1. Allowed path in whitelist
        let dec_allow = engine.evaluate(
            "hermes-agent",
            AccessAction::Write,
            r"/workspace/agent_trace_gw/src/main.rs",
        );
        assert_eq!(dec_allow.verdict, SecurityVerdict::Allowed);

        // 2. Blacklist violation takes precedence over whitelist: .env file inside allowed directory!
        let dec_env = engine.evaluate(
            "hermes-agent",
            AccessAction::Read,
            r"/workspace/agent_trace_gw/.env",
        );
        assert_eq!(dec_env.verdict, SecurityVerdict::Blocked);
        assert!(dec_env.reasons[0].contains(".env"));

        // 3. Sensitive pattern blocked by blacklist priority
        let dec_sys = engine.evaluate(
            "hermes-agent",
            AccessAction::Write,
            r"C:\Windows\System32\drivers\etc\hosts",
        );
        assert_eq!(dec_sys.verdict, SecurityVerdict::Blocked);
        assert!(dec_sys.reasons[0].contains("c:/windows/system32"));

        // 4. Unauthorized path outside whitelist boundary
        let dec_outside = engine.evaluate(
            "hermes-agent",
            AccessAction::Write,
            r"D:\unauthorized_dir\notes.txt",
        );
        assert_eq!(dec_outside.verdict, SecurityVerdict::Blocked);
        assert!(dec_outside.reasons[0].contains("not in allowed"));
    }

    #[test]
    fn test_safe_zone_vault_and_guarded_readonly_enforcement() {
        let engine = PolicyEngine::new_with_defaults();

        // 1. VaultNoAccess: Even Read is strictly BLOCKED for all agents
        let vault_read = engine.evaluate(
            "PC-1-Antigravity",
            AccessAction::Read,
            r"C:\Users\operator\.ssh\known_hosts",
        );
        assert_eq!(vault_read.verdict, SecurityVerdict::Blocked);
        assert!(vault_read.reasons[0].contains("SAFE ZONE VIOLATION"));

        // 2. GuardedReadOnly (`/workspace/core-repo`): Worker agent can Read, but Write is BLOCKED!
        let core_read = engine.evaluate(
            "PC-2-Worker",
            AccessAction::Read,
            r"/workspace/core-repo/src/main.rs",
        );
        assert_eq!(core_read.verdict, SecurityVerdict::Allowed);

        let core_write = engine.evaluate(
            "PC-2-Worker",
            AccessAction::Write,
            r"/workspace/core-repo/src/main.rs",
        );
        assert_eq!(core_write.verdict, SecurityVerdict::Blocked);
        assert!(core_write.reasons[0].contains("GuardedReadOnly"));

        // 3. Exempt Lead Architect (`PC-1-Antigravity`) IS permitted to write to core-repo
        let lead_write = engine.evaluate(
            "PC-1-Antigravity",
            AccessAction::Write,
            r"/workspace/core-repo/src/main.rs",
        );
        assert_eq!(lead_write.verdict, SecurityVerdict::Allowed);
    }

    #[test]
    fn test_danger_zone_playground_confinement_and_traversal_defense() {
        let mut engine = PolicyEngine::new_with_defaults();

        // Confine "PC-2-Rogue-Subagent" strictly to Danger Zone Playgrounds
        engine.set_playground_confinement("PC-2-Rogue-Subagent", true);

        // 1. Writing inside Danger Zone Playground (`/workspace/playground`) is ALLOWED
        let ok_play = engine.evaluate(
            "PC-2-Rogue-Subagent",
            AccessAction::Write,
            r"/workspace/playground/exploit_test.py",
        );
        assert_eq!(ok_play.verdict, SecurityVerdict::Allowed);
        assert_eq!(
            ok_play.allowed_by_rule.as_deref(),
            Some("danger_zone_playground:zone-playground-lab")
        );

        // 2. Writing outside Playground (even inside `/workspace/agent_trace_gw`) is BLOCKED!
        let blocked_out = engine.evaluate(
            "PC-2-Rogue-Subagent",
            AccessAction::Write,
            r"/workspace/agent_trace_gw/src/main.rs",
        );
        assert_eq!(blocked_out.verdict, SecurityVerdict::Blocked);
        assert!(blocked_out.reasons[0].contains("PLAYGROUND CONFINEMENT VIOLATION"));

        // 3. Attempting `../..` path traversal from Playground into Guarded Safe Zone is BLOCKED!
        let traversal_attempt = engine.evaluate(
            "PC-2-Rogue-Subagent",
            AccessAction::Write,
            r"/workspace/playground/../core-repo/src/lib.rs",
        );
        assert_eq!(traversal_attempt.verdict, SecurityVerdict::Blocked);

        // 4. Shell command attempting destructive action against Guarded Safe Zone is BLOCKED!
        let cmd_escape = engine.evaluate(
            "PC-2-Rogue-Subagent",
            AccessAction::Execute,
            r"Remove-Item -Recurse /workspace/core-repo/src",
        );
        assert_eq!(cmd_escape.verdict, SecurityVerdict::Blocked);
    }

    #[test]
    fn test_custom_agent_policy_override() {
        let mut engine = PolicyEngine::new_with_defaults();

        engine.set_policy(AgentAccessPolicy {
            agent_id: "untrusted-subagent".to_string(),
            read_whitelist: vec!["/workspace/sandbox".to_string()],
            write_whitelist: vec!["/workspace/sandbox/tmp".to_string()],
            network_whitelist: vec![],
            blocked_patterns: vec![".env".to_string()],
            enforcing: true,
            confine_to_playgrounds: false,
        });

        let dec_denied = engine.evaluate(
            "untrusted-subagent",
            AccessAction::Write,
            r"/workspace/agent_trace_gw/src/main.rs",
        );
        assert_eq!(dec_denied.verdict, SecurityVerdict::Blocked);

        let dec_ok = engine.evaluate(
            "untrusted-subagent",
            AccessAction::Write,
            r"/workspace/sandbox/tmp/output.json",
        );
        assert_eq!(dec_ok.verdict, SecurityVerdict::Allowed);
    }

    #[test]
    fn test_non_enforcing_audit_mode_flags_instead_of_blocks() {
        let mut engine = PolicyEngine::new_with_defaults();

        engine.set_policy(AgentAccessPolicy {
            agent_id: "audit-agent".to_string(),
            read_whitelist: vec!["/workspace/allowed".to_string()],
            write_whitelist: vec![],
            network_whitelist: vec![],
            blocked_patterns: vec![".env".to_string()],
            enforcing: false,
            confine_to_playgrounds: false,
        });

        let dec = engine.evaluate(
            "audit-agent",
            AccessAction::Read,
            r"C:\sensitive\report.pdf",
        );
        assert_eq!(dec.verdict, SecurityVerdict::Flagged);
        assert!(!dec.reasons.is_empty());
    }

    #[test]
    fn test_network_whitelist_evaluation() {
        let mut engine = PolicyEngine::new_with_defaults();

        engine.set_policy(AgentAccessPolicy {
            agent_id: "network-agent".to_string(),
            read_whitelist: vec![],
            write_whitelist: vec![],
            network_whitelist: vec![
                "https://api.github.com".to_string(),
                "http://localhost:8080".to_string(),
            ],
            blocked_patterns: vec![],
            enforcing: true,
            confine_to_playgrounds: false,
        });

        let dec_gh = engine.evaluate(
            "network-agent",
            AccessAction::NetworkPull,
            "https://api.github.com/repos",
        );
        assert_eq!(dec_gh.verdict, SecurityVerdict::Allowed);

        let dec_local = engine.evaluate(
            "network-agent",
            AccessAction::NetworkPull,
            "http://localhost:8080/v1/models",
        );
        assert_eq!(dec_local.verdict, SecurityVerdict::Allowed);

        let dec_evil = engine.evaluate(
            "network-agent",
            AccessAction::NetworkPull,
            "https://evil-exfiltration.com/data",
        );
        assert_eq!(dec_evil.verdict, SecurityVerdict::Blocked);
    }

    #[test]
    fn test_matches_pattern_wildcards() {
        assert!(matches_pattern("c:/foo/bar/baz.txt", "*"));
        assert!(matches_pattern("c:/foo/bar/baz.txt", "**"));
        assert!(matches_pattern("c:/foo/bar/baz.txt", "c:/foo/**"));
        assert!(matches_pattern("c:/foo/bar/baz.txt", "c:/foo/*"));
        assert!(matches_pattern("c:/foo/bar/baz.txt", "c:/foo/bar/baz.txt"));
        assert!(!matches_pattern("c:/other/bar/baz.txt", "c:/foo/**"));
    }
}
