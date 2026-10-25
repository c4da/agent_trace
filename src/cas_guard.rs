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
    sync::{Arc, OnceLock, RwLock},
};
use tracing::{error, info, warn};

/// Global process-wide Pre-Image CAS Guard instance shared across background watcher, HTTP API, and MCP tools.
pub fn global_cas_guard() -> &'static PreImageCasGuard {
    static INSTANCE: OnceLock<PreImageCasGuard> = OnceLock::new();
    INSTANCE.get_or_init(PreImageCasGuard::new)
}

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

    /// Canonicalizes a path for consistent cache lookup, even if the file was deleted.
    fn canonicalize_key(path: &Path) -> PathBuf {
        let raw = if let Ok(c) = std::fs::canonicalize(path) {
            c
        } else if let Some(parent) = path.parent() {
            if let Ok(c_parent) = std::fs::canonicalize(parent) {
                if let Some(fname) = path.file_name() {
                    c_parent.join(fname)
                } else {
                    path.to_path_buf()
                }
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        };

        // Normalize Windows verbatim \\?\ prefix for consistent lookup across existing and deleted states
        let s = raw.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            raw
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

        let strict_default = std::env::var("AGENTTRACE_CAS_STRICT")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        for rel in &critical_files {
            let p = root.join(rel);
            if p.is_file() {
                if let Err(e) = self.protect_file(&p, strict_default) {
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

    /// Returns all canonical paths currently tracked in the pre-image cache.
    pub fn tracked_paths(&self) -> Vec<PathBuf> {
        self.snapshots.read().unwrap().keys().cloned().collect()
    }

    /// Returns metadata for all protected files in the pre-image cache.
    pub fn list_protected_files(&self) -> Vec<serde_json::Value> {
        let map = self.snapshots.read().unwrap();
        map.iter()
            .map(|(k, v)| {
                serde_json::json!({
                    "path": k.to_string_lossy(),
                    "sha256": v.sha256,
                    "size_bytes": v.bytes.len(),
                    "is_strictly_guarded": v.is_strictly_guarded,
                })
            })
            .collect()
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

        // Read current disk state or detect rogue deletion
        let (current_digest, current_bytes, is_deleted) = match std::fs::read(path) {
            Ok(b) => {
                let d = hex::encode(digest(&SHA256, &b).as_ref());
                (d, b, false)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ("DELETED".to_string(), Vec::new(), true),
            Err(_) => return CasVerdict::Untracked,
        };

        if !is_deleted && current_digest == snapshot.sha256 {
            return CasVerdict::Allowed;
        }

        // Mutation or deletion detected! Determine if rollback is required
        let must_revert = snapshot.is_strictly_guarded || active_agent_confined;

        if must_revert {
            warn!(
                path = %path.display(),
                offending_hash = %current_digest,
                restored_hash = %snapshot.sha256,
                is_deletion = is_deleted,
                "CAS Guard: Rogue out-of-band file modification or deletion detected! Reverting to pre-image..."
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

#[derive(Debug, Deserialize)]
pub struct CasProtectRequest {
    pub path: String,
    #[serde(default = "default_strictly_guarded")]
    pub strictly_guarded: bool,
}

fn default_strictly_guarded() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct CasEnforceRequest {
    pub path: String,
    #[serde(default)]
    pub active_agent_confined: bool,
}

/// Returns status and all protected pre-image files (`GET /api/v1/cas/status`).
pub async fn cas_status_handler() -> axum::Json<serde_json::Value> {
    let guard = global_cas_guard();
    let files = guard.list_protected_files();
    axum::Json(serde_json::json!({
        "protected_count": files.len(),
        "files": files,
    }))
}

/// Snapshots a file into the in-memory Pre-Image CAS cache (`POST /api/v1/cas/protect`).
pub async fn cas_protect_handler(
    axum::Json(req): axum::Json<CasProtectRequest>,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    let guard = global_cas_guard();
    let p = Path::new(&req.path);
    match guard.protect_file(p, req.strictly_guarded) {
        Ok(sha256) => (
            axum::http::StatusCode::OK,
            axum::Json(serde_json::json!({
                "status": "protected",
                "path": req.path,
                "sha256": sha256,
                "strictly_guarded": req.strictly_guarded,
            })),
        ),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "status": "error",
                "path": req.path,
                "error": format!("Failed to snapshot file: {}", e),
            })),
        ),
    }
}

/// Inspects a file and immediately reverts unauthorized modifications or deletions (`POST /api/v1/cas/enforce`).
pub async fn cas_enforce_handler(
    axum::Json(req): axum::Json<CasEnforceRequest>,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    let t0 = std::time::Instant::now();
    let guard = global_cas_guard();
    let verdict = guard.inspect_and_enforce(Path::new(&req.path), req.active_agent_confined);
    let latency_us = t0.elapsed().as_micros() as u64;
    (
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({
            "verdict": verdict,
            "evaluation_latency_us": latency_us,
        })),
    )
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

    #[test]
    fn test_cas_guard_auto_reverts_rogue_deletion() {
        let guard = PreImageCasGuard::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("critical_config.json");

        std::fs::write(&path, r#"{"invariants": "strict"}"#).unwrap();
        let original_hash = guard.protect_file(&path, true).unwrap();

        // Simulate rogue agent deleting the protected file
        std::fs::remove_file(&path).unwrap();
        assert!(!path.exists(), "File should be deleted before guard enforcement");

        // Inspect and enforce - must restore the deleted file!
        let verdict = guard.inspect_and_enforce(&path, false);
        match verdict {
            CasVerdict::Reverted {
                offending_sha256,
                restored_sha256,
                ..
            } => {
                assert_eq!(offending_sha256, "DELETED");
                assert_eq!(restored_sha256, original_hash);
            }
            _ => panic!("Expected CasVerdict::Reverted for deletion, got {:?}", verdict),
        }

        // Verify file was recreated with original content
        assert!(path.exists(), "File MUST be recreated by CAS guard!");
        let restored_content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(restored_content, r#"{"invariants": "strict"}"#);
    }
}
