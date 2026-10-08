use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{Duration, SystemTime};
use tokio::sync::broadcast;
use crate::db::Database;
use crate::schema::{EventType, SecurityVerdict, TraceEvent};

/// Global holder for the most recently active Antigravity session ID.
static LATEST_ACTIVE_SESSION: RwLock<Option<String>> = RwLock::new(None);

/// Retrieves the latest session ID observed across all active Antigravity sessions.
pub fn get_latest_session_id() -> Option<String> {
    LATEST_ACTIVE_SESSION.read().ok().and_then(|guard| guard.clone())
}

/// Sets the latest active session ID.
pub fn set_latest_session_id(id: String) {
    if let Ok(mut guard) = LATEST_ACTIVE_SESSION.write() {
        *guard = Some(id);
    }
}

/// State tracking for each discovered session file.
#[derive(Debug)]
struct SessionWatchState {
    conv_id: String,
    last_offset: u64,
    #[allow(dead_code)]
    last_mtime: SystemTime,
}

/// Spawns a background thread that concurrently monitors all active Antigravity session transcripts
/// across all open conversations, capturing agent actions, subagents, tool calls, and Agent2 / PC-2
/// delegations into the tamper-evident ledger.
pub fn spawn_watcher(db: Database, tx_events: broadcast::Sender<String>) {
    std::thread::spawn(move || {
        run_watcher_loop(db, tx_events);
    });
}

fn get_brain_dir() -> Option<PathBuf> {
    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\milos".to_string());
    let brain_dir = PathBuf::from(user_profile)
        .join(".gemini")
        .join("antigravity")
        .join("brain");

    if brain_dir.exists() {
        Some(brain_dir)
    } else {
        None
    }
}

/// Discovers all conversation transcripts within the Antigravity brain directory.
/// Returns a list of (transcript_path, conv_id, mtime, file_len), sorted with the most recently modified first.
fn scan_all_transcripts(brain_dir: &PathBuf) -> Vec<(PathBuf, String, SystemTime, u64)> {
    let mut transcripts = Vec::new();

    let entries = match fs::read_dir(brain_dir) {
        Ok(e) => e,
        Err(_) => return transcripts,
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

        // Skip internal/scratch directories
        if dir_name.starts_with('.') || dir_name == "tempmediaStorage" {
            continue;
        }

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
                    if mtime >= cutoff {
                        transcripts.push((tf, dir_name, mtime, meta.len()));
                    }
                }
            }
        }
    }

    // Sort by mtime descending (most recent first) and cap to top 30
    transcripts.sort_by(|a, b| b.2.cmp(&a.2));
    transcripts.truncate(30);
    transcripts
}

fn run_watcher_loop(db: Database, tx: broadcast::Sender<String>) {
    println!("[+] Native Antigravity multi-session watcher initialized.");
    let brain_dir = match get_brain_dir() {
        Some(d) => d,
        None => {
            eprintln!("[-] Antigravity brain dir not found, watcher standing by.");
            return;
        }
    };

    let mut session_map: HashMap<PathBuf, SessionWatchState> = HashMap::new();
    let mut is_initial_scan = true;

    loop {
        std::thread::sleep(Duration::from_millis(1000));

        let active_transcripts = scan_all_transcripts(&brain_dir);

        for (path, conv_id, mtime, file_len) in active_transcripts {
            let state = session_map.entry(path.clone()).or_insert_with(|| {
                let initial_offset = if is_initial_scan {
                    file_len.saturating_sub(32768)
                } else {
                    0
                };
                println!("[+] Antigravity watcher tracking session: {} (offset: {})", conv_id, initial_offset);
                SessionWatchState {
                    conv_id: conv_id.clone(),
                    last_offset: initial_offset,
                    last_mtime: mtime,
                }
            });

            if file_len < state.last_offset {
                // File truncated or rewritten
                state.last_offset = 0;
            }

            if file_len > state.last_offset {
                if let Ok(mut file) = File::open(&path) {
                    if file.seek(SeekFrom::Start(state.last_offset)).is_ok() {
                        let reader = BufReader::new(file);
                        let mut bytes_read: u64 = 0;

                        for line_res in reader.lines() {
                            if let Ok(line) = line_res {
                                bytes_read += line.len() as u64 + 1; // +1 for newline
                                let trimmed = line.trim();
                                if !trimmed.is_empty() {
                                    set_latest_session_id(state.conv_id.clone());
                                    process_transcript_line(trimmed, &state.conv_id, &db, &tx);
                                }
                            } else {
                                break;
                            }
                        }
                        state.last_offset += bytes_read;
                        state.last_mtime = mtime;
                    }
                }
            }
        }

        is_initial_scan = false;
    }
}

