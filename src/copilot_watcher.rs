use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{Duration, SystemTime};
use tokio::sync::broadcast;
use crate::db::Database;
use crate::schema::{EventType, SecurityVerdict, TraceEvent};

/// Global holder for the most recently active Copilot session ID.
static LATEST_ACTIVE_COPILOT_SESSION: RwLock<Option<String>> = RwLock::new(None);

/// Retrieves the latest session ID observed across Copilot sessions.
#[allow(dead_code)]
pub fn get_latest_copilot_session_id() -> Option<String> {
    LATEST_ACTIVE_COPILOT_SESSION.read().ok().and_then(|guard| guard.clone())
}

/// Sets the latest active Copilot session ID.
pub fn set_latest_copilot_session_id(id: String) {
    if let Ok(mut guard) = LATEST_ACTIVE_COPILOT_SESSION.write() {
        *guard = Some(id);
    }
}

/// State tracking for each watched Copilot session events.jsonl file.
#[derive(Debug)]
struct CopilotSessionWatchState {
    session_id: String,
    last_offset: u64,
    #[allow(dead_code)]
    last_mtime: SystemTime,
}

/// Spawns a background thread that monitors all active GitHub Copilot CLI session transcripts
/// in ~/.copilot/session-state/*/events.jsonl and ingests commands, file edits, and model turns
/// into the tamper-evident ledger.
pub fn spawn_watcher(db: Database, tx_events: broadcast::Sender<String>) {
    std::thread::spawn(move || {
        run_copilot_watcher_loop(db, tx_events);
    });
}

fn get_copilot_session_state_dir() -> Option<PathBuf> {
    let user_profile = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    let session_dir = PathBuf::from(user_profile)
        .join(".copilot")
        .join("session-state");

    if session_dir.exists() {
        Some(session_dir)
    } else {
        None
    }
}

/// Scans for all events.jsonl files under ~/.copilot/session-state/*/events.jsonl.
fn scan_copilot_sessions(base_dir: &PathBuf) -> Vec<(PathBuf, String, SystemTime, u64)> {
    let mut results = Vec::new();

    let entries = match fs::read_dir(base_dir) {
        Ok(e) => e,
        Err(_) => return results,
    };

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(14 * 24 * 3600))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }

        let dir_name = match p.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Skip internal lock/cache directories
        if dir_name.starts_with('.') {
            continue;
        }

        let events_file = p.join("events.jsonl");
        if events_file.exists() {
            if let Ok(meta) = events_file.metadata() {
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                if mtime >= cutoff {
                    results.push((events_file, dir_name, mtime, meta.len()));
                }
            }
        }
    }

    // Sort by most recently modified first
    results.sort_by(|a, b| b.2.cmp(&a.2));
    results
}

fn run_copilot_watcher_loop(db: Database, tx: broadcast::Sender<String>) {
    let session_dir = match get_copilot_session_state_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("[-] Copilot session-state directory not found. Watcher disabled.");
            return;
        }
    };

    println!("[+] Native GitHub Copilot CLI session watcher initialized.");
    let mut tracked: HashMap<PathBuf, CopilotSessionWatchState> = HashMap::new();

    // Initial discovery
    let initial_sessions = scan_copilot_sessions(&session_dir);
    for (path, sid, mtime, file_len) in initial_sessions {
        // If file is large (> 64KB), start from near the end to ingest recent & live events without freezing
        let start_offset = if file_len > 64 * 1024 {
            file_len - 64 * 1024
        } else {
            0
        };

        println!("[+] Copilot watcher tracking session: {} (offset: {})", sid, start_offset);
        tracked.insert(
            path,
            CopilotSessionWatchState {
                session_id: sid,
                last_offset: start_offset,
                last_mtime: mtime,
            },
        );
    }

    loop {
        std::thread::sleep(Duration::from_millis(1500));

        // Periodically discover new sessions
        let current_sessions = scan_copilot_sessions(&session_dir);
        for (path, sid, mtime, file_len) in current_sessions {
            tracked.entry(path).or_insert_with(|| {
                println!("[+] Copilot watcher discovered active session: {}", sid);
                let start_offset = if file_len > 64 * 1024 { file_len - 64 * 1024 } else { 0 };
                CopilotSessionWatchState {
                    session_id: sid,
                    last_offset: start_offset,
                    last_mtime: mtime,
                }
            });
        }

        // Process tracked sessions
        for (path, state) in tracked.iter_mut() {
            let meta = match path.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let current_len = meta.len();
            if current_len <= state.last_offset {
                continue;
            }

            let mut file = match File::open(path) {
                Ok(f) => f,
                Err(_) => continue,
            };

            if file.seek(SeekFrom::Start(state.last_offset)).is_err() {
                continue;
            }

            let mut reader = BufReader::new(file);
            let mut line = String::new();
            let mut bytes_read = 0;

            while let Ok(n) = reader.read_line(&mut line) {
                if n == 0 {
                    break;
                }
                bytes_read += n as u64;

                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    process_copilot_line(trimmed, &state.session_id, &db, &tx);
                }
                line.clear();
            }

            state.last_offset += bytes_read;
            state.last_mtime = meta.modified().unwrap_or(SystemTime::now());
            set_latest_copilot_session_id(state.session_id.clone());
        }
    }
}

