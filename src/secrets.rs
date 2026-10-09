use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
};
use axum::http::HeaderMap;

#[derive(Clone, Debug)]
pub struct ClusterSecurityConfig {
    pub auth_token: Option<String>,
    pub upstream_allowlist: HashSet<String>,
    pub worker_hermes_key: Option<String>,
    pub key_path: PathBuf,
}

impl ClusterSecurityConfig {
    pub fn load() -> Self {
        let secrets_path = std::env::var("AGENTTRACE_SECRETS")
            .unwrap_or_else(|_| ".secrets/agenttrace.env".to_string());
        
        let mut file_vars = std::collections::HashMap::new();
        if let Ok(content) = fs::read_to_string(&secrets_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || !trimmed.contains('=') {
                    continue;
                }
                let mut parts = trimmed.splitn(2, '=');
                if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                    let key = k.trim().to_string();
                    let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
                    file_vars.insert(key, val);
                }
            }
        }

        let get_var = |name: &str| -> Option<String> {
            std::env::var(name)
                .ok()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| file_vars.get(name).cloned().filter(|s| !s.trim().is_empty()))
        };

        let auth_token = get_var("AGENTTRACE_TOKEN");
        let worker_hermes_key = get_var("WORKER_HERMES_KEY");

        // Parse comma-separated upstream allowlist
        let raw_allowlist = get_var("UPSTREAM_ALLOWLIST").unwrap_or_else(|| {
            "http://127.0.0.1:11434,http://localhost:11434,http://127.0.0.1:8642,http://localhost:8642,http://192.168.1.102:8642".to_string()
        });

        let mut allowlist = HashSet::new();
        for item in raw_allowlist.split(',') {
            let trimmed = item.trim().trim_end_matches('/');
            if !trimmed.is_empty() {
                allowlist.insert(trimmed.to_lowercase());
            }
        }

        let key_path = std::env::var("NODE_KEY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".secrets/node_key.pk8"));

        Self {
            auth_token,
            upstream_allowlist: allowlist,
            worker_hermes_key,
            key_path,
        }
    }

    /// Verifies incoming request authentication against the configured AGENTTRACE_TOKEN.
    /// If no AGENTTRACE_TOKEN is set, authentication is optional (permissive home-lab mode).
    pub fn is_authorized(&self, headers: &HeaderMap) -> bool {
        let expected = match &self.auth_token {
            Some(tok) if !tok.is_empty() => tok,
            _ => return true, // Permissive mode when no token is configured
        };

        if let Some(h) = headers.get("x-agenttrace-token").and_then(|v| v.to_str().ok()) {
            if h.trim() == expected {
                return true;
            }
        }

        if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
            if let Some(bearer) = auth.strip_prefix("Bearer ") {
                if bearer.trim() == expected {
                    return true;
                }
            }
        }

        false
    }

    /// Checks if a requested upstream URL prefix is explicitly allowlisted (prevents SSRF).
    pub fn is_upstream_allowed(&self, target_base: &str) -> bool {
        let normalized = target_base.trim_end_matches('/').to_lowercase();
        if self.upstream_allowlist.contains(&normalized) {
            return true;
        }

        // Also check if any allowlisted entry is a prefix
        for allowed in &self.upstream_allowlist {
            if normalized.starts_with(allowed) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_permissive_auth_when_token_is_none() {
        let cfg = ClusterSecurityConfig {
            auth_token: None,
            upstream_allowlist: HashSet::new(),
            worker_hermes_key: None,
            key_path: PathBuf::from(".secrets/node_key.pk8"),
        };
        let headers = HeaderMap::new();
        assert!(cfg.is_authorized(&headers));
    }

    #[test]
    fn test_bearer_authorization_success_and_failure() {
        let cfg = ClusterSecurityConfig {
            auth_token: Some("secret-token-123".to_string()),
            upstream_allowlist: HashSet::new(),
            worker_hermes_key: None,
            key_path: PathBuf::from(".secrets/node_key.pk8"),
        };

        // Empty headers should fail
        let headers = HeaderMap::new();
        assert!(!cfg.is_authorized(&headers));

        // Wrong Bearer token should fail
        let mut wrong_headers = HeaderMap::new();
        wrong_headers.insert("authorization", HeaderValue::from_static("Bearer wrong-token"));
        assert!(!cfg.is_authorized(&wrong_headers));

        // Correct Bearer token should succeed
        let mut valid_headers = HeaderMap::new();
        valid_headers.insert("authorization", HeaderValue::from_static("Bearer secret-token-123"));
        assert!(cfg.is_authorized(&valid_headers));
    }

    #[test]
    fn test_custom_header_token_success() {
        let cfg = ClusterSecurityConfig {
            auth_token: Some("my-agent-key".to_string()),
            upstream_allowlist: HashSet::new(),
            worker_hermes_key: None,
            key_path: PathBuf::from(".secrets/node_key.pk8"),
        };

        let mut headers = HeaderMap::new();
        headers.insert("x-agenttrace-token", HeaderValue::from_static("my-agent-key"));
        assert!(cfg.is_authorized(&headers));
    }

    #[test]
    fn test_upstream_allowlist_and_ssrf_prevention() {
        let mut allowlist = HashSet::new();
        allowlist.insert("http://127.0.0.1:11434".to_string());
        allowlist.insert("http://192.168.1.102:8642".to_string());

        let cfg = ClusterSecurityConfig {
            auth_token: None,
            upstream_allowlist: allowlist,
            worker_hermes_key: None,
            key_path: PathBuf::from(".secrets/node_key.pk8"),
        };

        // Exact match
        assert!(cfg.is_upstream_allowed("http://127.0.0.1:11434"));
        // Trailing slash normalized
        assert!(cfg.is_upstream_allowed("http://127.0.0.1:11434/"));
        // Upper-case normalized
        assert!(cfg.is_upstream_allowed("HTTP://127.0.0.1:11434"));
        // Worker allowed
        assert!(cfg.is_upstream_allowed("http://192.168.1.102:8642"));

        // Malicious SSRF target blocked
        assert!(!cfg.is_upstream_allowed("http://169.254.169.254/latest/meta-data"));
        assert!(!cfg.is_upstream_allowed("http://evil-attacker.com:8080"));
        assert!(!cfg.is_upstream_allowed("http://192.168.1.200:8000"));
    }
}
