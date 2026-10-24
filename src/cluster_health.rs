//! Autonomous Cluster Health & Sleep Mode Watchdog for AgentTrace Gateway.
//!
//! Actively probes distributed cluster nodes (Edge Gateways and LLM engines),
//! detects network disconnections, Windows sleep mode/hibernation, and powered-off nodes,
//! automatically updates the database and Prompt Hub router, and prevents deadlocks
//! or prolonged timeouts when routing tasks to asleep nodes.

use crate::{
    db::Database,
    registry::ClusterModelRegistry,
    schema::NodeHeartbeat,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{net::TcpStream, sync::{broadcast, RwLock}, time::timeout};
use tracing::{info, warn};

/// Operational health and power state of a cluster node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Actively responding to probes and ready for inference/task execution.
    Online,
    /// Previously active node whose ports stopped responding (Windows modern standby / sleep / hibernation).
    Sleeping,
    /// Node is unreachable, powered down, or network link is severed for multiple cycles.
    Offline,
    /// Edge gateway (:8080) is responsive but inference engine (:8642/:11434) is down, or vice versa.
    Degraded,
}

impl HealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Online => "ACTIVE",
            HealthStatus::Sleeping => "SLEEPING",
            HealthStatus::Offline => "OFFLINE",
            HealthStatus::Degraded => "DEGRADED",
        }
    }
}

/// Result of probing a single TCP port on a cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortProbeResult {
    pub port: u16,
    pub service: String,
    pub open: bool,
    pub latency_ms: Option<f64>,
}

/// Comprehensive health report for an individual cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealthReport {
    pub node_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub status: HealthStatus,
    pub latency_ms: Option<f64>,
    pub ports: Vec<PortProbeResult>,
    pub last_probe_epoch: i64,
    pub last_seen_rfc3339: String,
    pub failure_reason: Option<String>,
    pub consecutive_failures: u32,
    pub sleep_detected: bool,
}

/// Cluster-wide aggregated health snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterHealthSnapshot {
    pub timestamp_utc: String,
    pub total_nodes: usize,
    pub online_nodes: usize,
    pub sleeping_nodes: usize,
    pub offline_nodes: usize,
    pub degraded_nodes: usize,
    pub nodes: Vec<NodeHealthReport>,
}

/// Autonomous Health Watchdog coordinator.
pub struct ClusterHealthWatchdog {
    reports: Arc<RwLock<HashMap<String, NodeHealthReport>>>,
}

