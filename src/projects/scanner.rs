//! Project and Workspace discovery scanner for C:\cada\projects\ and local repos.

use super::types::ProjectRecord;
use std::{fs, path::Path};

/// Scans the default projects root (C:\cada\projects\) and the current working directory.
pub fn discover_projects() -> Vec<ProjectRecord> {
    let mut projects = Vec::new();

    // 1. Scan C:\cada\projects if it exists
    let default_root = Path::new("C:\\cada\\projects");
    if default_root.exists() {
        scan_projects_dir(default_root, &mut projects);
    }

    // 2. Fallback: scan current directory parent if default_root wasn't found or was empty
    if projects.is_empty() {
        if let Ok(cur) = std::env::current_dir() {
            if let Some(parent) = cur.parent() {
                scan_projects_dir(parent, &mut projects);
            }
        }
    }

    // Sort alphabetically by name
    projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    projects
}

fn scan_projects_dir(root: &Path, projects: &mut Vec<ProjectRecord>) {
    let Ok(entries) = fs::read_dir(root) else { return; };
    let cur_dir_name = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_default();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if fname.starts_with('.') || fname == "target" || fname == "node_modules" {
            continue;
        }

        let tech = infer_tech_stack(&path);
        let branch = read_git_branch(&path);
        let is_active = fname.eq_ignore_ascii_case(&cur_dir_name) || fname == "agent_trace_gw";

        projects.push(ProjectRecord {
            id: fname.to_lowercase().replace(' ', "-"),
            name: fname.to_string(),
            path: path.to_string_lossy().to_string(),
            is_active,
            tech_stack: tech,
            conversation_count: 0,
            git_branch: branch,
        });
    }
}

pub(crate) fn infer_tech_stack(dir: &Path) -> String {
    if dir.join("Cargo.toml").exists() {
        if dir.join("src/fem").exists() || dir.join("crates/strux_math").exists() {
            "Rust / Bevy CAD/FEM".to_string()
        } else {
            "Rust / Cargo".to_string()
        }
    } else if dir.join("package.json").exists() {
        "TypeScript / Node.js".to_string()
    } else if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() {
        "Python Standard Library".to_string()
    } else if dir.join("Makefile").exists() || dir.join("CMakeLists.txt").exists() {
        "C/C++ Systems".to_string()
    } else {
        "Software Workspace".to_string()
    }
}

pub(crate) fn read_git_branch(dir: &Path) -> Option<String> {

    let head_path = dir.join(".git").join("HEAD");
    if let Ok(head_str) = fs::read_to_string(head_path) {
        let trimmed = head_str.trim();
        if trimmed.starts_with("ref: refs/heads/") {
            return Some(trimmed.trim_start_matches("ref: refs/heads/").to_string());
        }
        if trimmed.len() >= 7 {
            return Some(trimmed[..7].to_string());
        }
    }
    None
}
