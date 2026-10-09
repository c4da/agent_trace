use crate::schema::{EventType, SecurityVerdict, TraceEvent};
use ring::{
    digest::{Context, SHA256},
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519},
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub const GENESIS_PREV_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerVerificationReport {
    pub is_valid: bool,
    pub total_blocks: usize,
    pub genesis_hash: String,
    pub latest_block_hash: String,
    pub broken_block_id: Option<String>,
    pub error_reason: Option<String>,
}

pub struct NodeIdentity {
    pub key_pair: Ed25519KeyPair,
    pub public_key_hex: String,
}

impl NodeIdentity {
    pub fn load_or_generate(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let p = path.as_ref();
        let is_valid_file = p.exists() && fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false);
        let pkcs8_bytes = if is_valid_file {
            fs::read(p)?
        } else {
            let rng = SystemRandom::new();
            let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
                .map_err(|e| format!("Failed to generate Ed25519 keypair: {:?}", e))?;
            let bytes = pkcs8.as_ref().to_vec();
            if let Some(parent) = p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(p, &bytes)?;
            bytes
        };

        let key_pair = Ed25519KeyPair::from_pkcs8(&pkcs8_bytes)
            .map_err(|e| format!("Failed to parse Ed25519 keypair: {:?}", e))?;
        let public_key_hex = hex::encode(key_pair.public_key().as_ref());

        Ok(Self {
            key_pair,
            public_key_hex,
        })
    }

    pub fn public_key_hex(&self) -> &str {
        &self.public_key_hex
    }

    pub fn sign_event(&self, event_hash: &str) -> String {
        let sig = self.key_pair.sign(event_hash.as_bytes());
        hex::encode(sig.as_ref())
    }

    pub fn verify_signature(event_hash: &str, signature_hex: &str, public_key_hex: &str) -> bool {
        let pk_bytes = match hex::decode(public_key_hex) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let sig_bytes = match hex::decode(signature_hex) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let peer_public_key = UnparsedPublicKey::new(&ED25519, pk_bytes);
        peer_public_key.verify(event_hash.as_bytes(), &sig_bytes).is_ok()
    }
}

/// Canonical length-prefixed field hashing (V2) from raw field strings.
/// Prevents JSON float/key-order re-serialization drift when verifying from database storage.
pub fn compute_event_hash_v2_raw(
    prev_hash: &str,
    event_id: &str,
    node_id: &str,
    session_id: &str,
    timestamp: &str,
    event_type: &EventType,
    target: &str,
    details_str: &str,
    diff: Option<&str>,
    sources_str: &str,
    security_verdict: &SecurityVerdict,
    reasons_str: &str,
    duration_ms: f64,
) -> String {
    let mut context = Context::new(&SHA256);
    context.update(b"AGENTTRACE-V2\n");

    let mut feed_field = |val: &[u8]| {
        let len = (val.len() as u32).to_be_bytes();
        context.update(&len);
        context.update(val);
    };

    feed_field(prev_hash.as_bytes());
    feed_field(event_id.as_bytes());
    feed_field(node_id.as_bytes());
    feed_field(session_id.as_bytes());
    feed_field(timestamp.as_bytes());
    feed_field(format!("{:?}", event_type).as_bytes());
    feed_field(target.as_bytes());
    feed_field(details_str.as_bytes());
    feed_field(diff.unwrap_or("").as_bytes());
    feed_field(sources_str.as_bytes());
    feed_field(format!("{:?}", security_verdict).as_bytes());
    feed_field(reasons_str.as_bytes());

    let duration_bytes = duration_ms.to_bits().to_be_bytes();
    feed_field(&duration_bytes);

    let digest = context.finish();
    hex::encode(digest.as_ref())
}

/// Canonical length-prefixed field hashing (V2).
/// Securely binds EVERY field into the block digest, including event_id, security_verdict,
/// security_reasons, duration_ms, and provenance_sources.
pub fn compute_event_hash_v2(event: &TraceEvent) -> String {
    let details_str = serde_json::to_string(&event.action_details).unwrap_or_default();
    let sources_str = serde_json::to_string(&event.provenance_sources).unwrap_or_default();
    let reasons_str = serde_json::to_string(&event.security_reasons).unwrap_or_default();
    compute_event_hash_v2_raw(
        &event.prev_hash,
        &event.event_id,
        &event.node_id,
        &event.session_id,
        &event.timestamp,
        &event.event_type,
        &event.target,
        &details_str,
        event.diff.as_deref(),
        &sources_str,
        &event.security_verdict,
        &reasons_str,
        event.duration_ms,
    )
}


