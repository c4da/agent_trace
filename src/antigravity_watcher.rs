use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::broadcast;
use crate::db::Database;
use crate::schema::{EventType, SecurityVerdict, TraceEvent};

/// Spawns a background thread that monitors Antigravity session transcripts
/// and automatically inserts Gemini actions, tool calls, and prompts into the ledger.
pub fn spawn_watcher(db: Database, tx_events: broadcast::Sender<String>) {
    std::thread::spawn(move || {
        run_watcher_loop(db, tx_events);
    });
}

fn find_latest_transcript() -> Option<(PathBuf, String)> {
    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\milos".to_string());
    let brain_dir = PathBuf::from(user_profile)
        .join(".gemini")
        .join("antigravity")
        .join("brain");

    if !brain_dir.exists() {
        return None;
    }

    let mut candidate: Option<(PathBuf, String, std::time::SystemTime)> = None;

    if let Ok(entries) = fs::read_dir(&brain_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let conv_id = p.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Prefer transcript_full.jsonl, fallback to transcript.jsonl
                let full_path = p.join(".system_generated").join("logs").join("transcript_full.jsonl");
                let compact_path = p.join(".system_generated").join("logs").join("transcript.jsonl");

                let target_file = if full_path.exists() {
                    Some(full_path)
                } else if compact_path.exists() {
                    Some(compact_path)
                } else {
                    None
                };

                if let Some(tf) = target_file {
                    if let Ok(meta) = tf.metadata() {
                        if let Ok(mtime) = meta.modified() {
                            if candidate.as_ref().map_or(true, |(_, _, best_time)| mtime > *best_time) {
                                candidate = Some((tf, conv_id, mtime));
                            }
                        }
                    }
                }
            }
        }
    }

    candidate.map(|(path, conv_id, _)| (path, conv_id))
}

fn run_watcher_loop(db: Database, tx: broadcast::Sender<String>) {
    println!("[+] Native Antigravity auto-watcher initialized.");
    let mut current_path: Option<PathBuf> = None;
    let mut current_conv_id = "active_session".to_string();
    let mut last_file_pos: u64 = 0;

    loop {
        std::thread::sleep(Duration::from_millis(1500));

        if let Some((path, conv_id)) = find_latest_transcript() {
            let switched = current_path.as_ref() != Some(&path);
            if switched {
                println!("[+] Antigravity watcher tracking active session: {}", conv_id);
                current_path = Some(path.clone());
                current_conv_id = conv_id;

                // Start from near the end on first discovery to avoid flooding with thousands of historical lines
                if let Ok(meta) = fs::metadata(&path) {
                    let len = meta.len();
                    // Read last ~16KB of existing session
                    last_file_pos = len.saturating_sub(16384);
                } else {
                    last_file_pos = 0;
                }
            }

            if let Ok(mut file) = File::open(&path) {
                if let Ok(meta) = file.metadata() {
                    let file_len = meta.len();
                    if file_len < last_file_pos {
                        // File was truncated or restarted
                        last_file_pos = 0;
                    }
                }

                if file.seek(SeekFrom::Start(last_file_pos)).is_ok() {
                    let reader = BufReader::new(file);
                    let mut bytes_read: u64 = 0;

                    for line_res in reader.lines() {
                        if let Ok(line) = line_res {
                            bytes_read += line.len() as u64 + 1; // +1 for newline
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                process_transcript_line(trimmed, &current_conv_id, &db, &tx);
                            }
                        } else {
                            break;
                        }
                    }
                    last_file_pos += bytes_read;
                }
            }
        }
    }
}

