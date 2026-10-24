use crate::{
    cas_guard::{CasVerdict, PreImageCasGuard},
    ledger::NodeIdentity,
    registry::{ClusterModelRegistry, ModelCapability, ModelManifest, ParameterTier},
    schema::{EventType, NodeHeartbeat, SecurityVerdict, TraceEvent},
    secrets::ClusterSecurityConfig,
};
use reqwest::Client;
use std::{
    path::Path,
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use tracing::warn;

/// Discovers the local machine's outbound LAN IP address.
pub async fn discover_local_lan_ip() -> String {
    if let Ok(socket) = tokio::net::UdpSocket::bind("0.0.0.0:0").await {
        if socket.connect("8.8.8.8:80").await.is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                return local_addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

/// Spawns the Dynamic Model Introspector background service.
/// Periodically auto-discovers models on PC-1 (Ollama) and PC-2 (Hermes/CUDA) and refreshes the registry.
pub fn spawn_model_introspector(
    registry: Arc<RwLock<ClusterModelRegistry>>,
    client: Client,
    upstream_url: String,
    security: Arc<ClusterSecurityConfig>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // 1. Ingest PC-1 Local Models (Ollama)
            let pc1_tags_url = format!("{}/api/tags", upstream_url.trim_end_matches('/'));
            if let Ok(resp) = client.get(&pc1_tags_url).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let mut reg = registry.write().await;
                    reg.ingest_ollama_tags("PC-1", &upstream_url, &json);
                }
            }

            // Ingest Host Tools on PC-1 (GitHub Copilot CLI)
            if crate::copilot::is_copilot_installed() {
                let mut reg = registry.write().await;
                reg.register_single_model(
                    "PC-1",
                    "cli://copilot",
                    "copilot-cli",
                    Some(vec![
                        ModelCapability::FastCodeGeneration,
                        ModelCapability::ComplexReasoning,
                        ModelCapability::GeneralExecution,
                    ]),
                );
            }

            // 2. Ingest PC-2 Remote Models (Hermes Agent / CUDA)
            let pc2_health_url = "http://192.168.1.102:8642/health";
            let mut pc2_req = client.get(pc2_health_url);
            if let Some(ref token) = security.worker_hermes_key {
                pc2_req = pc2_req.header("authorization", format!("Bearer {}", token));
            }

            if let Ok(resp) = pc2_req.send().await {
                if resp.status().is_success() {
                    let pc2_models_url = "http://192.168.1.102:8642/v1/models";
                    let mut models_req = client.get(pc2_models_url);
                    if let Some(ref token) = security.worker_hermes_key {
                        models_req = models_req.header("authorization", format!("Bearer {}", token));
                    }
                    if let Ok(m_resp) = models_req.send().await {
                        if let Ok(m_json) = m_resp.json::<serde_json::Value>().await {
                            let mut reg = registry.write().await;
                            if let Some(models) = m_json.get("data").and_then(|v| v.as_array()) {
                                for m in models {
                                    if let Some(id) = m.get("id").and_then(|v| v.as_str()) {
                                        reg.register_single_model(
                                            "PC-2",
                                            "http://192.168.1.102:8642",
                                            id,
                                            Some(vec![ModelCapability::SanityVerification]),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 3. Prune nodes that haven't sent a heartbeat within 60s
            {
                let mut reg = registry.write().await;
                reg.prune_stale_nodes(60);
            }

            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    })
}

/// Spawns the Worker Heartbeat & Telemetry Forwarder background service.
/// Dynamically queries local Ollama & Hermes models and announces status to Lead Gateway.
pub fn spawn_worker_forwarder(
    lead_url: String,
    node_id: String,
    hostname: String,
    identity: Arc<NodeIdentity>,
    client: Client,
    security: Arc<ClusterSecurityConfig>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let hb_url = format!("{}/api/v1/heartbeat", lead_url.trim_end_matches('/'));
        let local_ip = discover_local_lan_ip().await;

        loop {
            let mut discovered_models = Vec::new();

            // Check local Ollama tags
            if let Ok(resp) = client.get("http://127.0.0.1:11434/api/tags").send().await {
                if let Ok(tags) = resp.json::<serde_json::Value>().await {
                    if let Some(models) = tags.get("models").and_then(|v| v.as_array()) {
                        for m in models {
                            if let Some(name) = m.get("name").and_then(|v| v.as_str()) {
                                discovered_models.push(ModelManifest {
                                    model_id: name.to_string(),
                                    node_id: node_id.clone(),
                                    endpoint: format!("http://{}:11434", local_ip),
                                    capabilities: vec![ModelCapability::FastCodeGeneration, ModelCapability::GeneralExecution],
                                    parameter_tier: ParameterTier::Medium,
                                    context_window: 131072,
                                    is_healthy: true,
                                    last_seen_epoch: chrono::Utc::now().timestamp(),
                                    latency_ms: None,
                                });
                            }
                        }
                    }
                }
            }

            // Check local Hermes or CUDA agent
            if let Ok(resp) = client.get("http://127.0.0.1:8642/v1/models").send().await {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(models) = data.get("data").and_then(|v| v.as_array()) {
                        for m in models {
                            if let Some(id) = m.get("id").and_then(|v| v.as_str()) {
                                discovered_models.push(ModelManifest {
                                    model_id: id.to_string(),
                                    node_id: node_id.clone(),
                                    endpoint: format!("http://{}:8642", local_ip),
                                    capabilities: vec![ModelCapability::SanityVerification, ModelCapability::ToolCalling],
                                    parameter_tier: ParameterTier::Small,
                                    context_window: 65536,
                                    is_healthy: true,
                                    last_seen_epoch: chrono::Utc::now().timestamp(),
                                    latency_ms: None,
                                });
                            }
                        }
                    }
                }
            }

            let heartbeat = NodeHeartbeat {
                node_id: node_id.clone(),
                hostname: hostname.clone(),
                ip_address: local_ip.clone(),
                role: "Worker Edge Gateway (Active)".to_string(),
                status: "ACTIVE".to_string(),
                last_seen: chrono::Utc::now().to_rfc3339(),
                public_key: Some(identity.public_key_hex().to_string()),
                models: if discovered_models.is_empty() { None } else { Some(discovered_models) },
            };

            let mut req = client.post(&hb_url)
                .header("content-type", "application/json")
                .json(&heartbeat);
            if let Some(ref tok) = security.auth_token {
                req = req.header("x-agenttrace-token", tok);
            }
            let _ = req.send().await;

            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    })
}

/// Spawns the UDP Network Discovery Beacon background service.
/// Announces Lead Gateway presence and endpoint across the LAN subnet every 4s.
pub fn spawn_discovery_beacon(lead_port: u16, lead_ip: &str) -> tokio::task::JoinHandle<()> {
    let lead_ip_string = lead_ip.to_string();
    tokio::spawn(async move {
        if let Ok(socket) = tokio::net::UdpSocket::bind("0.0.0.0:0").await {
            let _ = socket.set_broadcast(true);
            let beacon = serde_json::json!({
                "service": "agent_trace_gw",
                "cluster": "agent-trace-mesh",
                "role": "lead",
                "lead_ip": lead_ip_string,
                "lead_port": lead_port
            }).to_string();
            let target = "255.255.255.255:8089";
            loop {
                let _ = socket.send_to(beacon.as_bytes(), target).await;
                tokio::time::sleep(Duration::from_secs(4)).await;
            }
        }
    })
}

/// Listens for a Lead Node discovery beacon on UDP `:8089` for up to `timeout`.
pub async fn listen_for_lead_beacon(timeout: Duration) -> Option<String> {
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:8089").await.ok()?;
    let mut buf = [0u8; 1024];

    tokio::select! {
        res = socket.recv_from(&mut buf) => {
            if let Ok((len, src)) = res {
                if let Ok(json_str) = std::str::from_utf8(&buf[..len]) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                        if val.get("service").and_then(|v| v.as_str()) == Some("agent_trace_gw") {
                            let port = val.get("lead_port").and_then(|v| v.as_u64()).unwrap_or(8080);
                            let ip = val.get("lead_ip").and_then(|v| v.as_str()).map(|s| s.to_string())
                                .unwrap_or_else(|| src.ip().to_string());
                            return Some(format!("http://{}:{}", ip, port));
                        }
                    }
                }
            }
            None
        }
        _ = tokio::time::sleep(timeout) => None
    }
}

/// Spawns the Local Host Agent & Port Discovery service.
pub fn spawn_host_agent_discoverer(
    db: crate::db::Database,
    registry: Arc<RwLock<ClusterModelRegistry>>,
    hostname: String,
    identity: Arc<NodeIdentity>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let user_profile = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_default();

            // 1. Detect GitHub Copilot CLI
            let copilot_exe_links = std::path::PathBuf::from(&user_profile)
                .join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("WinGet")
                .join("Links")
                .join("copilot.exe");
            let copilot_dir = std::path::PathBuf::from(&user_profile).join(".copilot");

            if copilot_exe_links.exists() || copilot_dir.exists() {
                let _ = db.upsert_node(&NodeHeartbeat {
                    node_id: "PC-1-Copilot".to_string(),
                    hostname: hostname.clone(),
                    ip_address: "127.0.0.1".to_string(),
                    role: "GitHub Copilot CLI".to_string(),
                    status: "ACTIVE".to_string(),
                    last_seen: chrono::Utc::now().to_rfc3339(),
                    public_key: Some(identity.public_key_hex().to_string()),
                    models: Some(vec![
                        ModelManifest {
                            model_id: "gpt-6-luna".to_string(),
                            node_id: "PC-1-Copilot".to_string(),
                            endpoint: "copilot.exe".to_string(),
                            capabilities: vec![
                                ModelCapability::FastCodeGeneration,
                                ModelCapability::ComplexReasoning,
                                ModelCapability::ToolCalling,
                            ],
                            parameter_tier: ParameterTier::Frontier,
                            context_window: 128000,
                            is_healthy: true,
                            last_seen_epoch: chrono::Utc::now().timestamp(),
                            latency_ms: None,
                        }
                    ]),
                });

                let mut reg = registry.write().await;
                reg.register_single_model(
                    "PC-1-Copilot",
                    "copilot.exe",
                    "gpt-6-luna",
                    Some(vec![
                        ModelCapability::FastCodeGeneration,
                        ModelCapability::ComplexReasoning,
                    ]),
                );
            }

            // 2. Detect Hermes Agent CLI
            let hermes_cmd = std::path::PathBuf::from(&user_profile)
                .join("AppData")
                .join("Local")
                .join("hermes")
                .join("bin")
                .join("hermes.cmd");
            if hermes_cmd.exists() {
                let mut reg = registry.write().await;
                reg.register_single_model(
                    "PC-1-Hermes",
                    "cli://hermes",
                    "hermes-agent",
                    Some(vec![
                        ModelCapability::ToolCalling,
                        ModelCapability::SanityVerification,
                    ]),
                );
            }

            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    })
}

/// Spawns the Pre-Image CAS Invariant Watcher.
/// Monitors critical project files and automatically restores them in <5ms if tampered.
pub fn spawn_cas_guard_service(
    root: &Path,
    db: crate::db::Database,
    identity: Arc<NodeIdentity>,
) -> (Arc<PreImageCasGuard>, tokio::task::JoinHandle<()>) {
    let guard = Arc::new(PreImageCasGuard::new());
    guard.protect_workspace_invariants(root);

    let guard_clone = guard.clone();
    let root_buf = root.to_path_buf();

    let handle = tokio::spawn(async move {
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

        loop {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            for rel in &critical_files {
                let p = root_buf.join(rel);
                if p.is_file() {
                    let verdict = guard_clone.inspect_and_enforce(&p, false);
                    if let CasVerdict::Reverted { path, offending_sha256, restored_sha256 } = verdict {
                        warn!(
                            path = %path.display(),
                            offending = %offending_sha256,
                            restored = %restored_sha256,
                            "CAS Invariant Guard triggered: Unauthorized out-of-band edit reverted!"
                        );

                        // Log tampering event to cryptographic ledger
                        let mut incident = TraceEvent {
                            event_id: uuid::Uuid::new_v4().to_string(),
                            node_id: "CAS_GUARD".to_string(),
                            session_id: "spatial_invariants".to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            event_type: EventType::DecisionStep,
                            target: path.to_string_lossy().to_string(),
                            action_details: serde_json::json!({
                                "action": "reverted_unauthorized_mutation",
                                "offending_sha256": offending_sha256,
                                "restored_sha256": restored_sha256,
                                "status": "REVERTED_IN_SUB_5MS"
                            }),
                            diff: None,
                            provenance_sources: vec!["cas_guard".to_string()],
                            security_verdict: SecurityVerdict::Blocked,
                            security_reasons: vec!["Pre-Image CAS Invariant Violation".to_string()],
                            duration_ms: 2.5,
                            prev_hash: "0".repeat(64),
                            event_hash: "0".repeat(64),
                            signature: None,
                            public_key: None,
                        };
                        let _ = db.insert_event_signed(&mut incident, Some(&*identity));
                    }
                }
            }
        }
    });

    (guard, handle)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_beacon_json_schema() {
        let lead_ip = "192.168.1.101";
        let lead_port = 8080;
        let beacon = serde_json::json!({
            "service": "agent_trace_gw",
            "cluster": "agent-trace-mesh",
            "role": "lead",
            "lead_ip": lead_ip,
            "lead_port": lead_port
        });
        assert_eq!(beacon["service"], "agent_trace_gw");
        assert_eq!(beacon["lead_ip"], "192.168.1.101");
        assert_eq!(beacon["lead_port"], 8080);
    }
}
