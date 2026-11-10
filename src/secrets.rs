use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
};
use axum::http::HeaderMap;

/// Cookie used by the browser UI to carry the gateway token.
pub const AUTH_COOKIE_NAME: &str = "agenttrace_token";
const AUTH_COOKIE_PREFIX: &str = "agenttrace_token=";

#[derive(Clone, Debug)]
pub struct ClusterSecurityConfig {
    pub require_auth: bool,
    pub auth_token: Option<String>,
    pub upstream_allowlist: HashSet<String>,
    pub worker_hermes_key: Option<String>,
    pub key_path: PathBuf,
}

impl ClusterSecurityConfig {
    pub fn load() -> Self {
        let secrets_path = std::env::var("AGENTTRACE_SECRETS")
            .unwrap_or_else(|_| ".secrets/agenttrace.env".to_string());
        
        let vault = crate::security_hardening::EncryptedSecretVault::load_default();
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
                    let raw_val = v.trim().trim_matches('"').trim_matches('\'').to_string();
                    // Secrets are stored in plaintext by default; only decrypt if user explicitly encrypted with ENCv1:
                    let val = if crate::security_hardening::EncryptedSecretVault::is_encrypted(&raw_val) {
                        vault.decrypt_or_passthrough(&raw_val).unwrap_or(raw_val)
                    } else {
                        raw_val
                    };
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

        // A configured AGENTTRACE_TOKEN is always enforced. AGENTTRACE_REQUIRE_AUTH=true
        // additionally fails closed when no token is configured.
        let auth_token = get_var("AGENTTRACE_TOKEN");
        let require_auth = auth_token.is_some()
            || get_var("AGENTTRACE_REQUIRE_AUTH")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
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
            require_auth,
            auth_token,
            upstream_allowlist: allowlist,
            worker_hermes_key,
            key_path,
        }
    }

    /// Verifies incoming request authentication against the configured AGENTTRACE_TOKEN using constant-time comparison.
    /// Fails closed if AGENTTRACE_REQUIRE_AUTH is set but AGENTTRACE_TOKEN is missing or empty.
    /// If require_auth is false and no AGENTTRACE_TOKEN is set, permits requests (permissive local mode).
    pub fn is_authorized(&self, headers: &HeaderMap) -> bool {
        if !self.require_auth && self.auth_token.is_none() {
            return true; // Explicitly unauthenticated local mode
        }

        let expected = match &self.auth_token {
            Some(tok) if !tok.trim().is_empty() => tok.trim(),
            _ => return false, // FAIL-CLOSED: auth required but token is missing or empty!
        };

        for hdr_name in ["x-agenttrace-token", "x-api-key"] {
            if let Some(h) = headers.get(hdr_name).and_then(|v| v.to_str().ok()) {
                if crate::security_hardening::constant_time_eq(h.trim(), expected) {
                    return true;
                }
            }
        }

        if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
            if let Some(bearer) = auth.strip_prefix("Bearer ") {
                if crate::security_hardening::constant_time_eq(bearer.trim(), expected) {
                    return true;
                }
            }
        }

        // Browser UI: SameSite=Strict cookie set by GET /login?token=...
        for cookie_hdr in headers.get_all("cookie").iter().filter_map(|v| v.to_str().ok()) {
            for pair in cookie_hdr.split(';') {
                if let Some(val) = pair.trim().strip_prefix(AUTH_COOKIE_PREFIX) {
                    if crate::security_hardening::constant_time_eq(val.trim(), expected) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Checks if a requested upstream URL matches an allowlisted origin (prevents SSRF and prefix attacks).
    pub fn is_upstream_allowed(&self, target_base: &str) -> bool {
        let target_url = match url::Url::parse(target_base) {
            Ok(u) => u,
            Err(_) => return false,
        };

        let target_origin = target_url.origin();

        for allowed in &self.upstream_allowlist {
            if let Ok(allowed_url) = url::Url::parse(allowed) {
                if target_origin == allowed_url.origin() {
                    return true;
                }
            } else {
                let normalized = target_base.trim_end_matches('/').to_lowercase();
                if normalized == *allowed {
                    return true;
                }
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
            require_auth: false,
            auth_token: None,
            upstream_allowlist: HashSet::new(),
            worker_hermes_key: None,
            key_path: PathBuf::from(".secrets/node_key.pk8"),
        };
        let headers = HeaderMap::new();
        assert!(cfg.is_authorized(&headers));
    }

    #[test]
    fn test_fail_closed_when_require_auth_true_without_token() {
        let cfg = ClusterSecurityConfig {
            require_auth: true,
            auth_token: None,
            upstream_allowlist: HashSet::new(),
            worker_hermes_key: None,
            key_path: PathBuf::from(".secrets/node_key.pk8"),
        };
        let headers = HeaderMap::new();
        assert!(!cfg.is_authorized(&headers), "Must fail closed if require_auth is true but token is missing");

        let mut empty_token_cfg = cfg.clone();
        empty_token_cfg.auth_token = Some("   ".to_string());
        assert!(!empty_token_cfg.is_authorized(&headers), "Must fail closed if token is whitespace/empty");
    }

    #[test]
    fn test_bearer_authorization_success_and_failure() {
        let cfg = ClusterSecurityConfig {
            require_auth: true,
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
            require_auth: true,
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
            require_auth: false,
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
        // Critical: Prefix collision attack blocked (e.g. 127.0.0.1:11434.evil.com)
        assert!(!cfg.is_upstream_allowed("http://127.0.0.1:11434.evil.com"));
        assert!(!cfg.is_upstream_allowed("http://127.0.0.1:11434.evil.com:8080"));
        assert!(!cfg.is_upstream_allowed("http://127.0.0.1:8080"));
    }
}
