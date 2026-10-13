//! Workspace filesystem scanner for discovering local Markdown rule files.

use super::types::DiscoveredMarkdownFile;
use std::{fs, path::Path};

/// Scans the workspace root and `docs/` directory for `.md` files so the user can
/// attach existing Markdown rule files with a single click in the UI.
pub fn discover_workspace_markdown_files() -> Vec<DiscoveredMarkdownFile> {
    let mut results = Vec::new();
    let candidate_dirs = [".", "docs"];

    for dir in candidate_dirs {
        let dir_path = Path::new(dir);
        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if ext.eq_ignore_ascii_case("md") {
                            let rel_path = path
                                .to_string_lossy()
                                .trim_start_matches(".\\")
                                .trim_start_matches("./")
                                .replace('\\', "/");
                            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            let raw_text = fs::read_to_string(&path).unwrap_or_default();
                            let title = raw_text
                                .lines()
                                .find(|l| l.trim_start().starts_with('#'))
                                .map(|l| l.trim_start_matches('#').trim().to_string())
                                .unwrap_or_else(|| rel_path.clone());
                            let preview: String = raw_text.chars().take(240).collect();

                            results.push(DiscoveredMarkdownFile {
                                path: rel_path,
                                title,
                                size_bytes,
                                preview,
                            });
                        }
                    }
                }
            }
        }
    }

    results.sort_by(|a, b| a.path.cmp(&b.path));
    results
}