fn process_transcript_line(line: &str, conv_id: &str, db: &Database, tx: &broadcast::Sender<String>) {
    let val: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return,
    };

    let source = val.get("source").and_then(|s| s.as_str()).unwrap_or("");
    let stype = val.get("type").and_then(|s| s.as_str()).unwrap_or("");
    let created = val.get("created_at")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    // 1. User Prompt
    if source == "USER_EXPLICIT" && stype == "USER_INPUT" {
        let content = val.get("content").and_then(|c| c.as_str()).unwrap_or("");
        let prompt_clean = if content.contains("<USER_REQUEST>") {
            content.split("<USER_REQUEST>")
                .nth(1)
                .and_then(|s| s.split("</USER_REQUEST>").next())
                .unwrap_or(content)
                .trim()
                .to_string()
        } else {
            content.chars().take(600).collect::<String>()
        };

        let mut ev = TraceEvent {
            event_id: uuid::Uuid::new_v4().simple().to_string(),
            node_id: "PC-1-Antigravity".to_string(),
            session_id: conv_id.to_string(),
            timestamp: created,
            event_type: EventType::LlmInference,
            target: "Gemini Direct RPC [User Prompt]".to_string(),
            action_details: serde_json::json!({
                "role": "user",
                "prompt": prompt_clean,
                "model": "User Input -> Gemini",
                "summary": "User prompt received"
            }),
            diff: None,
            provenance_sources: vec![],
            security_verdict: SecurityVerdict::Allowed,
            security_reasons: vec![],
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
    // 2. Model Tool Calls or Responses
    else if source == "MODEL" && stype == "PLANNER_RESPONSE" {
        let content = val.get("content").and_then(|c| c.as_str()).unwrap_or("");
        let thinking = val.get("thinking").and_then(|t| t.as_str()).unwrap_or("");
        let tool_calls_opt = val.get("tool_calls").and_then(|t| t.as_array());

        let intended_tools: String = if let Some(tc_list) = tool_calls_opt {
            let names: Vec<String> = tc_list.iter()
                .filter_map(|tc| {
                    let n = tc.get("name").and_then(|x| x.as_str()).unwrap_or("");
                    let s = tc.get("args").and_then(|a| a.get("toolSummary")).and_then(|x| x.as_str()).unwrap_or("");
                    if !s.is_empty() {
                        Some(format!("{} ({})", n, s))
                    } else if !n.is_empty() {
                        Some(n.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            names.join(", ")
        } else {
            String::new()
        };

        let target_str = if !intended_tools.is_empty() {
            format!("Gemini Direct RPC [Tool Planning: {}]", intended_tools)
        } else {
            "Gemini Direct RPC [Assistant Response]".to_string()
        };

        if !content.is_empty() || !thinking.is_empty() {
            let mut ev = TraceEvent {
                event_id: uuid::Uuid::new_v4().simple().to_string(),
                node_id: "PC-1-Antigravity".to_string(),
                session_id: conv_id.to_string(),
                timestamp: created.clone(),
                event_type: EventType::LlmInference,
                target: target_str,
                action_details: serde_json::json!({
                    "role": "assistant",
                    "model": "Gemini 3.8 / 2.5 Pro",
                    "completion": content.chars().take(2000).collect::<String>(),
                    "thinking": thinking.chars().take(3000).collect::<String>(),
                    "intended_tools": if !intended_tools.is_empty() { serde_json::Value::String(intended_tools) } else { serde_json::Value::Null }
                }),
                diff: None,
                provenance_sources: vec![],
                security_verdict: SecurityVerdict::Allowed,
                security_reasons: vec![],
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

        // Process tool_calls array
        if let Some(tool_calls) = val.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tool_calls {
                let name = tc.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = tc.get("args").cloned().unwrap_or(serde_json::Value::Null);

                let summary = args.get("toolSummary")
                    .and_then(|s| s.as_str())
                    .unwrap_or(name)
                    .to_string();

                let (ev_type, target, diff, prov) = match name {
                    "view_file" => {
                        let path = args.get("AbsolutePath")
                            .and_then(|p| p.as_str())
                            .unwrap_or("file")
                            .to_string();
                        (EventType::FileRead, path.clone(), None, vec![path])
                    }
                    "write_to_file" | "replace_file_content" => {
                        let path = args.get("TargetFile")
                            .and_then(|p| p.as_str())
                            .unwrap_or("file")
                            .to_string();
                        let diff_str = args.get("ReplacementContent")
                            .or_else(|| args.get("CodeContent"))
                            .and_then(|d| d.as_str())
                            .map(|s| s.chars().take(2000).collect::<String>());
                        (EventType::FileWrite, path.clone(), diff_str, vec![path])
                    }
                    "run_command" => {
                        let cmd = args.get("CommandLine")
                            .and_then(|c| c.as_str())
                            .unwrap_or("command")
                            .to_string();
                        (EventType::CommandRun, cmd, None, vec![])
                    }
                    _ => continue,
                };

                let mut ev = TraceEvent {
                    event_id: uuid::Uuid::new_v4().simple().to_string(),
                    node_id: "PC-1-Antigravity".to_string(),
                    session_id: conv_id.to_string(),
                    timestamp: created.clone(),
                    event_type: ev_type,
                    target,
                    action_details: serde_json::json!({
                        "tool": name,
                        "summary": summary,
                        "args": args
                    }),
                    diff,
                    provenance_sources: prov,
                    security_verdict: SecurityVerdict::Allowed,
                    security_reasons: vec![],
                    duration_ms: 15.0,
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
    }
}