impl ClusterHealthWatchdog {
    pub fn new() -> Self {
        Self {
            reports: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Probes a single TCP port with a fast asynchronous non-blocking timeout.
    pub async fn probe_port(ip: &str, port: u16, service: &str, timeout_ms: u64) -> PortProbeResult {
        let addr_str = format!("{}:{}", ip, port);
        let start = Instant::now();

        let target_addr: Result<SocketAddr, _> = addr_str.parse();
        let open = match target_addr {
            Ok(sock) => {
                match timeout(Duration::from_millis(timeout_ms), TcpStream::connect(sock)).await {
                    Ok(Ok(_stream)) => true,
                    _ => false,
                }
            }
            Err(_) => false,
        };

        let latency_ms = if open {
            Some((start.elapsed().as_micros() as f64) / 1000.0)
        } else {
            None
        };

        PortProbeResult {
            port,
            service: service.to_string(),
            open,
            latency_ms,
        }
    }

    /// Probes all standard ports for a cluster node and determines its health/sleep status.
    pub async fn probe_node(
        node: &NodeHeartbeat,
        prev_report: Option<&NodeHealthReport>,
        probe_timeout_ms: u64,
    ) -> NodeHealthReport {
        let ip = &node.ip_address;
        let is_localhost = ip == "127.0.0.1" || ip == "localhost";

        // Ports to probe based on node role / standard ports:
        // 8080: AgentTrace Edge Gateway
        // 8642: Hermes Agent / CUDA Engine
        // 11434: Ollama Local Engine
        let mut port_probes = Vec::new();

        if is_localhost {
            // Localhost node (PC-1 / Lead)
            let p_gw = Self::probe_port("127.0.0.1", 8080, "AgentTrace Gateway", probe_timeout_ms).await;
            let p_ol = Self::probe_port("127.0.0.1", 11434, "Ollama Engine", probe_timeout_ms).await;
            port_probes.push(p_gw);
            port_probes.push(p_ol);
        } else {
            let p_gw = Self::probe_port(ip, 8080, "AgentTrace Gateway", probe_timeout_ms).await;
            let p_hermes = Self::probe_port(ip, 8642, "Hermes Agent / CUDA", probe_timeout_ms).await;
            let p_ol = Self::probe_port(ip, 11434, "Ollama Engine", probe_timeout_ms).await;
            port_probes.push(p_gw);
            port_probes.push(p_hermes);
            port_probes.push(p_ol);
        }

        let any_open = port_probes.iter().any(|p| p.open);
        let gw_open = port_probes.iter().find(|p| p.port == 8080).map(|p| p.open).unwrap_or(false);
        let llm_open = port_probes.iter().find(|p| p.port != 8080).map(|p| p.open).unwrap_or(false);

        let min_latency = port_probes
            .iter()
            .filter_map(|p| p.latency_ms)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let prev_consecutive = prev_report.map(|r| r.consecutive_failures).unwrap_or(0);
        let prev_status = prev_report.map(|r| r.status).unwrap_or(HealthStatus::Online);

        let (status, consecutive_failures, failure_reason, sleep_detected) = if any_open {
            if gw_open && llm_open {
                (HealthStatus::Online, 0, None, false)
            } else if gw_open || llm_open {
                let reason = if gw_open {
                    "Gateway active (:8080) but LLM engine closed".to_string()
                } else {
                    "LLM engine active but Edge Gateway (:8080) closed".to_string()
                };
                (HealthStatus::Degraded, 0, Some(reason), false)
            } else {
                (HealthStatus::Online, 0, None, false)
            }
        } else {
            // Node is completely unreachable
            let new_failures = prev_consecutive + 1;
            // Sleep Detection Invariant:
            // If the node was previously Online or in Standby within the last 3-5 cycles,
            // non-responsive TCP connections typically mean Windows Modern Standby / Sleep / Hibernation.
            if new_failures <= 3 && (prev_status == HealthStatus::Online || prev_status == HealthStatus::Sleeping) {
                (
                    HealthStatus::Sleeping,
                    new_failures,
                    Some(format!("Node entered Sleep/Standby mode (probe failed {} cycle(s))", new_failures)),
                    true,
                )
            } else {
                (
                    HealthStatus::Offline,
                    new_failures,
                    Some(format!("Node is Offline or powered off (failed {} consecutive cycles)", new_failures)),
                    false,
                )
            }
        };

        let now_epoch = Utc::now().timestamp();
        let last_seen_rfc3339 = if any_open {
            Utc::now().to_rfc3339()
        } else {
            prev_report
                .map(|r| r.last_seen_rfc3339.clone())
                .unwrap_or_else(|| node.last_seen.clone())
        };

        NodeHealthReport {
            node_id: node.node_id.clone(),
            hostname: node.hostname.clone(),
            ip_address: node.ip_address.clone(),
            status,
            latency_ms: min_latency,
            ports: port_probes,
            last_probe_epoch: now_epoch,
            last_seen_rfc3339,
            failure_reason,
            consecutive_failures,
            sleep_detected,
        }
    }

    /// Executes an immediate health probe across all known nodes in the database.
    pub async fn probe_all(
        &self,
        db: &Database,
        registry: &Arc<RwLock<ClusterModelRegistry>>,
        tx_events: Option<&broadcast::Sender<String>>,
    ) -> ClusterHealthSnapshot {
        let nodes = db.get_all_nodes().unwrap_or_default();
        let prev_map = self.reports.read().await.clone();

        let mut probe_futs = Vec::new();
        for node in &nodes {
            let prev = prev_map.get(&node.node_id).cloned();
            probe_futs.push(async move {
                Self::probe_node(&node, prev.as_ref(), 750).await
            });
        }

        let results = futures_util::future::join_all(probe_futs).await;

        let mut new_map = HashMap::new();
        let mut online_count = 0;
        let mut sleeping_count = 0;
        let mut offline_count = 0;
        let mut degraded_count = 0;

        for rep in results {
            let prev_status = prev_map.get(&rep.node_id).map(|r| r.status);

            match rep.status {
                HealthStatus::Online => online_count += 1,
                HealthStatus::Sleeping => sleeping_count += 1,
                HealthStatus::Offline => offline_count += 1,
                HealthStatus::Degraded => degraded_count += 1,
            }

            // 1. Sync status to database `nodes` table
            let db_status = rep.status.as_str();
            let _ = db.update_node_status(&rep.node_id, db_status, &rep.last_seen_rfc3339);

            // 2. Sync to ClusterModelRegistry for Prompt Hub router
            {
                let mut reg = registry.write().await;
                let is_available = rep.status == HealthStatus::Online || rep.status == HealthStatus::Degraded;
                reg.set_node_models_health(
                    &rep.node_id,
                    is_available,
                    rep.last_probe_epoch,
                    rep.latency_ms.map(|l| l as u64),
                );
            }

            // 3. Log and broadcast transitions (e.g. Online -> Sleeping, Sleeping -> Online)
            if let Some(old_status) = prev_status {
                if old_status != rep.status {
                    if rep.status == HealthStatus::Sleeping {
                        warn!(
                            "[ClusterHealth] Node {} ({}) entered SLEEP MODE. Routing paused for this node.",
                            rep.node_id, rep.ip_address
                        );
                    } else if rep.status == HealthStatus::Online && old_status == HealthStatus::Sleeping {
                        info!(
                            "[ClusterHealth] Node {} ({}) RESUMED FROM SLEEP. Latency: {:.1}ms. Routing restored.",
                            rep.node_id, rep.ip_address, rep.latency_ms.unwrap_or(0.0)
                        );
                    } else if rep.status == HealthStatus::Offline {
                        warn!(
                            "[ClusterHealth] Node {} ({}) is now OFFLINE.",
                            rep.node_id, rep.ip_address
                        );
                    }

                    if let Some(tx) = tx_events {
                        let payload = serde_json::json!({
                            "type": "cluster_node_health_change",
                            "node_id": rep.node_id,
                            "previous_status": old_status.as_str(),
                            "new_status": rep.status.as_str(),
                            "ip_address": rep.ip_address,
                            "timestamp": rep.last_seen_rfc3339,
                            "latency_ms": rep.latency_ms,
                        });
                        let _ = tx.send(payload.to_string());
                    }
                }
            }

            new_map.insert(rep.node_id.clone(), rep);
        }

        let snapshot = ClusterHealthSnapshot {
            timestamp_utc: Utc::now().to_rfc3339(),
            total_nodes: new_map.len(),
            online_nodes: online_count,
            sleeping_nodes: sleeping_count,
            offline_nodes: offline_count,
            degraded_nodes: degraded_count,
            nodes: new_map.values().cloned().collect(),
        };

        *self.reports.write().await = new_map;

        snapshot
    }

    /// Returns the current cached health snapshot without triggering a new network probe.
    pub async fn get_cached_snapshot(&self) -> ClusterHealthSnapshot {
        let map = self.reports.read().await;
        let mut online_count = 0;
        let mut sleeping_count = 0;
        let mut offline_count = 0;
        let mut degraded_count = 0;

        for rep in map.values() {
            match rep.status {
                HealthStatus::Online => online_count += 1,
                HealthStatus::Sleeping => sleeping_count += 1,
                HealthStatus::Offline => offline_count += 1,
                HealthStatus::Degraded => degraded_count += 1,
            }
        }

        ClusterHealthSnapshot {
            timestamp_utc: Utc::now().to_rfc3339(),
            total_nodes: map.len(),
            online_nodes: online_count,
            sleeping_nodes: sleeping_count,
            offline_nodes: offline_count,
            degraded_nodes: degraded_count,
            nodes: map.values().cloned().collect(),
        }
    }

    /// Spawns the autonomous background health watchdog task.
    pub fn spawn(
        db: Database,
        registry: Arc<RwLock<ClusterModelRegistry>>,
        tx_events: broadcast::Sender<String>,
        interval: Duration,
    ) -> Arc<Self> {
        let watchdog = Arc::new(Self::new());
        let wd_clone = watchdog.clone();

        tokio::spawn(async move {
            info!("[ClusterHealth] Autonomous Health & Sleep Watchdog started (interval: {:?})", interval);
            // Run initial probe after a brief startup grace period
            tokio::time::sleep(Duration::from_millis(1500)).await;

            loop {
                wd_clone.probe_all(&db, &registry, Some(&tx_events)).await;
                tokio::time::sleep(interval).await;
            }
        });

        watchdog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_strings() {
        assert_eq!(HealthStatus::Online.as_str(), "ACTIVE");
        assert_eq!(HealthStatus::Sleeping.as_str(), "SLEEPING");
        assert_eq!(HealthStatus::Offline.as_str(), "OFFLINE");
        assert_eq!(HealthStatus::Degraded.as_str(), "DEGRADED");
    }

    #[tokio::test]
    async fn test_node_probe_status_transition() {
        let node = NodeHeartbeat {
            node_id: "TEST-PC".to_string(),
            hostname: "NODE-TEST".to_string(),
            ip_address: "192.0.2.1".to_string(), // RFC 5737 TEST-NET (non-routable)
            role: "Worker Node".to_string(),
            status: "ACTIVE".to_string(),
            last_seen: Utc::now().to_rfc3339(),
            public_key: None,
            models: None,
        };

        // First probe on unreachable IP should be marked Sleeping
        let rep1 = ClusterHealthWatchdog::probe_node(&node, None, 50).await;
        assert_eq!(rep1.status, HealthStatus::Sleeping);
        assert_eq!(rep1.consecutive_failures, 1);
        assert!(rep1.sleep_detected);

        // After multiple consecutive failures, should become Offline
        let mut rep_acc = rep1;
        for _ in 0..4 {
            rep_acc = ClusterHealthWatchdog::probe_node(&node, Some(&rep_acc), 50).await;
        }
        assert_eq!(rep_acc.status, HealthStatus::Offline);
        assert!(rep_acc.consecutive_failures >= 4);
    }
}
