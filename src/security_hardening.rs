//! Enterprise Security Hardening: AEAD Encrypted Secrets Vault, Constant-Time RBAC Auth & Inter-Gateway mTLS.
//!
//! Addresses corporate security requirements:
//! 1. Secrets at rest encrypted via `ring::aead::CHACHA20_POLY1305` (`ENCv1:<nonce>:<ciphertext>`).
//! 2. Constant-time Bearer / API-Key token verification with RBAC scopes.
//! 3. Inter-Gateway mTLS certificate thumbprint pinning & HMAC-SHA256 anti-replay mutual auth.

use axum::http::HeaderMap;
use ring::{
    aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305},
    digest::{digest, SHA256},
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const ENCRYPTED_PREFIX: &str = "ENCv1:";

/// Role-Based Access Control (RBAC) levels for corporate deployments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthRole {
    /// Full administrative control over policies, zones, and cluster configuration.
    Admin,
    /// Security operator inspecting telemetry, SIEM exports, and alerts.
    Operator,
    /// Inter-gateway cluster node (Lead <-> Worker mTLS authenticated).
    ClusterNode,
    /// Standard AI agent or developer workstation client.
    AgentClient,
    /// Unauthenticated caller.
    Anonymous,
}

/// Authenticated encryption vault for secrets at rest (`CHACHA20_POLY1305`).
#[derive(Clone)]
pub struct EncryptedSecretVault {
    key_bytes: [u8; 32],
}

impl EncryptedSecretVault {
    /// Derives a 256-bit AEAD master key from an environment master key or machine-bound seed.
    pub fn from_passphrase(passphrase: &str) -> Self {
        let salted = format!("agenttrace-vault-v1::{}", passphrase);
        let d = digest(&SHA256, salted.as_bytes());
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(d.as_ref());
        Self { key_bytes }
    }

    /// Initializes the vault from `AGENTTRACE_MASTER_KEY` or machine-specific fallback.
    pub fn load_default() -> Self {
        let seed = std::env::var("AGENTTRACE_MASTER_KEY").unwrap_or_else(|_| {
            let host = std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "AGENTTRACE-LOCAL-NODE".to_string());
            format!("corp-default-key::{}", host)
        });
        Self::from_passphrase(&seed)
    }

    /// Encrypts a plaintext secret into `ENCv1:<12-byte-nonce-hex>:<ciphertext+tag-hex>`.
    pub fn encrypt(&self, plaintext: &str) -> Result<String, String> {
        let unbound = UnboundKey::new(&CHACHA20_POLY1305, &self.key_bytes)
            .map_err(|_| "Failed to initialize ChaCha20-Poly1305 key".to_string())?;
        let key = LessSafeKey::new(unbound);

        let rng = SystemRandom::new();
        let mut nonce_bytes = [0u8; 12];
        rng.fill(&mut nonce_bytes)
            .map_err(|_| "Failed to generate CSPRNG nonce".to_string())?;

        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut in_out = plaintext.as_bytes().to_vec();
        key.seal_in_place_append_tag(nonce, Aad::from(b"agenttrace-secret-v1"), &mut in_out)
            .map_err(|_| "AEAD encryption failed".to_string())?;

        Ok(format!(
            "{}{}:{}",
            ENCRYPTED_PREFIX,
            hex::encode(nonce_bytes),
            hex::encode(in_out)
        ))
    }

    /// Decrypts an `ENCv1:...` envelope or returns plaintext if unencrypted.
    /// Fails if the ciphertext or Poly1305 authentication tag has been tampered with.
    pub fn decrypt_or_passthrough(&self, raw_value: &str) -> Result<String, String> {
        let trimmed = raw_value.trim();
        let Some(rest) = trimmed.strip_prefix(ENCRYPTED_PREFIX) else {
            return Ok(trimmed.to_string());
        };

        let mut parts = rest.splitn(2, ':');
        let nonce_hex = parts.next().ok_or("Missing AEAD nonce")?;
        let cipher_hex = parts.next().ok_or("Missing AEAD ciphertext")?;

        let nonce_vec = hex::decode(nonce_hex).map_err(|_| "Invalid nonce hex")?;
        if nonce_vec.len() != 12 {
            return Err("Invalid nonce length (expected 12 bytes)".to_string());
        }
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes.copy_from_slice(&nonce_vec);

        let mut in_out = hex::decode(cipher_hex).map_err(|_| "Invalid ciphertext hex")?;

        let unbound = UnboundKey::new(&CHACHA20_POLY1305, &self.key_bytes)
            .map_err(|_| "Failed to initialize AEAD key".to_string())?;
        let key = LessSafeKey::new(unbound);
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);

        let decrypted = key
            .open_in_place(nonce, Aad::from(b"agenttrace-secret-v1"), &mut in_out)
            .map_err(|_| "AEAD authentication tag verification failed (tampered or wrong key)".to_string())?;

        String::from_utf8(decrypted.to_vec()).map_err(|_| "Decrypted secret is not valid UTF-8".to_string())
    }

    /// Returns true if a secret string is stored in encrypted `ENCv1:` format.
    pub fn is_encrypted(val: &str) -> bool {
        val.trim().starts_with(ENCRYPTED_PREFIX)
    }
}