fn process_copilot_line(line: &str, session_id: &str, db: &Database, tx: &broadcast::Sender<String>) {
    crate::idle_worker::record_user_activity();
    let val: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return,
    };

    let ev_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let timestamp = val.get("timestamp").and_then(|t| t.as_str())
        .unwrap_or_else(|| "")
        .to_string();

    let ts_str = if timestamp.is_empty() {
        chrono::Utc::now().to_rfc3339()
    } else {
        timestamp
    };

    let data = match val.get("data") {
        Some(d) => d,
        None => return,
    };

    // 1. Tool execution start (Commands, File Edits, Reads)
    if ev_type == "tool.execution_start" {
        let tool_name = data.get("toolName").and_then(|t| t.as_str()).unwrap_or("tool");
        let args = data.get("arguments").cloned().unwrap_or_default();
        let model = data.get("model").and_then(|m| m.as_str()).unwrap_or("gpt-6-luna");

        let (event_type, target) = match tool_name {
            "powershell" | "bash" | "shell" => {
                let cmd = args.get("command").and_then(|c| c.as_str()).unwrap_or(tool_name);
                (EventType::CommandRun, cmd.to_string())
            }
            "write" | "edit" | "create" => {
                let path = args.get("path").or_else(|| args.get("file_path"))
                    .and_then(|p| p.as_str())
                    .unwrap_or("file");
                (EventType::FileWrite, path.to_string())
            }
            "view" | "read" | "search" | "grep" => {
                let target = args.get("path")
                    .or_else(|| args.get("query"))
                    .and_then(|p| p.as_str())
                    .unwrap_or("read");
                (EventType::FileRead, target.to_string())
            }
            _ => {
                let title = data.get("toolTitle").and_then(|t| t.as_str()).unwrap_or(tool_name);
                (EventType::DecisionStep, format!("Copilot Tool: {}", title))
            }
        };

        let mut ev = TraceEvent {
            event_id: uuid::Uuid::new_v4().to_string().replace('-', ""),
            node_id: "PC-1-Copilot".to_string(),
            session_id: session_id.to_string(),
            timestamp: ts_str,
            event_type,
            target,
            action_details: serde_json::json!({
                "engine": "GitHub Copilot CLI",
                "model": model,
                "tool": tool_name,
                "arguments": args,
                "toolTitle": data.get("toolTitle")
            }),
            diff: None,
            provenance_sources: vec!["copilot_cli_events".to_string()],
            security_verdict: SecurityVerdict::Allowed,
            security_reasons: vec!["Copilot CLI workspace operation".to_string()],
            duration_ms: 0.0,
            prev_hash: String::new(),
            event_hash: String::new(),
            signature: None,
            public_key: None,
        };

        if db.insert_event(&mut ev).is_ok() {
            let msg = serde_json::to_string(&ev).unwrap_or_default();
            let _ = tx.send(msg);
        }
    }
    // 2. Model Call / Turn Started (LLM Inference tracking)
    else if ev_type == "model.model_call_started" || ev_type == "model.turn_started" {
        let model = data.get("model").and_then(|m| m.as_str()).unwrap_or("gpt-6-luna");

        let mut ev = TraceEvent {
            event_id: uuid::Uuid::new_v4().to_string().replace('-', ""),
            node_id: "PC-1-Copilot".to_string(),
            session_id: session_id.to_string(),
            timestamp: ts_str,
            event_type: EventType::LlmInference,
            target: format!("GitHub Copilot Model Inference [{}]", model),
            action_details: serde_json::json!({
                "engine": "GitHub Copilot CLI",
                "model": model,
                "turnId": data.get("turnId"),
            }),
            diff: None,
            provenance_sources: vec!["copilot_cli_events".to_string()],
            security_verdict: SecurityVerdict::Allowed,
            security_reasons: vec!["Autonomous Copilot inference turn".to_string()],
            duration_ms: 0.0,
            prev_hash: String::new(),
            event_hash: String::new(),
            signature: None,
            public_key: None,
        };

        if db.insert_event(&mut ev).is_ok() {
            let msg = serde_json::to_string(&ev).unwrap_or_default();
            let _ = tx.send(msg);
        }
    }
}
