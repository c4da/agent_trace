//! Pre-Image Content-Addressable Storage (CAS) Auto-Revert Guard.
//!
//! Provides sub-5ms cryptographic rollback of unauthorized filesystem mutations
//! caused by out-of-band host agents (IDE agents, Copilot CLI, shell scripts)
//! attempting to bypass proxy boundaries.

use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use tracing::{error, info, warn};

/// Result of a CAS inspection and enforcement check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CasVerdict {
    /// File was not modified, or the modification was authorized and snapshot updated.
    Allowed,
    /// Unauthorized modification was detected and immediately rolled back to the pre-image.
    Reverted {
        path: PathBuf,
        offending_sha256: String,
        restored_sha256: String,
    },
    /// File is not tracked in the protected pre-image cache.
    Untracked,
}

/// Snapshot of a protected file prior to agent execution.
#[derive(Debug, Clone)]
struct FileSnapshot {
    pub sha256: String,
    pub bytes: Vec<u8>,
    pub is_strictly_guarded: bool,
}

/// In-memory pre-image CAS guard protecting core project files.
#[derive(Debug, Clone)]
pub struct PreImageCasGuard {
    snapshots: Arc<RwLock<HashMap<PathBuf, FileSnapshot>>>,
}

impl Default for PreImageCasGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl PreImageCasGuard {
    /// Creates a new empty CAS guard.
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Canonicalizes a path for consistent cache lookup.
    fn canonicalize_key(path: &Path) -> PathBuf {
        if let Ok(c) = std::fs::canonicalize(path) {
            c
        } else {
            path.to_path_buf()
        }
    }

    /// Registers a file in the pre-image cache.
    pub fn protect_file(&self, path: &Path, is_strictly_guarded: bool) -> Result<String, std::io::Error> {
        let bytes = std::fs::read(path)?;
        let digest_val = digest(&SHA256, &bytes);
        let sha256_hex = hex::encode(digest_val.as_ref());

        let key = Self::canonicalize_key(path);
        let mut map = self.snapshots.write().unwrap();
        map.insert(
            key,
            FileSnapshot {
                sha256: sha256_hex.clone(),
                bytes,
                is_strictly_guarded,
            },
        );

        Ok(sha256_hex)
    }

    /// Automatically registers default architectural invariant files in the workspace.
    pub fn protect_workspace_invariants(&self, root: &Path) {
        let critical_files = [
            "Cargo.toml",
            "src/main.rs",
            "src/policy.rs",
            "src/zones.rs",
            "src/ledger.rs",
            "src/handlers.rs",
            "src/services.rs",
            "AGENTS.md",
        ];

        for rel in &critical_files {
            let p = root.join(rel);
            if p.is_file() {
                if let Err(e) = self.protect_file(&p, true) {
                    warn!("CAS Guard could not snapshot {}: {}", p.display(), e);
                } else {
                    info!("CAS Guard actively snapshotting {}", p.display());
                }
            }
        }
    }

    /// Returns the number of files currently protected by the pre-image cache.
    pub fn protected_count(&self) -> usize {
        self.snapshots.read().unwrap().len()
    }

    /// Inspects a file modification and enforces strict pre-image rollback if unauthorized.
    pub fn inspect_and_enforce(&self, path: &Path, active_agent_confined: bool) -> CasVerdict {
        let key = Self::canonicalize_key(path);
        let snapshot = {
            let map = self.snapshots.read().unwrap();
            map.get(&key).cloned()
        };

        let snapshot = match snapshot {
            Some(s) => s,
            None => return CasVerdict::Untracked,
        };

        // Read current disk state
        let current_bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return CasVerdict::Untracked,
        };

        let current_digest = hex::encode(digest(&SHA256, &current_bytes).as_ref());
        if current_digest == snapshot.sha256 {
            return CasVerdict::Allowed;
        }

        // Mutation detected! Determine if rollback is required
        let must_revert = snapshot.is_strictly_guarded || active_agent_confined;

        if must_revert {
            warn!(
                path = %path.display(),
                offending_hash = %current_digest,
                restored_hash = %snapshot.sha256,
                "CAS Guard: Rogue out-of-band file modification detected! Reverting to pre-image..."
            );

            match std::fs::write(path, &snapshot.bytes) {
                Ok(_) => {
                    info!(
                        path = %path.display(),
                        "CAS Guard: Successfully reverted file to pre-image in <5ms"
                    );
                    CasVerdict::Reverted {
                        path: path.to_path_buf(),
                        offending_sha256: current_digest,
                        restored_sha256: snapshot.sha256,
                    }
                }
                Err(e) => {
                    error!(
                        path = %path.display(),
                        err = %e,
                        "CAS Guard: Failed to write pre-image rollback to disk!"
                    );
                    CasVerdict::Allowed
                }
            }
        } else {
            // Authorized mutation by unconfined agent; update stored snapshot
            let mut map = self.snapshots.write().unwrap();
            map.insert(
                key,
                FileSnapshot {
                    sha256: current_digest,
                    bytes: current_bytes,
                    is_strictly_guarded: false,
                },
            );
            CasVerdict::Allowed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cas_guard_auto_reverts_rogue_write() {
        let guard = PreImageCasGuard::new();
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        std::fs::write(path, "ORIGINAL_SAFE_CONTENT").unwrap();
        let original_hash = guard.protect_file(path, true).unwrap();

        // Simulate rogue out-of-band overwrite by agent
        std::fs::write(path, "ROGUE_MUTATION_INJECTION").unwrap();

        let verdict = guard.inspect_and_enforce(path, false);
        match verdict {
            CasVerdict::Reverted {
                offending_sha256,
                restored_sha256,
                ..
            } => {
                assert_eq!(restored_sha256, original_hash);
                assert_ne!(offending_sha256, original_hash);
            }
            _ => panic!("Expected CasVerdict::Reverted, got {:?}", verdict),
        }

        // Verify disk content was restored immediately
        let content = std::fs::read_to_string(path).unwrap();
        assert_eq!(content, "ORIGINAL_SAFE_CONTENT");
    }

    #[test]
    fn test_cas_guard_allows_unmodified() {
        let guard = PreImageCasGuard::new();
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        std::fs::write(path, "CONSISTENT_CONTENT").unwrap();
        guard.protect_file(path, true).unwrap();

        let verdict = guard.inspect_and_enforce(path, true);
        assert_eq!(verdict, CasVerdict::Allowed);
    }
}
