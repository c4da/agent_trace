//! Deep Repository Grounding Engine.
//!
//! Connects the active Workspace/Repository to the LLM Context:
//! 1. Resolves repository root path on disk (e.g. `C:\cada\projects\Strux`).
//! 2. Extracts real architecture and directory hierarchy.
//! 3. Scans for project roadmap/delegation specs (`CAD_TOOLS_DELEGATION.md`, `HANDOVER.md`, `TODO.md`, `ROADMAP.md`).
//! 4. Scans codebase files (`src/`, `crates/`) for active `TODO:`, `FIXME:`, `unimplemented!()`, `todo!()` markers.
//! 5. Compiles a concise, authoritative Grounding Block injected into the dispatch system prompt,
//!    guaranteeing zero hallucination and ground-truth CAD/FEM/systems domain awareness.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// Builds an authoritative ground-truth context snapshot for a given project.
pub async fn build_repository_context(
    project_id: Option<&str>,
    project_path: Option<&str>,
) -> Option<String> {
    let mut resolved_path: Option<PathBuf> = None;
    let mut project_name = "Active Project".to_string();

    if let Some(p) = project_path {
        let pb = PathBuf::from(p);
        if pb.exists() {
            resolved_path = Some(pb);
        }
    }

    if resolved_path.is_none() {
        if let Some(pid) = project_id {
            // Check global project store
            let mgr = crate::projects::global_project_store();
            let guard = mgr.read().await;
            if let Some(prec) = guard.get_project_by_id_or_name(pid) {
                let pb = PathBuf::from(&prec.path);
                if pb.exists() {
                    project_name = prec.name;
                    resolved_path = Some(pb);
                }
            } else {
                // Fallback direct disk check under C:\cada\projects\{pid}
                let cand = PathBuf::from("C:\\cada\\projects").join(pid);
                if cand.exists() {
                    project_name = pid.to_string();
                    resolved_path = Some(cand);
                }
            }
        }
    }

    let root = resolved_path?;
    let mut context_parts: Vec<String> = Vec::new();

    // 1. Header & Tech Stack
    let tech_stack = crate::projects::scanner::infer_tech_stack(&root);
    let git_branch =
        crate::projects::scanner::read_git_branch(&root).unwrap_or_else(|| "main".to_string());

    context_parts.push(format!(
        "### [AUTHORITATIVE GROUND-TRUTH REPOSITORY CONTEXT: {}]\n- Root: {}\n- Tech Stack: {}\n- Branch: {}",
        project_name,
        root.display(),
        tech_stack,
        git_branch
    ));

    // 2. Scan Key Architecture & Modules
    let mut modules = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !fname.starts_with('.') && fname != "target" && fname != "node_modules" {
                    if fname == "src" || fname == "crates" {
                        if let Ok(subs) = fs::read_dir(&p) {
                            for sub in subs.flatten() {
                                let sname = sub.file_name().to_string_lossy().to_string();
                                if !sname.starts_with('.') {
                                    modules.push(format!("{}/{}", fname, sname));
                                }
                            }
                        }
                    } else {
                        modules.push(fname.to_string());
                    }
                }
            }
        }
    }
    if !modules.is_empty() {
        context_parts.push(format!("- Modules & Directory Structure: {}", modules.join(", ")));
    }

    // 3. Scan Known Project Documentation (CAD_TOOLS_DELEGATION.md, HANDOVER.md, ROADMAP.md, TODO.md)
    let doc_candidates = [
        "CAD_TOOLS_DELEGATION.md",
        "HANDOVER.md",
        "KINEMATICS_AND_SUSPENSION_ROADMAP.md",
        "TODO.md",
        "ROADMAP.md",
        "PROJECT.md",
    ];

    let mut doc_summaries = Vec::new();
    for doc in doc_candidates {
        let doc_path = root.join(doc);
        if doc_path.exists() {
            if let Ok(content) = fs::read_to_string(&doc_path) {
                let mut lines_summary = Vec::new();
                for line in content.lines().take(50) {
                    let trimmed = line.trim();
                    if (trimmed.starts_with('#')
                        || trimmed.starts_with("1.")
                        || trimmed.starts_with("2.")
                        || trimmed.starts_with("3.")
                        || trimmed.starts_with("4.")
                        || trimmed.starts_with('-'))
                        && !trimmed.is_empty()
                    {
                        lines_summary.push(trimmed.to_string());
                        if lines_summary.len() >= 8 {
                            break;
                        }
                    }
                }
                if !lines_summary.is_empty() {
                    doc_summaries.push(format!("• [{}]\n  {}", doc, lines_summary.join("\n  ")));
                }
            }
        }
    }

    if !doc_summaries.is_empty() {
        context_parts.push(format!(
            "\n### Documented Roadmap & Active Unfinished Delegations:\n{}",
            doc_summaries.join("\n\n")
        ));
    }

    // 4. Scan for Code Markers (TODO, FIXME, todo!(), unimplemented!()) in src/ and crates/
    let mut code_todos = Vec::new();
    scan_todos_in_dir(&root.join("src"), &mut code_todos, 8);
    if code_todos.len() < 8 {
        scan_todos_in_dir(&root.join("crates"), &mut code_todos, 8);
    }

    if !code_todos.is_empty() {
        context_parts.push(format!(
            "\n### Codebase Unfinished Work Markers (TODO / todo!() / unimplemented!()):\n{}",
            code_todos.join("\n")
        ));
    }

    Some(context_parts.join("\n"))
}

fn scan_todos_in_dir(dir: &Path, todos: &mut Vec<String>, max_items: usize) {
    if !dir.exists() || !dir.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if todos.len() >= max_items {
            break;
        }
        let p = entry.path();
        if p.is_dir() {
            let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !fname.starts_with('.') && fname != "target" {
                scan_todos_in_dir(&p, todos, max_items);
            }
        } else if p.is_file() {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "rs" || ext == "py" || ext == "ts" || ext == "js" {
                if let Ok(content) = fs::read_to_string(&p) {
                    for (line_num, line) in content.lines().enumerate() {
                        let trimmed = line.trim();
                        if trimmed.contains("todo!(")
                            || trimmed.contains("unimplemented!(")
                            || trimmed.contains("TODO:")
                            || trimmed.contains("FIXME:")
                        {
                            let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                            let cleaned = trimmed.chars().take(80).collect::<String>();
                            todos.push(format!("• {}:{} → {}", fname, line_num + 1, cleaned));
                            if todos.len() >= max_items {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}