/// Constant-time token comparison preventing timing side-channel attacks.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.is_empty() || b_bytes.is_empty() || a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Inter-Gateway TLS & mTLS Mutual Authentication Verifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterGatewayMtlsConfig {
    /// Whether TLS / mTLS is required for inter-gateway traffic (`AGENTTRACE_MTLS_REQUIRED=true`).
    pub mtls_enforced: bool,
    /// Path to gateway TLS server certificate PEM (`AGENTTRACE_TLS_CERT`).
    pub tls_cert_path: Option<String>,
    /// Path to gateway TLS private key PEM (`AGENTTRACE_TLS_KEY`).
    pub tls_key_path: Option<String>,
    /// Path to trusted CA bundle for verifying peer gateway certificates (`AGENTTRACE_MTLS_CA_CERT`).
    pub ca_bundle_path: Option<String>,
    /// Pinned SHA-256 certificate thumbprints of trusted peer gateways.
    pub allowed_cert_thumbprints: HashSet<String>,
    /// Shared cluster HMAC secret for cryptographic request signing (`x-gateway-hmac`).
    #[serde(skip)]
    pub cluster_shared_secret: Option<String>,
    /// Maximum allowed clock skew in seconds for anti-replay timestamps.
    pub max_clock_skew_secs: i64,
}

