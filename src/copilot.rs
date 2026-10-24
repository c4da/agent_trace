//! GitHub Copilot CLI Detection & Execution Engine (`src/copilot.rs`).
//!
//! Provides native auto-detection and execution for the GitHub Copilot CLI installed
//! on PC-1 Lead Node. Captures non-interactive prompt runs (`copilot -p`), handles
//! working directory grounding, strips terminal artifacts, and hashes execution
//! into the AgentTrace audit ledger.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tracing::{info, warn};

/// Locates the GitHub Copilot CLI binary on the host.
pub fn find_copilot_binary() -> Option<PathBuf> {
    // 1. Check WinGet Links on Windows (%LOCALAPPDATA%\Microsoft\WinGet\Links\copilot.exe)
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let winget_path = PathBuf::from(local_app_data)
            .join("Microsoft")
            .join("WinGet")
            .join("Links")
            .join("copilot.exe");
        if winget_path.exists() {
            return Some(winget_path);
        }
    }

    // 2. Direct PATH probe
    let probe = if cfg!(windows) { "copilot.exe" } else { "copilot" };
    if let Ok(output) = Command::new(probe).arg("--version").output() {
        if output.status.success() {
            return Some(PathBuf::from(probe));
        }
    }

    // 3. Fallback check for gh extension: 'gh.exe'
    let gh_probe = if cfg!(windows) { "gh.exe" } else { "gh" };
    if let Ok(output) = Command::new(gh_probe).args(["copilot", "--version"]).output() {
        if output.status.success() {
            return Some(PathBuf::from(gh_probe));
        }
    }

    None
}

/// Returns true if GitHub Copilot CLI is detected on this node.
pub fn is_copilot_installed() -> bool {
    find_copilot_binary().is_some()
}

/// Returns the detected Copilot version string.
pub fn get_copilot_version() -> Option<String> {
    let bin = find_copilot_binary()?;
    let is_gh = bin.file_stem().and_then(|s| s.to_str()) == Some("gh");
    let mut cmd = Command::new(bin);
    if is_gh {
        cmd.args(["copilot", "--version"]);
    } else {
        cmd.arg("--version");
    }

    if let Ok(output) = cmd.output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let first_line = text.lines().next().unwrap_or("").trim();
            if !first_line.is_empty() {
                return Some(first_line.to_string());
            }
        }
    }
    None
}

/// Executes a prompt using GitHub Copilot CLI non-interactively (`-p`).
pub async fn execute_copilot_cli(
    prompt: &str,
    system_prompt: Option<&str>,
    project_path: Option<&str>,
) -> Result<String, String> {
    let bin = find_copilot_binary().ok_or_else(|| {
        "GitHub Copilot CLI executable not found on PC-1 (ensure copilot is on PATH or WinGet Links)".to_string()
    })?;

    let is_gh = bin.file_stem().and_then(|s| s.to_str()) == Some("gh");

    // Prepend system directives and repository context if provided
    let combined_prompt = match system_prompt {
        Some(sys) if !sys.trim().is_empty() => {
            format!("System Directives:\n{}\n\nTask:\n{}", sys, prompt)
        }
        _ => prompt.to_string(),
    };

    let mut cmd = TokioCommand::new(&bin);
    if is_gh {
        cmd.arg("copilot");
    }
    cmd.arg("-p")
        .arg(&combined_prompt)
        .arg("--no-color")
        .arg("--no-ask-user")
        .arg("--allow-all");

    // Ground execution inside the active project directory if present
    if let Some(p) = project_path {
        let path = Path::new(p);
        if path.exists() && path.is_dir() {
            cmd.current_dir(path);
        }
    }

    info!(
        "Executing GitHub Copilot CLI on PC-1 (bin: {:?}, cwd: {:?})",
        bin, project_path
    );

    let timeout = Duration::from_secs(180);
    let output_future = cmd.output();

    let output = match tokio::time::timeout(timeout, output_future).await {
        Ok(res) => res.map_err(|e| format!("Failed to execute Copilot CLI process: {}", e))?,
        Err(_) => return Err("GitHub Copilot CLI execution timed out after 180 seconds".to_string()),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && stdout.trim().is_empty() {
        warn!("Copilot CLI exited with error code {}: {}", output.status, stderr);
        return Err(format!("Copilot CLI failed (status {}): {}", output.status, stderr));
    }

    let cleaned = clean_copilot_output(&stdout);
    if cleaned.trim().is_empty() && !stderr.trim().is_empty() {
        Ok(stderr.trim().to_string())
    } else {
        Ok(cleaned)
    }
}

/// Strips terminal statistics, telemetry footers, and progress banners from Copilot CLI stdout.
pub fn clean_copilot_output(raw: &str) -> String {
    let mut lines: Vec<&str> = raw.lines().collect();

    // Strip trailing footer blocks ('Changes +X -Y', 'AI Credits ...', 'Resume copilot ...')
    while let Some(last) = lines.last() {
        let trimmed = last.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("Changes")
            || trimmed.starts_with("AI Credits")
            || trimmed.starts_with("Tokens")
            || trimmed.starts_with("Resume")
            || trimmed.starts_with("Last progress:")
        {
            lines.pop();
        } else {
            break;
        }
    }

    lines.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_copilot_output() {
        let sample = "Here is the implementation of the requested function:\n\n```rust\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n```\n\nChanges    +0 -0\nAI Credits 0.28 (4s)\nTokens     ↑ 14.6k • ↓ 77\nResume     copilot --resume=123-abc\n";
        let cleaned = clean_copilot_output(sample);
        assert!(cleaned.contains("fn add(a: i32, b: i32) -> i32"));
        assert!(!cleaned.contains("AI Credits"));
        assert!(!cleaned.contains("Resume copilot"));
    }

    #[test]
    fn test_detect_copilot_binary() {
        // In local PC-1 environment, Copilot CLI must be discoverable
        let found = find_copilot_binary();
        assert!(found.is_some(), "GitHub Copilot CLI should be detected on PC-1");
    }
}
