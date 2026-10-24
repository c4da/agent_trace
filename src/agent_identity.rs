//! Distinct Multi-Agent Identity & Instance Fingerprinting Engine.
//!
//! Resolves unique, human-readable, and cryptographically attributable agent identities
//! across concurrent IDE sessions, CLI tools (Claude Code, Copilot CLI, Hermes),
//! subagents, and remote worker nodes so the Gateway never conflates multiple agents.

use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

/// Structured identity of an AI agent instance interacting with the Gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Canonical composite identifier (e.g. `claude-cli:dev@VECTOR#s8a1f` or `antigravity:architect@PC-1#b0a75a`).
    pub canonical_id: String,
    /// Base engine or family (`claude-cli`, `antigravity`, `copilot-cli`, `qwen3.8:27b`, `deepseek-r1:1.5b`, `hermes`).
    pub engine: String,
    /// Specific role or subagent specialization (`lead`, `subagent`, `security-audit`, `coder`, `verifier`).
    pub role: String,
    /// Host workstation or cluster node (`PC-1`, `PC-2`, `CORP-DESKTOP`).
    pub host_node: String,
    /// Short deterministic session or process instance tag (`#b0a75a`, `#pid4812`).
    pub instance_tag: String,
}

impl AgentIdentity {
    /// Constructs a canonical agent identity from explicit components.
    pub fn new(engine: &str, role: &str, host_node: &str, session_or_pid: &str) -> Self {
        let clean_engine = sanitize_token(engine, "agent");
        let clean_role = sanitize_token(role, "main");
        let clean_host = sanitize_token(host_node, "LOCAL");
        let short_tag = short_instance_fingerprint(session_or_pid);

        let canonical_id = if clean_role == "main" || clean_role.is_empty() {
            format!("{}@{}#{}", clean_engine, clean_host, short_tag)
        } else {
            format!("{}:{}@{}#{}", clean_engine, clean_role, clean_host, short_tag)
        };

        Self {
            canonical_id,
            engine: clean_engine,
            role: clean_role,
            host_node: clean_host,
            instance_tag: short_tag,
        }
    }

    /// Resolves a distinct `AgentIdentity` from HTTP request headers, user-agent strings, and session IDs.
    pub fn from_headers_and_session(
        headers: &HeaderMap,
        fallback_node: &str,
        session_id: &str,
        requested_model: Option<&str>,
    ) -> Self {
        let explicit_agent = headers
            .get("x-agent-id")
            .or_else(|| headers.get("x-agent-name"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let explicit_role = headers
            .get("x-agent-role")
            .or_else(|| headers.get("x-subagent-role"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let explicit_host = headers
            .get("x-node-id")
            .or_else(|| headers.get("x-workstation-id"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or(fallback_node);

        let explicit_instance = headers
            .get("x-agent-instance")
            .or_else(|| headers.get("x-client-pid"))
            .or_else(|| headers.get("x-session-id"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or(session_id);

        let user_agent = headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        let engine = if !explicit_agent.is_empty() {
            explicit_agent.to_string()
        } else if user_agent.contains("claude-cli")
            || user_agent.contains("claude-code")
            || user_agent.contains("anthropic")
            || headers.contains_key("anthropic-version")
        {
            "claude-code".to_string()
        } else if user_agent.contains("cursor") {
            "cursor-agent".to_string()
        } else if user_agent.contains("copilot") {
            "copilot-cli".to_string()
        } else if user_agent.contains("python") {
            "python-sdk-agent".to_string()
        } else if let Some(m) = requested_model {
            if !m.is_empty() && m != "unknown" {
                format!("llm-{}", m)
            } else {
                "api-agent".to_string()
            }
        } else {
            "api-agent".to_string()
        };

        let role = if !explicit_role.is_empty() {
            explicit_role
        } else if headers.contains_key("anthropic-version") {
            "terminal"
        } else {
            "main"
        };

        Self::new(&engine, role, explicit_host, explicit_instance)
    }

    /// Formats a distinct Antigravity session or subagent identity so concurrent conversations
    /// and delegated subagents are clearly differentiated in the UI and ledger.
    pub fn for_antigravity_session(conv_id: &str, subagent_role: Option<&str>, target_node: &str) -> String {
        let short_conv = short_instance_fingerprint(conv_id);
        match subagent_role {
            Some(role) if !role.trim().is_empty() => {
                let clean_role = sanitize_token(role, "subagent");
                format!("Antigravity:{}@{}#{}", clean_role, target_node, short_conv)
            }
            _ => format!("Antigravity@{}#{}", target_node, short_conv),
        }
    }

    /// Formats a distinct GitHub Copilot CLI session identity.
    pub fn for_copilot_session(session_id: &str, target_node: &str) -> String {
        let short_sess = short_instance_fingerprint(session_id);
        format!("Copilot-CLI@{}#{}", target_node, short_sess)
    }
}

fn sanitize_token(raw: &str, default_val: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        default_val.to_string()
    } else {
        trimmed.chars().take(28).collect()
    }
}

fn short_instance_fingerprint(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "active_session" {
        return "s0000".to_string();
    }
    let alnum: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if alnum.len() >= 6 {
        alnum[..6].to_lowercase()
    } else if !alnum.is_empty() {
        alnum.to_lowercase()
    } else {
        "s0001".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_distinct_identities_for_different_sessions_and_roles() {
        let id1 = AgentIdentity::for_antigravity_session("b0a75aec-8471-4edf", None, "PC-1");
        let id2 = AgentIdentity::for_antigravity_session("9c5286a5-416f-45b6", Some("image-generator"), "PC-1");
        let id3 = AgentIdentity::for_copilot_session("630605ce-44e5-46df", "PC-1");

        assert_eq!(id1, "Antigravity@PC-1#b0a75a");
        assert_eq!(id2, "Antigravity:image-generator@PC-1#9c5286");
        assert_eq!(id3, "Copilot-CLI@PC-1#630605");
        assert_ne!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_claude_code_header_fingerprinting() {
        let mut headers = HeaderMap::new();
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        headers.insert("user-agent", HeaderValue::from_static("claude-cli/1.0.24"));
        headers.insert("x-client-pid", HeaderValue::from_static("pid9482"));

        let id = AgentIdentity::from_headers_and_session(&headers, "CORP-WS01", "sess-1", Some("claude-3-7-sonnet"));
        assert_eq!(id.engine, "claude-code");
        assert_eq!(id.role, "terminal");
        assert_eq!(id.canonical_id, "claude-code:terminal@CORP-WS01#pid948");
    }
}