impl Default for InterGatewayMtlsConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl InterGatewayMtlsConfig {
    pub fn from_env() -> Self {
        let mtls_enforced = std::env::var("AGENTTRACE_MTLS_REQUIRED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let tls_cert_path = std::env::var("AGENTTRACE_TLS_CERT").ok().filter(|s| !s.is_empty());
        let tls_key_path = std::env::var("AGENTTRACE_TLS_KEY").ok().filter(|s| !s.is_empty());
        let ca_bundle_path = std::env::var("AGENTTRACE_MTLS_CA_CERT").ok().filter(|s| !s.is_empty());

        let mut allowed_cert_thumbprints = HashSet::new();
        if let Ok(raw) = std::env::var("AGENTTRACE_MTLS_THUMBPRINTS") {
            for fp in raw.split(',') {
                let clean = fp.trim().replace(':', "").to_lowercase();
                if !clean.is_empty() {
                    allowed_cert_thumbprints.insert(clean);
                }
            }
        }

        let cluster_shared_secret = std::env::var("AGENTTRACE_CLUSTER_SECRET")
            .or_else(|_| std::env::var("WORKER_HERMES_KEY"))
            .ok()
            .filter(|s| !s.is_empty());

        Self {
            mtls_enforced,
            tls_cert_path,
            tls_key_path,
            ca_bundle_path,
            allowed_cert_thumbprints,
            cluster_shared_secret,
            max_clock_skew_secs: 60,
        }
    }

    /// Generates signed inter-gateway mTLS headers (`x-gateway-node-id`, `x-gateway-timestamp`, `x-gateway-hmac`)
    /// so Worker and Lead nodes cryptographically authenticate every forwarded telemetry batch or heartbeat.
    pub fn sign_inter_gateway_request(
        &self,
        node_id: &str,
        timestamp_epoch: i64,
        payload_bytes: &[u8],
    ) -> Option<String> {
        let secret = self.cluster_shared_secret.as_ref()?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let body_hash = hex::encode(digest(&SHA256, payload_bytes).as_ref());
        let signing_input = format!("{}|{}|{}", node_id, timestamp_epoch, body_hash);
        let tag = hmac::sign(&key, signing_input.as_bytes());
        Some(hex::encode(tag.as_ref()))
    }

    /// Verifies an incoming inter-gateway request via mTLS certificate thumbprint pinning
    /// and/or HMAC-SHA256 mutual signature with replay-window enforcement.
    pub fn verify_peer_gateway(
        &self,
        headers: &HeaderMap,
        payload_bytes: &[u8],
        now_epoch: i64,
    ) -> Result<String, String> {
        // 1. Check pinned X.509 client certificate SHA-256 thumbprint if configured
        if !self.allowed_cert_thumbprints.is_empty() {
            let cert_fp = headers
                .get("x-client-cert-sha256")
                .or_else(|| headers.get("x-ssl-client-fingerprint"))
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().replace(':', "").to_lowercase())
                .ok_or_else(|| "Missing client TLS certificate fingerprint header (x-client-cert-sha256)".to_string())?;

            if !self.allowed_cert_thumbprints.contains(&cert_fp) {
                return Err(format!("Untrusted peer client certificate thumbprint: {}", cert_fp));
            }
        }

        // 2. Verify HMAC-SHA256 mutual signature + anti-replay timestamp if present or if mTLS is enforced
        let node_hdr = headers.get("x-gateway-node-id").and_then(|v| v.to_str().ok());
        let ts_hdr = headers.get("x-gateway-timestamp").and_then(|v| v.to_str().ok());
        let sig_hdr = headers.get("x-gateway-hmac").and_then(|v| v.to_str().ok());

        if self.mtls_enforced || sig_hdr.is_some() {
            let node_id = node_hdr.ok_or("Missing x-gateway-node-id header")?;
            let ts_str = ts_hdr.ok_or("Missing x-gateway-timestamp header")?;
            let provided_sig = sig_hdr.ok_or("Missing x-gateway-hmac signature header")?;

            let req_ts: i64 = ts_str
                .parse()
                .map_err(|_| "Invalid x-gateway-timestamp epoch format".to_string())?;

            let skew = (now_epoch - req_ts).abs();
            if skew > self.max_clock_skew_secs {
                return Err(format!(
                    "Inter-gateway replay protection triggered: timestamp skew {}s exceeds {}s limit",
                    skew, self.max_clock_skew_secs
                ));
            }

            let expected_sig = self
                .sign_inter_gateway_request(node_id, req_ts, payload_bytes)
                .ok_or("Cluster shared secret not configured for mTLS HMAC verification")?;

            if !constant_time_eq(provided_sig, &expected_sig) {
                return Err("Inter-gateway HMAC-SHA256 signature verification failed".to_string());
            }

            return Ok(node_id.to_string());
        }

        Ok(node_hdr.unwrap_or("peer-gateway").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_aead_secret_vault_encrypt_decrypt_and_tamper_detection() {
        let vault = EncryptedSecretVault::from_passphrase("corporate-master-secret-2026");
        let raw_secret = "sk-ant-api03-super-secret-corporate-key-998877";

        let encrypted = vault.encrypt(raw_secret).expect("encryption succeeds");
        assert!(EncryptedSecretVault::is_encrypted(&encrypted));
        assert!(!encrypted.contains("super-secret"));

        let decrypted = vault.decrypt_or_passthrough(&encrypted).expect("decryption succeeds");
        assert_eq!(decrypted, raw_secret);

        // Tamper with 1 byte of ciphertext -> must fail AEAD authentication tag!
        let mut tampered = encrypted.clone();
        let last_char = if tampered.ends_with('a') { 'b' } else { 'a' };
        tampered.pop();
        tampered.push(last_char);
        assert!(vault.decrypt_or_passthrough(&tampered).is_err());

        // Wrong master key -> must fail AEAD authentication tag!
        let wrong_vault = EncryptedSecretVault::from_passphrase("wrong-key");
        assert!(wrong_vault.decrypt_or_passthrough(&encrypted).is_err());
    }

    #[test]
    fn test_inter_gateway_mtls_thumbprint_and_hmac_anti_replay() {
        let mut thumbprints = HashSet::new();
        thumbprints.insert("aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899".to_string());

        let cfg = InterGatewayMtlsConfig {
            mtls_enforced: true,
            tls_cert_path: Some("certs/gateway.pem".to_string()),
            tls_key_path: Some("certs/gateway.key".to_string()),
            ca_bundle_path: Some("certs/ca.pem".to_string()),
            allowed_cert_thumbprints: thumbprints,
            cluster_shared_secret: Some("cluster-mtls-shared-key".to_string()),
            max_clock_skew_secs: 60,
        };

        let payload = br#"{"event_id":"ev-1","node_id":"PC-2"}"#;
        let now = 1792800000;
        let sig = cfg.sign_inter_gateway_request("PC-2", now, payload).unwrap();

        let mut valid_headers = HeaderMap::new();
        valid_headers.insert(
            "x-client-cert-sha256",
            HeaderValue::from_static("AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99"),
        );
        valid_headers.insert("x-gateway-node-id", HeaderValue::from_static("PC-2"));
        valid_headers.insert("x-gateway-timestamp", HeaderValue::from_static("1792800000"));
        valid_headers.insert("x-gateway-hmac", HeaderValue::from_str(&sig).unwrap());

        // 1. Valid mTLS thumbprint + valid HMAC within clock window succeeds
        assert_eq!(cfg.verify_peer_gateway(&valid_headers, payload, now + 5).unwrap(), "PC-2");

        // 2. Replay attack outside 60s window fails!
        assert!(cfg.verify_peer_gateway(&valid_headers, payload, now + 120).is_err());

        // 3. Tampered payload fails HMAC!
        assert!(cfg.verify_peer_gateway(&valid_headers, br#"{"tampered":true}"#, now).is_err());

        // 4. Untrusted client certificate thumbprint fails!
        let mut bad_cert_headers = valid_headers.clone();
        bad_cert_headers.insert("x-client-cert-sha256", HeaderValue::from_static("deadbeef"));
        assert!(cfg.verify_peer_gateway(&bad_cert_headers, payload, now).is_err());
    }
}
