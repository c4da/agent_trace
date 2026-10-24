//! Safe Zone (Protected Vault / Guarded Read-Only) & Danger Zone (Agent Playground) Containment Engine.
//!
//! Enforces strict spatial boundaries across global (`*`) and per-agent scopes,
//! preventing path traversal (`..`) and shell command escapes into guarded directories.

use crate::policy::{matches_pattern, AccessAction, PolicyDecision};
use crate::schema::SecurityVerdict;
use serde::{Deserialize, Serialize};

/// Defines the security posture of a filesystem directory zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZoneKind {
    /// Strict Safe Zone (Vault): Zero agent access. Read, Write, and Execute are all BLOCKED.
    VaultNoAccess,
    /// Guarded Safe Zone (Protected Core): Agents may Read for context, but Write & destructive Execute are BLOCKED.
    GuardedReadOnly,
    /// Danger Zone (Agent Playground): Dedicated sandbox directory where agents are free to Read, Write, and Execute.
    AgentPlayground,
}

/// A named spatial containment zone enforced by the gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainmentZone {
    pub zone_id: String,
    pub name: String,
    pub path_prefix: String,
    pub kind: ZoneKind,
    /// `"*"` for all agents, or a specific `agent_id` (e.g. `"PC-2"`, `"PC-2-Rogue-Subagent"`).
    #[serde(default = "default_all_agents")]
    pub applies_to: String,
    /// Optional list of trusted agent IDs exempt from a VaultNoAccess or GuardedReadOnly restriction.
    #[serde(default)]
    pub exempt_agents: Vec<String>,
    #[serde(default)]
    pub description: String,
}

fn default_all_agents() -> String {
    "*".to_string()
}

/// Request payload to toggle strict Playground Confinement for an agent or globally (`*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaygroundConfinementRequest {
    pub agent_id: String,
    pub confined: bool,
}

/// Canonicalizes a path string and collapses `.` and `..` traversal segments lexically.
pub fn resolve_canonical_target(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if s.starts_with("file:///") {
        // Windows file:///C:/foo -> C:/foo; Unix file:///workspace/foo -> /workspace/foo
        let after = &s[8..];
        if after.chars().nth(1) == Some(':') {
            s = after.to_string();
        } else {
            s = s[7..].to_string();
        }
    } else if s.starts_with("file://") {
        s = s[7..].to_string();
    }
    s = s.replace('\\', "/").to_lowercase();

    // Preserve URL schemes like https:// or http:// without path collapsing
    if s.starts_with("http://") || s.starts_with("https://") {
        return s;
    }

    let is_abs_slash = s.starts_with('/');
    let mut stack: Vec<&str> = Vec::new();

    for part in s.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            // Do not pop Windows drive letters like "c:"
            if let Some(last) = stack.last() {
                if !last.ends_with(':') {
                    stack.pop();
                }
            }
        } else {
            stack.push(part);
        }
    }

    let joined = stack.join("/");
    let preliminary = if is_abs_slash {
        format!("/{}", joined)
    } else {
        joined
    };

    // If candidate path exists and is a symlink/junction, resolve its target to prevent symlink traversal
    let candidate = std::path::Path::new(&preliminary);
    if let Ok(target) = std::fs::read_link(candidate) {
        let mut canon_str = target.to_string_lossy().to_string();
        if canon_str.starts_with(r"\\?\") {
            canon_str = canon_str[4..].to_string();
        }
        return canon_str.replace('\\', "/").to_lowercase();
    }

    preliminary
}

/// Checks whether a zone rule applies to the given `agent_id` (supports both base agent IDs and `#instance` tagged IDs).
pub fn zone_applies_to_agent(zone: &ContainmentZone, agent_id: &str) -> bool {
    let agent_lower = agent_id.to_lowercase();
    let base_agent = agent_lower.split('#').next().unwrap_or(&agent_lower);
    if zone.exempt_agents.iter().any(|ex| {
        let ex_l = ex.to_lowercase();
        ex_l == agent_lower || ex_l == base_agent
    }) {
        return false;
    }
    let scope = zone.applies_to.trim().to_lowercase();
    scope == "*" || scope == "all" || scope == agent_lower || scope == base_agent
}

/// Extracts candidate filesystem paths and destructive flags from a shell command string.
pub fn analyze_shell_command(cmd: &str) -> (bool, Vec<String>) {
    let lower = cmd.to_lowercase();
    let is_destructive = [
        "rm -rf",
        "rm -r ",
        "del /f",
        "del /s",
        "remove-item",
        "ri ",
        "rmdir",
        "format ",
        "git reset --hard",
        "git push --force",
        "git clean -fd",
        " > ",
        " >> ",
        "out-file",
        "set-content",
        "add-content",
        "mv ",
        "move-item",
        "cp ",
        "copy-item",
    ]
    .iter()
    .any(|pat| lower.contains(pat));

    let mut extracted_paths = Vec::new();
    for raw_tok in cmd.split_whitespace() {
        let tok = raw_tok.trim_matches(|c: char| c == '"' || c == '\'' || c == ';' || c == ',');
        if tok.contains('/') || tok.contains('\\') || tok.starts_with('.') {
            extracted_paths.push(resolve_canonical_target(tok));
        }
    }

    (is_destructive, extracted_paths)
}

