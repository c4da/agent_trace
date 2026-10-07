use crate::schema::TraceEvent;
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
    pub fn load_or_generate(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let p = Path::new(path);
        let pkcs8_bytes = if p.exists() {
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

pub fn compute_event_hash(event: &TraceEvent) -> String {
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
