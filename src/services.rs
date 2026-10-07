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
            let pc2_url = "http://192.168.1.102:8642";
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
                                reg.register_single_model("PC-2", pc2_url, id, None);
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
                "cluster": "vector-mesh",
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

#[cfg(test)]
mod tests {

    #[test]
    fn test_beacon_json_schema() {
        let lead_ip = "192.168.1.101";
        let lead_port = 8080;
        let beacon = serde_json::json!({
            "service": "agent_trace_gw",
            "cluster": "vector-mesh",
            "role": "lead",
            "lead_ip": lead_ip,
            "lead_port": lead_port
        });
        assert_eq!(beacon["service"], "agent_trace_gw");
        assert_eq!(beacon["lead_ip"], "192.168.1.101");
        assert_eq!(beacon["lead_port"], 8080);
    }
}
