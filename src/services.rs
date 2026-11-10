//! Background services: live model discovery for the routing registry.

use crate::{
    registry::{ClusterModelRegistry, ModelCapability},
    secrets::ClusterSecurityConfig,
};
use reqwest::Client;
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;

/// Parses `WORKER_UPSTREAMS`, e.g. `PC-2=http://192.168.1.102:8642,GPU=http://10.0.0.5:11434`.
pub fn parse_worker_upstreams(raw: &str) -> Vec<(String, String)> {
    raw.split(',')
        .filter_map(|item| {
            let (node, url) = item.trim().split_once('=')?;
            let (node, url) = (node.trim(), url.trim().trim_end_matches('/'));
            (!node.is_empty() && !url.is_empty()).then(|| (node.to_string(), url.to_string()))
        })
        .collect()
}

/// Every 15s: ingests the local Ollama model list (`/api/tags`) and each worker's
/// OpenAI-compatible `/v1/models`, then prunes models that stopped answering.
pub fn spawn_model_introspector(
    registry: Arc<RwLock<ClusterModelRegistry>>,
    client: Client,
    upstream_url: String,
    workers: Vec<(String, String)>,
    security: Arc<ClusterSecurityConfig>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let tags_url = format!("{}/api/tags", upstream_url.trim_end_matches('/'));
            if let Ok(resp) = client.get(&tags_url).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    registry.write().await.ingest_ollama_tags("LOCAL", &upstream_url, &json);
                }
            }

            for (node_id, base) in &workers {
                if !security.is_upstream_allowed(base) {
                    tracing::warn!("worker {} ({}) is not in UPSTREAM_ALLOWLIST; skipping", node_id, base);
                    continue;
                }
                let mut req = client.get(format!("{}/v1/models", base));
                if let Some(ref key) = security.worker_hermes_key {
                    req = req.header("authorization", format!("Bearer {}", key));
                }
                let Ok(resp) = req.send().await else { continue };
                let Ok(json) = resp.json::<serde_json::Value>().await else { continue };
                if let Some(models) = json.get("data").and_then(|v| v.as_array()) {
                    let mut reg = registry.write().await;
                    for id in models.iter().filter_map(|m| m.get("id").and_then(|v| v.as_str())) {
                        reg.register_single_model(node_id, base, id, Some(vec![ModelCapability::SanityVerification]));
                    }
                }
            }

            registry.write().await.prune_stale_nodes(60);
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_worker_list() {
        let w = parse_worker_upstreams(" PC-2=http://192.168.1.102:8642/ , bad, =x, GPU=http://10.0.0.5:11434");
        assert_eq!(w, vec![
            ("PC-2".to_string(), "http://192.168.1.102:8642".to_string()),
            ("GPU".to_string(), "http://10.0.0.5:11434".to_string()),
        ]);
        assert!(parse_worker_upstreams("").is_empty());
    }
}
