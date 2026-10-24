//! Scanner for discovering artifacts across the workspace and local Antigravity brain.

use super::types::{ArtifactItem, ArtifactKind};
use chrono::{DateTime, Utc};
use std::{fs, path::Path};

/// Discovers all available artifacts in the workspace and brain folders.
pub fn scan_artifacts() -> Vec<ArtifactItem> {
    let mut items = Vec::new();

    // 1. Scan workspace docs/ and docs/assets/
    scan_dir(Path::new("docs"), "docs", &mut items, 2);
    scan_dir(Path::new("artifacts"), "artifacts", &mut items, 2);

    // 2. Scan Antigravity Brain artifacts if available
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let brain_root = Path::new(&userprofile)
            .join(".gemini")
            .join("antigravity")
            .join("brain");
        if brain_root.exists() {
            scan_brain_dir(&brain_root, &mut items);
        }
    }

    // Sort newest modified first
    items.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    items
}

fn scan_brain_dir(root: &Path, items: &mut Vec<ArtifactItem>) {
    let Ok(entries) = fs::read_dir(root) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Each conversation subfolder
            let conv_id = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let Ok(sub_entries) = fs::read_dir(&path) else { continue; };
            for sub_entry in sub_entries.flatten() {
                let sub_path = sub_entry.path();
                if sub_path.is_file() {
                    let rel_tag = format!("brain/{}/{}", conv_id, sub_path.file_name().unwrap_or_default().to_string_lossy());
                    if let Some(item) = inspect_artifact_file(&sub_path, &rel_tag) {
                        items.push(item);
                    }
                }
            }
        }
    }
}

fn scan_dir(dir: &Path, prefix: &str, items: &mut Vec<ArtifactItem>, max_depth: usize) {
    if max_depth == 0 || !dir.exists() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if fname.starts_with('.') || fname == "target" || fname == "node_modules" {
            continue;
        }
        if path.is_dir() {
            let next_prefix = format!("{}/{}", prefix, fname);
            scan_dir(&path, &next_prefix, items, max_depth - 1);
        } else if path.is_file() {
            let rel = format!("{}/{}", prefix, fname);
            if let Some(item) = inspect_artifact_file(&path, &rel) {
                items.push(item);
            }
        }
    }
}

fn inspect_artifact_file(path: &Path, relative_path: &str) -> Option<ArtifactItem> {
    let fname = path.file_name()?.to_str()?;
    if fname.starts_with('.') || fname.ends_with(".log") || fname.ends_with(".db") {
        return None;
    }

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let kind = ArtifactKind::from_extension(ext);
    if kind == ArtifactKind::Unknown {
        return None;
    }

    let meta = fs::metadata(path).ok()?;
    let size_bytes = meta.len();

    let modified_at = meta
        .modified()
        .map(|t| {
            let dt: DateTime<Utc> = t.into();
            dt.to_rfc3339()
        })
        .unwrap_or_else(|_| Utc::now().to_rfc3339());

    let (title, preview) = match kind {
        ArtifactKind::Markdown => {
            let content = fs::read_to_string(path).unwrap_or_default();
            let title = extract_title(&content, fname);
            let prev = content.chars().take(240).collect::<String>();
            (title, prev)
        }
        ArtifactKind::Image => {
            let clean_name = fname.replace(['-', '_'], " ");
            (clean_name, format!("Visual Image Asset ({} KB)", size_bytes / 1024))
        }
        _ => {
            let content = fs::read_to_string(path).unwrap_or_default();
            let prev = content.chars().take(240).collect::<String>();
            (fname.to_string(), prev)
        }
    };

    let id = format!("{:x}", md5_or_simple_hash(relative_path));
    let mime = kind.mime_type(ext).to_string();

    Some(ArtifactItem {
        id,
        title,
        kind,
        file_path: path.to_string_lossy().to_string(),
        relative_path: relative_path.to_string(),
        size_bytes,
        modified_at,
        preview,
        mime_type: mime,
    })
}

fn extract_title(content: &str, fallback: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return trimmed.trim_start_matches("# ").trim().to_string();
        }
    }
    fallback.replace(".md", "").replace(['-', '_'], " ")
}

fn md5_or_simple_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h = h ^ (*b as u64);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
