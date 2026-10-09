//! Autonomous AI Agent Access Whitelist & Sandbox Policy Engine.
//!
//! Provides granular path-based and network-based whitelisting for AI agents,
//! restricting where agents can pull/read data, write data, or send outbound requests,
//! while strictly enforcing blacklists against sensitive targets (e.g. credentials, .env).

use crate::schema::{EventType, SecurityVerdict};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Normalizes file and network target paths across platforms (Windows / Unix).
pub fn normalize_target_path(target: &str) -> String {
    let mut norm = target.trim().to_string();

    // Strip URL schemes like file:/// or file://
    if norm.starts_with("file:///") {
        norm = norm[8..].to_string();
    } else if norm.starts_with("file://") {
        norm = norm[7..].to_string();
    }

    // Convert backslashes to forward slashes for unified cross-platform matching
    norm = norm.replace('\\', "/");

    // Remove leading ./ if present
    if norm.starts_with("./") {
        norm = norm[2..].to_string();
    }

    // Lowercase for Windows case-insensitive comparisons
    norm.to_lowercase()
}

/// Checks whether a normalized target matches an allowed pattern or prefix.
pub fn matches_pattern(target: &str, pattern: &str) -> bool {
    let p_norm = normalize_target_path(pattern);
    if p_norm == "*" || p_norm == "**" {
        return true;
    }

    // Check glob wildcard suffix (e.g. /path/** or /path/*)
    if let Some(prefix) = p_norm.strip_suffix("/**") {
        return target.starts_with(prefix);
    }
    if let Some(prefix) = p_norm.strip_suffix("/*") {
        return target.starts_with(prefix);
    }

    // Standard prefix or exact match
    target.starts_with(&p_norm) || target == p_norm
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
            let matched_rule = whitelist
                .iter()
                .find(|p| matches_pattern(&norm_target, p));

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

        // Whitelist is empty -> unrestricted access for this action type (unless blacklisted)
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

/// Autonomous Access Policy Engine store.
#[derive(Debug, Clone)]
pub struct PolicyEngine {
    policies: HashMap<String, AgentAccessPolicy>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new_with_defaults()
    }
}

