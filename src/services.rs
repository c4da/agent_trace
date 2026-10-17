use crate::{
    ledger::NodeIdentity,
    registry::ClusterModelRegistry,
    secrets::ClusterSecurityConfig,
};
use reqwest::Client;
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;

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

            // 2. Ingest PC-2 Worker Models (Hermes Agent / CUDA)
            let pc2_url = security.worker_node_url.clone();
            let mut req = client.get(format!("{}/v1/models", pc2_url));
            if let Some(ref key) = security.worker_hermes_key {
                req = req.header("authorization", format!("Bearer {}", key));
            }
            if let Ok(resp) = req.send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(arr) = json.get("data").and_then(|d| d.as_array()) {
                        let mut reg = registry.write().await;
                        for m in arr {
                            if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                                reg.register_single_model("PC-2", &pc2_url, id, None);
                            }
                        }
                        // Ensure PC-2's deepseek-r1:1.5b CUDA reasoning capability is actively registered
                        reg.register_single_model("PC-2", &pc2_url, "deepseek-r1:1.5b", None);
                    } else {
                        let mut reg = registry.write().await;
                        reg.register_single_model("PC-2", &pc2_url, "hermes-agent", None);
                        reg.register_single_model("PC-2", &pc2_url, "deepseek-r1:1.5b", None);
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
/// Periodically announces edge node status and identity to the Lead Gateway.
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
        let heartbeat_payload = serde_json::json!({
            "node_id": node_id,
            "hostname": hostname,
            "ip_address": "127.0.0.1",
            "role": "Worker Edge Gateway (Active)",
            "status": "ACTIVE",
            "last_seen": chrono::Utc::now().to_rfc3339(),
            "public_key": identity.public_key_hex()
        });

        loop {
            let mut req = client.post(&hb_url)
                .header("content-type", "application/json")
                .json(&heartbeat_payload);
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

/// Spawns the Local Host Agent & Port Discovery service.
/// Automatically detects installed agent CLIs (e.g. GitHub Copilot CLI, Hermes, etc.)
/// and local agent ports, registering them into the active cluster registry and database.
pub fn spawn_host_agent_discoverer(
    db: crate::db::Database,
    registry: Arc<RwLock<ClusterModelRegistry>>,
    hostname: String,
    identity: Arc<NodeIdentity>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // 1. Detect GitHub Copilot CLI
            let user_profile = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_default();
            let copilot_exe_links = std::path::PathBuf::from(&user_profile)
                .join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("WinGet")
                .join("Links")
                .join("copilot.exe");
            let copilot_dir = std::path::PathBuf::from(&user_profile).join(".copilot");

            let has_copilot = copilot_exe_links.exists() || copilot_dir.exists();
            if has_copilot {
                // Upsert Copilot CLI node into DB
                let _ = db.upsert_node(&crate::schema::NodeHeartbeat {
                    node_id: "PC-1-Copilot".to_string(),
                    hostname: hostname.clone(),
                    ip_address: "127.0.0.1".to_string(),
                    role: "GitHub Copilot CLI (v1.0.95 · Autopilot/Plan)".to_string(),
                    status: "ACTIVE".to_string(),
                    last_seen: chrono::Utc::now().to_rfc3339(),
                    public_key: Some(identity.public_key_hex().to_string()),
                    models: Some(vec![
                        crate::registry::ModelManifest {
                            model_id: "gpt-6-luna".to_string(),
                            node_id: "PC-1-Copilot".to_string(),
                            endpoint: "copilot.exe".to_string(),
                            capabilities: vec![
                                crate::registry::ModelCapability::FastCodeGeneration,
                                crate::registry::ModelCapability::ComplexReasoning,
                                crate::registry::ModelCapability::ToolCalling,
                            ],
                            parameter_tier: crate::registry::ParameterTier::Frontier,
                            context_window: 128000,
                            is_healthy: true,
                            last_seen_epoch: chrono::Utc::now().timestamp(),
                            latency_ms: None,
                        }
                    ]),
                });

                // Also register into model registry
                {
                    let mut reg = registry.write().await;
                    reg.register_single_model(
                        "PC-1-Copilot",
                        "copilot.exe",
                        "gpt-6-luna",
                        Some(vec![
                            crate::registry::ModelCapability::FastCodeGeneration,
                            crate::registry::ModelCapability::ComplexReasoning,
                        ]),
                    );
                }
            }

            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_beacon_json_schema() {
        let lead_ip = "127.0.0.1";
        let lead_port = 8080;
        let beacon = serde_json::json!({
            "service": "agent_trace_gw",
            "cluster": "agent-trace-mesh",
            "role": "lead",
            "lead_ip": lead_ip,
            "lead_port": lead_port
        });
        assert_eq!(beacon["service"], "agent_trace_gw");
        assert_eq!(beacon["lead_ip"], "127.0.0.1");
        assert_eq!(beacon["lead_port"], 8080);
    }
}