/// Legacy V1 hash computation (for backwards compatibility with existing historical blocks).
pub fn compute_event_hash_v1(event: &TraceEvent) -> String {
    let mut context = Context::new(&SHA256);
    context.update(event.prev_hash.as_bytes());
    context.update(b"|");
    context.update(event.node_id.as_bytes());
    context.update(b"|");
    context.update(event.session_id.as_bytes());
    context.update(b"|");
    context.update(event.timestamp.as_bytes());
    context.update(b"|");
    context.update(format!("{:?}", event.event_type).as_bytes());
    context.update(b"|");
    context.update(event.target.as_bytes());
    context.update(b"|");
    let details_str = serde_json::to_string(&event.action_details).unwrap_or_default();
    context.update(details_str.as_bytes());
    context.update(b"|");
    if let Some(ref d) = event.diff {
        context.update(d.as_bytes());
    }
    let digest = context.finish();
    hex::encode(digest.as_ref())
}

/// Default canonical event hash (V2).
#[allow(dead_code)]
pub fn compute_event_hash(event: &TraceEvent) -> String {
    compute_event_hash_v2(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_genesis_prev_hash_length() {
        assert_eq!(GENESIS_PREV_HASH.len(), 64);
        assert_eq!(GENESIS_PREV_HASH, "0".repeat(64));
    }

    #[test]
    fn test_node_identity_generate_sign_and_verify() {
        let temp_dir = std::env::temp_dir();
        let key_path = temp_dir.join(format!("test_key_{}.pk8", uuid::Uuid::new_v4()));

        let identity = NodeIdentity::load_or_generate(&key_path).expect("Failed to generate identity");
        assert_eq!(identity.public_key_hex().len(), 64); // 32 bytes hex encoded = 64 hex chars

        let message_hash = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
        let signature = identity.sign_event(message_hash);
        assert_eq!(signature.len(), 128); // 64 bytes hex encoded = 128 hex chars

        // Valid verification
        assert!(NodeIdentity::verify_signature(message_hash, &signature, identity.public_key_hex()));

        // Tampered message hash fails verification
        let tampered_hash = "1111111234567890abcdef1234567890abcdef1234567890abcdef1234567890";
        assert!(!NodeIdentity::verify_signature(tampered_hash, &signature, identity.public_key_hex()));

        // Tampered signature fails verification
        let tampered_sig = format!("00{}", &signature[2..]);
        assert!(!NodeIdentity::verify_signature(message_hash, &tampered_sig, identity.public_key_hex()));

        // Cleanup
        let _ = std::fs::remove_file(key_path);
    }

    #[test]
    fn test_compute_event_hash_v2_determinism_and_sensitivity() {
        let event = TraceEvent {
            event_id: "evt-001".to_string(),
            node_id: "PC-1".to_string(),
            session_id: "sess-01".to_string(),
            timestamp: "2026-10-09T10:00:00Z".to_string(),
            event_type: EventType::FileWrite,
            target: "C:/test.txt".to_string(),
            action_details: json!({"bytes": 42}),
            diff: Some("+hello world".to_string()),
            provenance_sources: vec!["tool_call".to_string()],
            security_verdict: SecurityVerdict::Allowed,
            security_reasons: vec!["within sandbox".to_string()],
            duration_ms: 12.5,
            prev_hash: GENESIS_PREV_HASH.to_string(),
            event_hash: String::new(),
            signature: None,
            public_key: None,
        };

        let hash1 = compute_event_hash_v2(&event);
        let hash2 = compute_event_hash_v2(&event);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);

        // Mutating a field must produce a totally different hash
        let mut tampered = event.clone();
        tampered.target = "C:/hacked.txt".to_string();
        let hash_tampered = compute_event_hash_v2(&tampered);
        assert_ne!(hash1, hash_tampered);
    }
}