/// Returns the default Safe Zones and Danger Zone Playgrounds for the cluster.
pub fn default_cluster_zones() -> Vec<ContainmentZone> {
    vec![
        ContainmentZone {
            zone_id: "zone-vault-secrets".to_string(),
            name: "Credentials & Secrets Vault".to_string(),
            path_prefix: "/workspace/.secrets".to_string(),
            kind: ZoneKind::VaultNoAccess,
            applies_to: "*".to_string(),
            exempt_agents: vec![],
            description: "Strict No-Access Vault for API keys, tokens, and cluster secrets.".to_string(),
        },
        ContainmentZone {
            zone_id: "zone-vault-ssh".to_string(),
            name: "Host SSH & Crypto Keys".to_string(),
            path_prefix: "*.ssh*".to_string(),
            kind: ZoneKind::VaultNoAccess,
            applies_to: "*".to_string(),
            exempt_agents: vec![],
            description: "Strict No-Access Vault preventing any agent from reading SSH keys.".to_string(),
        },
        ContainmentZone {
            zone_id: "zone-guarded-core".to_string(),
            name: "Core Production Repository".to_string(),
            path_prefix: "/workspace/core-repo".to_string(),
            kind: ZoneKind::GuardedReadOnly,
            applies_to: "*".to_string(),
            exempt_agents: vec!["PC-1-Antigravity".to_string()],
            description: "Guarded Safe Zone: Subagents and worker nodes may read for reference, but only lead architect can write.".to_string(),
        },
        ContainmentZone {
            zone_id: "zone-playground-lab".to_string(),
            name: "Agent Experiment Playground".to_string(),
            path_prefix: "/workspace/playground".to_string(),
            kind: ZoneKind::AgentPlayground,
            applies_to: "*".to_string(),
            exempt_agents: vec![],
            description: "Danger Zone Playground: Autonomous agents have full Read/Write/Execute freedom here.".to_string(),
        },
        ContainmentZone {
            zone_id: "zone-playground-sandbox".to_string(),
            name: "Cluster Temp Sandbox".to_string(),
            path_prefix: "/workspace/sandbox".to_string(),
            kind: ZoneKind::AgentPlayground,
            applies_to: "*".to_string(),
            exempt_agents: vec![],
            description: "Danger Zone Playground: Isolated scratch directory for untrusted subagent tests.".to_string(),
        },
    ]
}

/// Evaluates a single normalized target path against active Containment Zones.
pub fn evaluate_zones_for_path(
    zones: &[ContainmentZone],
    agent_id: &str,
    action: AccessAction,
    norm_target: &str,
    enforcing: bool,
) -> Option<PolicyDecision> {
    // Priority 1: VaultNoAccess Safe Zones (Blocks Read, Write, Execute)
    for z in zones.iter().filter(|z| z.kind == ZoneKind::VaultNoAccess) {
        if zone_applies_to_agent(z, agent_id) && matches_pattern(norm_target, &z.path_prefix) {
            let verdict = if enforcing {
                SecurityVerdict::Blocked
            } else {
                SecurityVerdict::Flagged
            };
            return Some(PolicyDecision {
                verdict,
                reasons: vec![format!(
                    "SAFE ZONE VIOLATION [VaultNoAccess '{}' ({})]: Agent '{}' is strictly forbidden from accessing '{}'",
                    z.name, z.path_prefix, agent_id, norm_target
                )],
                evaluated_agent: agent_id.to_string(),
                normalized_target: norm_target.to_string(),
                action,
                allowed_by_rule: None,
            });
        }
    }

    // Priority 2: GuardedReadOnly Safe Zones (Allows Read, Blocks Write)
    for z in zones.iter().filter(|z| z.kind == ZoneKind::GuardedReadOnly) {
        if zone_applies_to_agent(z, agent_id) && matches_pattern(norm_target, &z.path_prefix) {
            if action == AccessAction::Write {
                let verdict = if enforcing {
                    SecurityVerdict::Blocked
                } else {
                    SecurityVerdict::Flagged
                };
                return Some(PolicyDecision {
                    verdict,
                    reasons: vec![format!(
                        "SAFE ZONE VIOLATION [GuardedReadOnly '{}' ({})]: Agent '{}' has Read-Only access and cannot modify '{}'",
                        z.name, z.path_prefix, agent_id, norm_target
                    )],
                    evaluated_agent: agent_id.to_string(),
                    normalized_target: norm_target.to_string(),
                    action,
                    allowed_by_rule: None,
                });
            }
            if action == AccessAction::Read {
                return Some(PolicyDecision {
                    verdict: SecurityVerdict::Allowed,
                    reasons: vec![],
                    evaluated_agent: agent_id.to_string(),
                    normalized_target: norm_target.to_string(),
                    action,
                    allowed_by_rule: Some(format!("safe_zone_readonly:{}", z.zone_id)),
                });
            }
        }
    }

    // Priority 3: AgentPlayground Danger Zones (Explicitly permits Read, Write, Execute)
    for z in zones.iter().filter(|z| z.kind == ZoneKind::AgentPlayground) {
        if zone_applies_to_agent(z, agent_id) && matches_pattern(norm_target, &z.path_prefix) {
            return Some(PolicyDecision {
                verdict: SecurityVerdict::Allowed,
                reasons: vec![],
                evaluated_agent: agent_id.to_string(),
                normalized_target: norm_target.to_string(),
                action,
                allowed_by_rule: Some(format!("danger_zone_playground:{}", z.zone_id)),
            });
        }
    }

    None
}
