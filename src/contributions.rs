//! Multi-Agent Problem Contribution & Topic Attribution Engine (`/graph` & `/api/v1/telemetry/contributions`).
//!
//! Co-designed with Qwen 3.5 and DeepSeek-R1 to quantify and visualize how much each
//! agent (`PC-1-Antigravity`, `PC-1` Qwen/Ollama, `PC-2` DeepSeek-R1/Hermes, `PC-1-Copilot`)
//! contributed to solving each problem across both **Feature-Based** and **Prompt/Role-Based** dimensions.

use crate::schema::{EventType, SecurityVerdict, TraceEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Canonicalize raw `node_id` / `agent_id` strings into clear cluster agent identities.
pub fn canonical_agent_label(raw_node_id: &str, target: &str, details: &serde_json::Value) -> (String, String) {
    let lower = raw_node_id.to_lowercase();
    let target_lower = target.to_lowercase();
    let model_str = details
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    if lower.contains("antigravity") {
        (
            "PC-1-Antigravity".to_string(),
            "Antigravity (Lead Architect)".to_string(),
        )
    } else if lower.contains("copilot") {
        (
            "PC-1-Copilot".to_string(),
            "GitHub Copilot CLI".to_string(),
        )
    } else if lower.contains("pc-2")
        || target_lower.contains("deepseek")
        || model_str.contains("deepseek")
        || model_str.contains("hermes")
    {
        (
            "PC-2-DeepSeek".to_string(),
            "PC-2 Worker (DeepSeek-R1 / Hermes)".to_string(),
        )
    } else if lower == "pc-1"
        || target_lower.contains("qwen")
        || model_str.contains("qwen")
        || model_str.contains("gemma")
    {
        (
            "PC-1-Qwen".to_string(),
            "PC-1 Local LLM (Qwen 27B / 9B)".to_string(),
        )
    } else if lower.contains("governance") || lower.contains("watchdog") {
        (
            "Cluster-Governance".to_string(),
            "Autonomous Governance Watchdog".to_string(),
        )
    } else {
        (raw_node_id.to_string(), raw_node_id.to_string())
    }
}

/// Classifies an event into a **Feature / Subsystem Topic** based on file paths, commands, and prompts.
pub fn infer_feature_topic(ev: &TraceEvent) -> String {
    let t = ev.target.to_lowercase();
    let prompt = ev
        .action_details
        .get("prompt")
        .or_else(|| ev.action_details.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    let combined = format!("{} {}", t, prompt);

    if combined.contains("zone")
        || combined.contains("policy")
        || combined.contains("playground")
        || combined.contains(".env")
        || combined.contains(".secrets")
        || combined.contains(".ssh")
        || combined.contains("sandbox")
        || combined.contains("whitelist")
    {
        "Safe Zones & Policy Firewall".to_string()
    } else if combined.contains("dashboard")
        || combined.contains("console")
        || combined.contains("graph")
        || combined.contains("flyer")
        || combined.contains("mermaid")
        || combined.contains(".html")
        || combined.contains("ui ")
    {
        "UI, Console & Graph Workbench".to_string()
    } else if combined.contains("ledger")
        || combined.contains("ed25519")
        || combined.contains("sha256")
        || combined.contains("verify")
        || combined.contains("signature")
        || combined.contains("hash")
    {
        "Cryptographic Ledger & Provenance".to_string()
    } else if combined.contains("hub")
        || combined.contains("router")
        || combined.contains("delegate")
        || combined.contains("delegation")
        || combined.contains("mcp")
        || combined.contains("handoff")
        || combined.contains("agent_trace_client")
        || combined.contains("chat/completions")
    {
        "Prompt Hub, Routing & SDK".to_string()
    } else if combined.contains("strux")
        || combined.contains("rustcad")
        || combined.contains("cad")
        || combined.contains("fem")
        || combined.contains("mesh")
        || combined.contains("camera.rs")
        || combined.contains("extrude")
    {
        "CAD & Geometry Engine (Strux)".to_string()
    } else if combined.contains("test")
        || combined.contains("cargo check")
        || combined.contains("assert")
        || combined.contains("clippy")
    {
        "Tests & Invariant Verification".to_string()
    } else {
        "Cluster Ops & Telemetry".to_string()
    }
}

/// Classifies an event into a **Prompt / Cognitive Role Topic** (how the problem was tackled).
pub fn infer_prompt_role_topic(ev: &TraceEvent) -> String {
    let t = ev.target.to_lowercase();
    let prompt = ev
        .action_details
        .get("prompt")
        .or_else(|| ev.action_details.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    match ev.event_type {
        EventType::FileWrite => "Code Implementation & File Diffs".to_string(),
        EventType::LlmInference => {
            if prompt.contains("verify")
                || prompt.contains("sanity")
                || prompt.contains("audit")
                || prompt.contains("regex")
                || t.contains("deepseek")
            {
                "Sanity & Security Verification (LLM)".to_string()
            } else if prompt.contains("architect")
                || prompt.contains("design")
                || prompt.contains("refactor")
                || prompt.contains("plan")
                || t.contains("27b")
            {
                "Architecture & Deep Reasoning (LLM)".to_string()
            } else {
                "Fast Code & Prompt Generation (LLM)".to_string()
            }
        }
        EventType::CommandRun => {
            if t.contains("test") || prompt.contains("test") || t.contains("verify") {
                "Automated Test & Build Execution".to_string()
            } else {
                "Shell & Tool Orchestration".to_string()
            }
        }
        EventType::DecisionStep => "Governance & Policy Decisions".to_string(),
        EventType::FileRead | EventType::WebRequest => {
            "Codebase Research & Context Pickup".to_string()
        }
    }
}

/// Counts changed lines (`+` / `-`) in a unified diff string.
pub fn count_diff_lines(diff: Option<&str>) -> u64 {
    let Some(d) = diff else { return 0 };
    d.lines()
        .filter(|line| {
            (line.starts_with('+') && !line.starts_with("+++"))
                || (line.starts_with('-') && !line.starts_with("---"))
        })
        .count() as u64
}

/// Computes the weighted engineering impact score for a single `TraceEvent`.
///
/// Weights:
/// - `FileWrite`: `5.0 + min(10.0, diff_lines * 0.15)`
/// - `LlmInference`: `3.5`
/// - `CommandRun` / `DecisionStep`: `2.0`
/// - `FileRead` / `WebRequest`: `1.0`
pub fn compute_event_impact(ev: &TraceEvent) -> f64 {
    let base = match ev.event_type {
        EventType::FileWrite => {
            let lines = count_diff_lines(ev.diff.as_deref()) as f64;
            5.0 + (lines * 0.15).min(10.0)
        }
        EventType::LlmInference => 3.5,
        EventType::CommandRun | EventType::DecisionStep => 2.0,
        EventType::FileRead | EventType::WebRequest => 1.0,
    };
    // Round to 2 decimal places
    (base * 100.0).round() / 100.0
}

/// Per-(Topic, Agent) contribution cell across all 4 metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTopicCell {
    pub topic: String,
    pub topic_mode: String, // "feature" or "prompt_role"
    pub agent_id: String,
    pub agent_label: String,
    pub touchpoint_count: u64,
    pub weighted_impact: f64,
    pub compute_ms: f64,
    pub write_diff_score: u64,
    pub reads: u64,
    pub writes: u64,
    pub commands: u64,
    pub llm_calls: u64,
    pub blocked_or_flagged: u64,
}

/// Summary node for an Agent in the contribution graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentGraphNode {
    pub agent_id: String,
    pub agent_label: String,
    pub total_touchpoints: u64,
    pub total_weighted_impact: f64,
    pub total_compute_ms: f64,
    pub total_write_diff_score: u64,
    pub share_pct: f64,
    pub topics_touched: usize,
}

/// Summary node for a Problem/Feature Topic in the contribution graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicGraphNode {
    pub topic: String,
    pub topic_mode: String,
    pub total_touchpoints: u64,
    pub total_weighted_impact: f64,
    pub total_compute_ms: f64,
    pub total_write_diff_score: u64,
    pub top_contributor: String,
}