impl PolicyEngine {
    /// Creates a new policy engine pre-configured with secure cluster defaults.
    pub fn new_with_defaults() -> Self {
        let mut engine = Self {
            policies: HashMap::new(),
        };

        // 1. Default Global Baseline Policy ("*")
        let default_policy = AgentAccessPolicy {
            agent_id: "*".to_string(),
            read_whitelist: vec![
                "c:/cada/projects".to_string(),
                "c:/users/milos/appdata/local/hermes".to_string(),
                "src".to_string(),
                "tests".to_string(),
                "crates".to_string(),
                "target".to_string(),
                "docs".to_string(),
            ],
            write_whitelist: vec![
                "c:/cada/projects".to_string(),
                "src".to_string(),
                "tests".to_string(),
                "crates".to_string(),
                "target".to_string(),
                "docs".to_string(),
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
        };

        engine.set_policy(default_policy);
        engine
    }

    /// Evaluates an access request: matches agent-specific policy first, then global "*".
    pub fn evaluate(&self, agent_id: &str, action: AccessAction, target: &str) -> PolicyDecision {
        if let Some(p) = self.policies.get(agent_id) {
            p.evaluate(action, target)
        } else if let Some(global) = self.policies.get("*") {
            let mut dec = global.evaluate(action, target);
            dec.evaluated_agent = agent_id.to_string();
            dec
        } else {
            // Unrestricted fallback if no policy configured
            PolicyDecision {
                verdict: SecurityVerdict::Allowed,
                reasons: vec![],
                evaluated_agent: agent_id.to_string(),
                normalized_target: normalize_target_path(target),
                action,
                allowed_by_rule: Some("no_policy".to_string()),
            }
        }
    }

    /// Inserts or replaces an agent access policy.
    pub fn set_policy(&mut self, policy: AgentAccessPolicy) {
        self.policies.insert(policy.agent_id.clone(), policy);
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
        self.policies.remove(agent_id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_normalization() {
        assert_eq!(
            normalize_target_path(r"C:\cada\projects\agent_trace_gw\.env"),
            "c:/cada/projects/agent_trace_gw/.env"
        );
        assert_eq!(
            normalize_target_path("file:///C:/cada/projects/RustCAD/src/main.rs"),
            "c:/cada/projects/rustcad/src/main.rs"
        );
        assert_eq!(
            normalize_target_path("./src/policy.rs"),
            "src/policy.rs"
        );
    }

    #[test]
    fn test_whitelist_and_blacklist_enforcement() {
        let engine = PolicyEngine::new_with_defaults();

        // 1. Allowed path in whitelist
        let dec_allow = engine.evaluate(
            "hermes-agent",
            AccessAction::Write,
            r"C:\cada\projects\agent_trace_gw\src\main.rs",
        );
        assert_eq!(dec_allow.verdict, SecurityVerdict::Allowed);

        // 2. Blacklist violation takes precedence over whitelist: .env file inside allowed directory!
        let dec_env = engine.evaluate(
            "hermes-agent",
            AccessAction::Read,
            r"C:\cada\projects\agent_trace_gw\.env",
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
    fn test_custom_agent_policy_override() {
        let mut engine = PolicyEngine::new_with_defaults();

        // Register custom restricted sandbox for "untrusted-subagent"
        engine.set_policy(AgentAccessPolicy {
            agent_id: "untrusted-subagent".to_string(),
            read_whitelist: vec!["c:/cada/projects/sandbox".to_string()],
            write_whitelist: vec!["c:/cada/projects/sandbox/tmp".to_string()],
            network_whitelist: vec![],
            blocked_patterns: vec![".env".to_string()],
            enforcing: true,
        });

        // Attempt write outside its specific sandbox: should be blocked
        let dec_denied = engine.evaluate(
            "untrusted-subagent",
            AccessAction::Write,
            r"C:\cada\projects\agent_trace_gw\src\main.rs",
        );
        assert_eq!(dec_denied.verdict, SecurityVerdict::Blocked);

        // Allowed write inside its specific sandbox
        let dec_ok = engine.evaluate(
            "untrusted-subagent",
            AccessAction::Write,
            r"C:\cada\projects\sandbox\tmp\output.json",
        );
        assert_eq!(dec_ok.verdict, SecurityVerdict::Allowed);
    }

    #[test]
    fn test_non_enforcing_audit_mode_flags_instead_of_blocks() {
        let mut engine = PolicyEngine::new_with_defaults();

        engine.set_policy(AgentAccessPolicy {
            agent_id: "audit-agent".to_string(),
            read_whitelist: vec!["c:/cada/projects/allowed".to_string()],
            write_whitelist: vec![],
            network_whitelist: vec![],
            blocked_patterns: vec![".env".to_string()],
            enforcing: false, // Audit mode only!
        });

        // Reading unauthorized path in non-enforcing mode results in Flagged, NOT Blocked
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
            network_whitelist: vec!["https://api.github.com".to_string(), "http://localhost:8080".to_string()],
            blocked_patterns: vec![],
            enforcing: true,
        });

        // Allowed network calls
        let dec_gh = engine.evaluate("network-agent", AccessAction::NetworkPull, "https://api.github.com/repos");
        assert_eq!(dec_gh.verdict, SecurityVerdict::Allowed);

        let dec_local = engine.evaluate("network-agent", AccessAction::NetworkPull, "http://localhost:8080/v1/models");
        assert_eq!(dec_local.verdict, SecurityVerdict::Allowed);

        // Forbidden network call
        let dec_evil = engine.evaluate("network-agent", AccessAction::NetworkPull, "https://evil-exfiltration.com/data");
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