pub fn process_transcript_line(line: &str, conv_id: &str, db: &Database, tx: &broadcast::Sender<String>) {
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

        let targets_agent2 = prompt_clean.to_lowercase().contains("agent2")
            || prompt_clean.to_lowercase().contains("pc2")
            || prompt_clean.to_lowercase().contains("pc-2")
            || prompt_clean.to_lowercase().contains("deepseek")
            || prompt_clean.to_lowercase().contains("worker node")
            || prompt_clean.to_lowercase().contains("192.168.1.102");

        let mut ev = TraceEvent {
            event_id: uuid::Uuid::new_v4().simple().to_string(),
            node_id: "PC-1-Antigravity".to_string(),
            session_id: conv_id.to_string(),
            timestamp: created,
            event_type: EventType::LlmInference,
            target: if targets_agent2 {
                "Gemini Direct RPC [User Prompt -> Targeting Agent2 / PC-2]".to_string()
            } else {
                "Gemini Direct RPC [User Prompt]".to_string()
            },
            action_details: serde_json::json!({
                "role": "user",
                "prompt": prompt_clean,
                "model": "User Input -> Gemini",
                "summary": "User prompt received",
                "cross_node_target": if targets_agent2 { "PC-2" } else { "PC-1" }
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
            let mentions_agent2 = content.contains("192.168.1.102")
                || content.contains(":8642")
                || content.contains("agent2")
                || content.contains("PC-2")
                || content.contains("deepseek-r1:1.5b")
                || thinking.contains("192.168.1.102")
                || thinking.contains(":8642")
                || thinking.contains("agent2")
                || thinking.contains("PC-2");

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
                    "intended_tools": if !intended_tools.is_empty() { serde_json::Value::String(intended_tools) } else { serde_json::Value::Null },
                    "cross_node_intent": if mentions_agent2 { "PC-2 / Agent2 Delegation" } else { "Local PC-1" }
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

                let (ev_type, target, diff, prov, node_id) = match name {
                    "view_file" => {
                        let path = args.get("AbsolutePath")
                            .and_then(|p| p.as_str())
                            .unwrap_or("file")
                            .to_string();
                        (EventType::FileRead, path.clone(), None, vec![path], "PC-1-Antigravity".to_string())
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
                        (EventType::FileWrite, path.clone(), diff_str, vec![path], "PC-1-Antigravity".to_string())
                    }
                    "run_command" => {
                        let cmd = args.get("CommandLine")
                            .and_then(|c| c.as_str())
                            .unwrap_or("command")
                            .to_string();

                        let is_agent2_pc2 = cmd.contains("192.168.1.102")
                            || cmd.contains(":8642")
                            || cmd.contains("node-02")
                            || cmd.contains("NODE-02")
                            || cmd.contains("agent2")
                            || cmd.contains("agent-2")
                            || cmd.contains("hermes-agent")
                            || cmd.contains("deepseek-r1:1.5b")
                            || (cmd.contains("hermes_dual_runner.py") && cmd.contains("remote"));

                        if is_agent2_pc2 {
                            (
                                EventType::CommandRun,
                                format!("Agent2 / PC-2 Worker (192.168.1.102:8642) [{}]", summary),
                                None,
                                vec![],
                                "PC-2".to_string(),
                            )
                        } else {
                            (EventType::CommandRun, cmd, None, vec![], "PC-1-Antigravity".to_string())
                        }
                    }
                    "send_message" => {
                        let recipient = args.get("Recipient")
                            .and_then(|r| r.as_str())
                            .unwrap_or("subagent");
                        let is_pc2 = recipient.to_lowercase().contains("agent2")
                            || recipient.to_lowercase().contains("pc2")
                            || recipient.to_lowercase().contains("pc-2")
                            || recipient.to_lowercase().contains("worker")
                            || recipient.to_lowercase().contains("remote");

                        let nid = if is_pc2 { "PC-2".to_string() } else { "PC-1-Antigravity".to_string() };
                        (
                            EventType::DecisionStep,
                            format!("Agent Direct Message -> {} [{}]", recipient, summary),
                            None,
                            vec![],
                            nid,
                        )
                    }
                    "invoke_subagent" => {
                        let subagents = args.get("Subagents").and_then(|s| s.as_array());
                        let roles: Vec<String> = subagents.map(|arr| {
                            arr.iter().filter_map(|sub| {
                                sub.get("Role").or_else(|| sub.get("TypeName")).and_then(|r| r.as_str()).map(|s| s.to_string())
                            }).collect()
                        }).unwrap_or_default();

                        let args_str = serde_json::to_string(&args).unwrap_or_default().to_lowercase();
                        let is_pc2 = args_str.contains("agent2")
                            || args_str.contains("pc2")
                            || args_str.contains("pc-2")
                            || args_str.contains("worker")
                            || args_str.contains("deepseek")
                            || args_str.contains("192.168.1.102");

                        let nid = if is_pc2 { "PC-2".to_string() } else { "PC-1-Antigravity".to_string() };
                        (
                            EventType::DecisionStep,
                            format!("Subagent Swarm Invocation: {} [{}]", roles.join(", "), summary),
                            None,
                            vec![],
                            nid,
                        )
                    }
                    "manage_subagents" => {
                        let action = args.get("Action").and_then(|a| a.as_str()).unwrap_or("manage");
                        (
                            EventType::DecisionStep,
                            format!("Subagent Lifecycle [Action: {}]", action),
                            None,
                            vec![],
                            "PC-1-Antigravity".to_string(),
                        )
                    }
                    "read_url_content" => {
                        let url = args.get("Url").and_then(|u| u.as_str()).unwrap_or("url").to_string();
                        let is_pc2 = url.contains("192.168.1.102") || url.contains(":8642");
                        let nid = if is_pc2 { "PC-2".to_string() } else { "PC-1-Antigravity".to_string() };
                        let target = if is_pc2 {
                            format!("Agent2 / PC-2 HTTP Request: {} [{}]", url, summary)
                        } else {
                            url
                        };
                        (EventType::WebRequest, target, None, vec![], nid)
                    }
                    "manage_task" => {
                        let action = args.get("Action").and_then(|a| a.as_str()).unwrap_or("manage");
                        let task_id = args.get("TaskId").and_then(|t| t.as_str()).unwrap_or("");
                        (
                            EventType::CommandRun,
                            format!("Background Task Management: {} ({})", action, task_id),
                            None,
                            vec![],
                            "PC-1-Antigravity".to_string(),
                        )
                    }
                    "schedule" => {
                        (
                            EventType::CommandRun,
                            format!("Scheduled Task: {}", summary),
                            None,
                            vec![],
                            "PC-1-Antigravity".to_string(),
                        )
                    }
                    _ => {
                        (
                            EventType::CommandRun,
                            format!("Tool Action [{}]: {}", name, summary),
                            None,
                            vec![],
                            "PC-1-Antigravity".to_string(),
                        )
                    }
                };

                let mut ev = TraceEvent {
                    event_id: uuid::Uuid::new_v4().simple().to_string(),
                    node_id,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_latest_session_id() {
        set_latest_session_id("session-test-123".to_string());
        assert_eq!(get_latest_session_id(), Some("session-test-123".to_string()));
    }

    #[test]
    fn test_agent2_run_command_detection() {
        let db = Database::new(":memory:").expect("memory db");
        let (tx, mut rx) = broadcast::channel(16);

        let line = serde_json::json!({
            "source": "MODEL",
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-10-08T00:00:00Z",
            "tool_calls": [{
                "name": "run_command",
                "args": {
                    "CommandLine": "python assets/scripts/hermes_dual_runner.py remote \"run tests\"",
                    "toolSummary": "Run remote tests on Agent2",
                    "toolAction": "Running remote tests"
                }
            }]
        }).to_string();

        process_transcript_line(&line, "conv-multi-session-01", &db, &tx);

        let msg = rx.try_recv().expect("event emitted");
        let ev: serde_json::Value = serde_json::from_str(&msg).expect("json parsed");

        assert_eq!(ev["node_id"], "PC-2");
        assert_eq!(ev["session_id"], "conv-multi-session-01");
        assert!(ev["target"].as_str().unwrap().contains("Agent2 / PC-2 Worker"));
    }

    #[test]
    fn test_agent2_send_message_detection() {
        let db = Database::new(":memory:").expect("memory db");
        let (tx, mut rx) = broadcast::channel(16);

        let line = serde_json::json!({
            "source": "MODEL",
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-10-08T00:00:00Z",
            "tool_calls": [{
                "name": "send_message",
                "args": {
                    "Recipient": "agent2",
                    "Message": "Verify physics solver on CUDA",
                    "toolSummary": "Dispatch to agent2"
                }
            }]
        }).to_string();

        process_transcript_line(&line, "conv-multi-session-02", &db, &tx);

        let msg = rx.try_recv().expect("event emitted");
        let ev: serde_json::Value = serde_json::from_str(&msg).expect("json parsed");

        assert_eq!(ev["node_id"], "PC-2");
        assert_eq!(ev["session_id"], "conv-multi-session-02");
        assert_eq!(ev["event_type"], "DecisionStep");
    }
}