/// Complete payload returned by `GET /api/v1/telemetry/contributions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionGraphReport {
    pub generated_at: String,
    pub total_events_analyzed: usize,
    pub agents: Vec<AgentGraphNode>,
    pub feature_topics: Vec<TopicGraphNode>,
    pub prompt_role_topics: Vec<TopicGraphNode>,
    pub feature_cells: Vec<AgentTopicCell>,
    pub prompt_role_cells: Vec<AgentTopicCell>,
}

/// Builds a complete `ContributionGraphReport` from a slice of `TraceEvent` records.
pub fn build_contribution_report(events: &[TraceEvent]) -> ContributionGraphReport {
    let mut feature_map: HashMap<(String, String), AgentTopicCell> = HashMap::new();
    let mut role_map: HashMap<(String, String), AgentTopicCell> = HashMap::new();
    let mut agent_map: HashMap<String, (String, u64, f64, f64, u64, std::collections::HashSet<String>)> =
        HashMap::new();

    for ev in events {
        let (agent_id, agent_label) =
            canonical_agent_label(&ev.node_id, &ev.target, &ev.action_details);
        let feat_topic = infer_feature_topic(ev);
        let role_topic = infer_prompt_role_topic(ev);
        let impact = compute_event_impact(ev);
        let compute_ms = if ev.duration_ms > 0.0 {
            ev.duration_ms
        } else {
            ev.action_details
                .get("duration_ms")
                .and_then(|v| v.as_f64())
                .unwrap_or(12.0)
        };
        let diff_lines = count_diff_lines(ev.diff.as_deref());
        let write_score = if ev.event_type == EventType::FileWrite {
            10 + diff_lines
        } else {
            0
        };

        let update_cell = |map: &mut HashMap<(String, String), AgentTopicCell>,
                           topic: &str,
                           mode: &str| {
            let key = (topic.to_string(), agent_id.clone());
            let cell = map.entry(key).or_insert_with(|| AgentTopicCell {
                topic: topic.to_string(),
                topic_mode: mode.to_string(),
                agent_id: agent_id.clone(),
                agent_label: agent_label.clone(),
                touchpoint_count: 0,
                weighted_impact: 0.0,
                compute_ms: 0.0,
                write_diff_score: 0,
                reads: 0,
                writes: 0,
                commands: 0,
                llm_calls: 0,
                blocked_or_flagged: 0,
            });
            cell.touchpoint_count += 1;
            cell.weighted_impact = ((cell.weighted_impact + impact) * 100.0).round() / 100.0;
            cell.compute_ms = ((cell.compute_ms + compute_ms) * 100.0).round() / 100.0;
            cell.write_diff_score += write_score;
            match ev.event_type {
                EventType::FileRead | EventType::WebRequest => cell.reads += 1,
                EventType::FileWrite => cell.writes += 1,
                EventType::CommandRun | EventType::DecisionStep => cell.commands += 1,
                EventType::LlmInference => cell.llm_calls += 1,
            }
            if ev.security_verdict != SecurityVerdict::Allowed {
                cell.blocked_or_flagged += 1;
            }
        };

        update_cell(&mut feature_map, &feat_topic, "feature");
        update_cell(&mut role_map, &role_topic, "prompt_role");

        let ag_entry = agent_map.entry(agent_id.clone()).or_insert_with(|| {
            (
                agent_label.clone(),
                0,
                0.0,
                0.0,
                0,
                std::collections::HashSet::new(),
            )
        });
        ag_entry.1 += 1;
        ag_entry.2 = ((ag_entry.2 + impact) * 100.0).round() / 100.0;
        ag_entry.3 = ((ag_entry.3 + compute_ms) * 100.0).round() / 100.0;
        ag_entry.4 += write_score;
        ag_entry.5.insert(feat_topic);
    }

    let grand_total_impact: f64 = agent_map.values().map(|v| v.2).sum::<f64>().max(1.0);

    let mut agents: Vec<AgentGraphNode> = agent_map
        .into_iter()
        .map(
            |(agent_id, (agent_label, tp, imp, ms, wscore, topics))| AgentGraphNode {
                agent_id,
                agent_label,
                total_touchpoints: tp,
                total_weighted_impact: imp,
                total_compute_ms: ms,
                total_write_diff_score: wscore,
                share_pct: ((imp / grand_total_impact) * 1000.0).round() / 10.0,
                topics_touched: topics.len(),
            },
        )
        .collect();
    agents.sort_by(|a, b| {
        b.total_weighted_impact
            .partial_cmp(&a.total_weighted_impact)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let summarize_topics = |cells: &[AgentTopicCell], mode: &str| -> Vec<TopicGraphNode> {
        let mut tmap: HashMap<String, Vec<&AgentTopicCell>> = HashMap::new();
        for c in cells {
            tmap.entry(c.topic.clone()).or_default().push(c);
        }
        let mut out: Vec<TopicGraphNode> = tmap
            .into_iter()
            .map(|(topic, group)| {
                let total_touchpoints = group.iter().map(|c| c.touchpoint_count).sum();
                let total_weighted_impact = ((group.iter().map(|c| c.weighted_impact).sum::<f64>())
                    * 100.0)
                    .round()
                    / 100.0;
                let total_compute_ms =
                    ((group.iter().map(|c| c.compute_ms).sum::<f64>()) * 100.0).round() / 100.0;
                let total_write_diff_score = group.iter().map(|c| c.write_diff_score).sum();
                let top_contributor = group
                    .iter()
                    .max_by(|a, b| {
                        a.weighted_impact
                            .partial_cmp(&b.weighted_impact)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|c| c.agent_id.clone())
                    .unwrap_or_else(|| "N/A".to_string());

                TopicGraphNode {
                    topic,
                    topic_mode: mode.to_string(),
                    total_touchpoints,
                    total_weighted_impact,
                    total_compute_ms,
                    total_write_diff_score,
                    top_contributor,
                }
            })
            .collect();
        out.sort_by(|a, b| {
            b.total_weighted_impact
                .partial_cmp(&a.total_weighted_impact)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    };

    let mut feature_cells: Vec<AgentTopicCell> = feature_map.into_values().collect();
    feature_cells.sort_by(|a, b| {
        b.weighted_impact
            .partial_cmp(&a.weighted_impact)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut prompt_role_cells: Vec<AgentTopicCell> = role_map.into_values().collect();
    prompt_role_cells.sort_by(|a, b| {
        b.weighted_impact
            .partial_cmp(&a.weighted_impact)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let feature_topics = summarize_topics(&feature_cells, "feature");
    let prompt_role_topics = summarize_topics(&prompt_role_cells, "prompt_role");

    ContributionGraphReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        total_events_analyzed: events.len(),
        agents,
        feature_topics,
        prompt_role_topics,
        feature_cells,
        prompt_role_cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(
        node_id: &str,
        event_type: EventType,
        target: &str,
        prompt: &str,
        diff: Option<&str>,
        duration_ms: f64,
    ) -> TraceEvent {
        TraceEvent {
            event_id: "ev-test".to_string(),
            node_id: node_id.to_string(),
            session_id: "sess-1".to_string(),
            timestamp: "2026-10-12T22:00:00Z".to_string(),
            event_type,
            target: target.to_string(),
            action_details: serde_json::json!({ "prompt": prompt }),
            diff: diff.map(|s| s.to_string()),
            provenance_sources: vec![],
            security_verdict: SecurityVerdict::Allowed,
            security_reasons: vec![],
            duration_ms,
            prev_hash: "0".repeat(64),
            event_hash: "1".repeat(64),
            signature: None,
            public_key: None,
        }
    }

    #[test]
    fn test_infer_feature_topics_across_all_domains() {
        let ev_zone = make_event("PC-1-Antigravity", EventType::FileWrite, "src/zones.rs", "", None, 10.0);
        assert_eq!(infer_feature_topic(&ev_zone), "Safe Zones & Policy Firewall");

        let ev_ui = make_event("PC-1-Antigravity", EventType::FileWrite, "src/dashboard.html", "", None, 10.0);
        assert_eq!(infer_feature_topic(&ev_ui), "UI, Console & Graph Workbench");

        let ev_ledger = make_event("PC-1", EventType::CommandRun, "src/ledger.rs", "verify ed25519", None, 10.0);
        assert_eq!(infer_feature_topic(&ev_ledger), "Cryptographic Ledger & Provenance");

        let ev_hub = make_event("PC-1", EventType::LlmInference, "src/hub.rs", "router delegation", None, 10.0);
        assert_eq!(infer_feature_topic(&ev_hub), "Prompt Hub, Routing & SDK");

        let ev_cad = make_event("PC-1-Antigravity", EventType::FileWrite, "C:/cada/projects/Strux/src/cad/extrude.rs", "", None, 10.0);
        assert_eq!(infer_feature_topic(&ev_cad), "CAD & Geometry Engine (Strux)");

        let ev_test = make_event("PC-2", EventType::CommandRun, "cargo test --workspace", "", None, 10.0);
        assert_eq!(infer_feature_topic(&ev_test), "Tests & Invariant Verification");
    }

    #[test]
    fn test_infer_prompt_role_topics() {
        let ev_write = make_event("PC-1-Antigravity", EventType::FileWrite, "src/main.rs", "", Some("+fn main() {}"), 15.0);
        assert_eq!(infer_prompt_role_topic(&ev_write), "Code Implementation & File Diffs");

        let ev_verify = make_event("PC-2", EventType::LlmInference, "http://192.168.1.102:8642 [deepseek-r1:1.5b]", "Verify regex sanity", None, 120.0);
        assert_eq!(infer_prompt_role_topic(&ev_verify), "Sanity & Security Verification (LLM)");

        let ev_arch = make_event("PC-1", EventType::LlmInference, "http://127.0.0.1:11434 [qwen3.8:27b]", "Architect a lock-free channel", None, 400.0);
        assert_eq!(infer_prompt_role_topic(&ev_arch), "Architecture & Deep Reasoning (LLM)");
    }

    #[test]
    fn test_count_diff_lines_and_weighted_impact() {
        let diff = "--- a/src/zones.rs\n+++ b/src/zones.rs\n@@ -1,2 +1,4 @@\n-old_line\n+new_line_1\n+new_line_2\n+new_line_3\n";
        assert_eq!(count_diff_lines(Some(diff)), 4);

        let ev = make_event("PC-1-Antigravity", EventType::FileWrite, "src/zones.rs", "", Some(diff), 25.0);
        let impact = compute_event_impact(&ev);
        // 5.0 + 4 * 0.15 = 5.6
        assert_eq!(impact, 5.6);
    }

    #[test]
    fn test_build_contribution_report_multi_agent_aggregation() {
        let events = vec![
            make_event(
                "PC-1-Antigravity",
                EventType::FileWrite,
                "src/zones.rs",
                "",
                Some("+pub struct ContainmentZone;\n+pub enum ZoneKind;\n"),
                45.0,
            ),
            make_event(
                "PC-1",
                EventType::LlmInference,
                "http://127.0.0.1:11434 [qwen3.5:9b]",
                "Generate unit test for safe zone policy",
                None,
                850.0,
            ),
            make_event(
                "PC-2",
                EventType::LlmInference,
                "http://192.168.1.102:8642 [deepseek-r1:1.5b]",
                "Verify safe zone path traversal regex",
                None,
                210.0,
            ),
            make_event(
                "PC-1-Copilot",
                EventType::CommandRun,
                "cargo test --lib",
                "cargo test",
                None,
                320.0,
            ),
        ];

        let report = build_contribution_report(&events);
        assert_eq!(report.total_events_analyzed, 4);
        assert_eq!(report.agents.len(), 4);

        // Verify share percentages sum to ~100%
        let total_pct: f64 = report.agents.iter().map(|a| a.share_pct).sum();
        assert!((total_pct - 100.0).abs() < 1.0);

        // Safe Zones topic should have 3 events (Antigravity write + Qwen test prompt + DeepSeek verify prompt)
        let sz_topic = report
            .feature_topics
            .iter()
            .find(|t| t.topic == "Safe Zones & Policy Firewall")
            .expect("Safe Zones topic should exist");
        assert_eq!(sz_topic.total_touchpoints, 3);
        assert_eq!(sz_topic.top_contributor, "PC-1-Antigravity");
    }
}
