//! Dynamic Capability-Driven Model Registry for AgentTrace Gateway.
//!
//! Enables zero-configuration cluster orchestration: nodes self-advertise their deployed
//! models and capabilities via heartbeats, or the gateway auto-derives them from runtime
//! metadata (parameter sizes, model families, and tags). The lead node does not need
//! hardcoded model lists or static node mappings.

use crate::router::RoutingIntent;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Fundamental capabilities that a model or worker node can provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelCapability {
    /// Deep architecture design, complex refactoring, multi-file planning, physics, math.
    ComplexReasoning,
    /// High-throughput code generation, unit tests (cargo test), boilerplate, telemetry parsers.
    FastCodeGeneration,
    /// Ultra-fast sanity verification, sub-second regex validation, assertions, lint audits.
    SanityVerification,
    /// General instruction following, offline execution, and general summarization.
    GeneralExecution,
    /// Multimodal UI / viewport screenshot analysis.
    MultimodalVision,
    /// Whole-repository sweeps, large context replay.
    LongContextAnalysis,
    /// Autonomous tool calling and function execution.
    ToolCalling,
}

/// Coarse parameter size tier of the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterTier {
    /// < 3B parameters (optimized for ultra-fast GPU verification).
    Small,
    /// 3B - 16B parameters (balanced speed & code generation).
    Medium,
    /// 16B - 40B parameters (heavy reasoning & architecture).
    Large,
    /// > 40B parameters (frontier reasoning).
    Frontier,
}

/// Metadata describing a model deployed on a specific cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    /// Model identifier (e.g. "qwen3.8:27b", "deepseek-r1:1.5b", "gemma4:latest").
    pub model_id: String,
    /// Host node identifier where the model resides (e.g. "PC-1", "PC-2", "NODE-03").
    pub node_id: String,
    /// Upstream base URL for dispatch (e.g. "http://192.168.1.102:8642" or "http://127.0.0.1:11434").
    pub endpoint: String,
    /// Capabilities provided by this model.
    pub capabilities: Vec<ModelCapability>,
    /// Parameter size tier.
    pub parameter_tier: ParameterTier,
    /// Context window limit in tokens.
    pub context_window: usize,
    /// Whether the model is currently healthy and online.
    pub is_healthy: bool,
    /// Unix timestamp of the last heartbeat announcement.
    pub last_seen_epoch: i64,
    /// Optional observed round-trip latency in milliseconds.
    pub latency_ms: Option<u64>,
}

/// Auto-derives capabilities and tier from raw model metadata.
///
/// Introspects model names, parameter strings (e.g. "27.3B", "1.5B"), families, and tags.
pub fn auto_derive_capabilities(
    model_id: &str,
    param_str: Option<&str>,
    family: Option<&str>,
    tags: &[String],
) -> (Vec<ModelCapability>, ParameterTier) {
    let lower_name = model_id.to_lowercase();
    let lower_fam = family.map(|f| f.to_lowercase()).unwrap_or_default();
    let mut caps = Vec::new();

    // 1. Determine Parameter Tier
    let tier = if let Some(p) = param_str {
        let p_clean = p.to_lowercase();
        if p_clean.contains("70b") || p_clean.contains("405b") || p_clean.contains("120b") {
            ParameterTier::Frontier
        } else if p_clean.contains("27") || p_clean.contains("32b") || p_clean.contains("30b") {
            ParameterTier::Large
        } else if p_clean.contains("7b") || p_clean.contains("8b") || p_clean.contains("14b") || p_clean.contains("9b") {
            ParameterTier::Medium
        } else {
            ParameterTier::Small
        }
    } else if lower_name.contains("27b") || lower_name.contains("32b") {
        ParameterTier::Large
    } else if lower_name.contains("14b") || lower_name.contains("8b") || lower_name.contains("7b") {
        ParameterTier::Medium
    } else if lower_name.contains("1.5b") || lower_name.contains("1b") || lower_name.contains("3b") {
        ParameterTier::Small
    } else {
        ParameterTier::Medium
    };

    // 2. Derive Capabilities
    let has_thinking = tags.iter().any(|t| t.to_lowercase().contains("thinking"))
        || lower_name.contains("r1")
        || lower_fam.contains("deepseek");

    let has_vision = tags.iter().any(|t| t.to_lowercase().contains("vision"))
        || lower_name.contains("vl")
        || lower_name.contains("vision");

    let has_tools = tags.iter().any(|t| t.to_lowercase().contains("tools"))
        || lower_name.contains("hermes");

    if has_vision {
        caps.push(ModelCapability::MultimodalVision);
    }
    if has_tools {
        caps.push(ModelCapability::ToolCalling);
    }

    match tier {
        ParameterTier::Frontier | ParameterTier::Large => {
            caps.push(ModelCapability::ComplexReasoning);
            caps.push(ModelCapability::FastCodeGeneration);
            caps.push(ModelCapability::GeneralExecution);
        }
        ParameterTier::Medium => {
            caps.push(ModelCapability::FastCodeGeneration);
            caps.push(ModelCapability::GeneralExecution);
            if has_thinking {
                caps.push(ModelCapability::ComplexReasoning);
            }
        }
        ParameterTier::Small => {
            if has_thinking {
                caps.push(ModelCapability::SanityVerification);
            }
            caps.push(ModelCapability::GeneralExecution);
        }
    }

    // Explicit heuristics for known families
    if ((lower_name.contains("deepseek") && lower_name.contains("r1")) || lower_name.contains("r1:1.5b"))
        && !caps.contains(&ModelCapability::SanityVerification)
    {
        caps.push(ModelCapability::SanityVerification);
    }

    if caps.is_empty() {
        caps.push(ModelCapability::GeneralExecution);
    }

    (caps, tier)
}

/// Dynamic registry of all models currently active across cluster nodes.
#[derive(Debug, Clone, Default)]
pub struct ClusterModelRegistry {
    /// Map of model_key ("node_id:model_id") to model manifest.
    models: HashMap<String, ModelManifest>,
}

impl ClusterModelRegistry {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    /// Registers or updates models announced by a cluster node.
    pub fn register_models(&mut self, manifests: Vec<ModelManifest>) {
        for m in manifests {
            let key = format!("{}:{}", m.node_id, m.model_id);
            self.models.insert(key, m);
        }
    }

    /// Ingests models from an Ollama `/api/tags` JSON payload.
    pub fn ingest_ollama_tags(
        &mut self,
        node_id: &str,
        endpoint: &str,
        tags_json: &serde_json::Value,
    ) {
        let now = Utc::now().timestamp();
        if let Some(models_arr) = tags_json.get("models").and_then(|m| m.as_array()) {
            for item in models_arr {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    let param_size = item
                        .get("details")
                        .and_then(|d| d.get("parameter_size"))
                        .and_then(|p| p.as_str());
                    let family = item
                        .get("details")
                        .and_then(|d| d.get("family"))
                        .and_then(|f| f.as_str());
                    let mut tags = Vec::new();
                    if let Some(caps_arr) = item.get("capabilities").and_then(|c| c.as_array()) {
                        for c in caps_arr {
                            if let Some(s) = c.as_str() {
                                tags.push(s.to_string());
                            }
                        }
                    }

                    let (capabilities, parameter_tier) =
                        auto_derive_capabilities(name, param_size, family, &tags);

                    let manifest = ModelManifest {
                        model_id: name.to_string(),
                        node_id: node_id.to_string(),
                        endpoint: endpoint.trim_end_matches('/').to_string(),
                        capabilities,
                        parameter_tier,
                        context_window: 65536,
                        is_healthy: true,
                        last_seen_epoch: now,
                        latency_ms: None,
                    };

                    let key = format!("{}:{}", node_id, name);
                    self.models.insert(key, manifest);
                }
            }
        }
    }

    /// Registers a single custom model (e.g. from Hermes or edge worker).
    pub fn register_single_model(
        &mut self,
        node_id: &str,
        endpoint: &str,
        model_id: &str,
        custom_caps: Option<Vec<ModelCapability>>,
    ) {
        let now = Utc::now().timestamp();
        let (caps, tier) = if let Some(c) = custom_caps {
            (c, ParameterTier::Medium)
        } else {
            auto_derive_capabilities(model_id, None, None, &[])
        };

        let manifest = ModelManifest {
            model_id: model_id.to_string(),
            node_id: node_id.to_string(),
            endpoint: endpoint.trim_end_matches('/').to_string(),
            capabilities: caps,
            parameter_tier: tier,
            context_window: 32768,
            is_healthy: true,
            last_seen_epoch: now,
            latency_ms: None,
        };

        let key = format!("{}:{}", node_id, model_id);
        self.models.insert(key, manifest);
    }

    /// Finds an exact match for an explicitly requested model ID across any healthy node.
    pub fn find_model(&self, model_id: &str) -> Option<ModelManifest> {
        let lower = model_id.to_lowercase();
        // 1. Exact match on model_id
        if let Some(m) = self.models.values().find(|m| m.is_healthy && m.model_id.to_lowercase() == lower) {
            return Some(m.clone());
        }
        // 2. Substring match (e.g. "qwen3.8" matches "qwen3.8:27b")
        if let Some(m) = self.models.values().find(|m| m.is_healthy && (m.model_id.to_lowercase().contains(&lower) || lower.contains(&m.model_id.to_lowercase()))) {
            return Some(m.clone());
        }
        None
    }

    /// Resolves the optimal candidate model matching a classified RoutingIntent.
    ///
    /// Evaluates healthy models matching the required capability, ranking them by parameter tier.
    pub fn find_best_candidate(&self, intent: &RoutingIntent) -> Option<ModelManifest> {
        let target_cap = match intent {
            RoutingIntent::ComplexReasoning => ModelCapability::ComplexReasoning,
            RoutingIntent::FastCodeGeneration => ModelCapability::FastCodeGeneration,
            RoutingIntent::SanityVerification => ModelCapability::SanityVerification,
            RoutingIntent::GeneralExecution => ModelCapability::GeneralExecution,
        };

        // Filter healthy models supporting the required capability
        let mut candidates: Vec<&ModelManifest> = self
            .models
            .values()
            .filter(|m| m.is_healthy && m.capabilities.contains(&target_cap))
            .collect();

        if candidates.is_empty() {
            // Graceful fallback: pick any healthy model with GeneralExecution
            candidates = self
                .models
                .values()
                .filter(|m| m.is_healthy && m.capabilities.contains(&ModelCapability::GeneralExecution))
                .collect();
        }

        if candidates.is_empty() {
            // Absolute fallback: pick any healthy model
            candidates = self.models.values().filter(|m| m.is_healthy).collect();
        }

        // Rank candidates
        candidates.sort_by(|a, b| {
            // For ComplexReasoning: favor Large/Frontier
            if *intent == RoutingIntent::ComplexReasoning {
                b.parameter_tier_rank().cmp(&a.parameter_tier_rank())
            } else if *intent == RoutingIntent::SanityVerification {
                // For SanityVerification: favor Small (sub-second latency)
                a.parameter_tier_rank().cmp(&b.parameter_tier_rank())
            } else {
                // For code/general: balanced
                b.parameter_tier_rank().cmp(&a.parameter_tier_rank())
            }
        });

        candidates.first().cloned().cloned()
    }

    /// Returns a list of all active healthy models in the cluster.
    pub fn all_active_models(&self) -> Vec<ModelManifest> {
        self.models.values().filter(|m| m.is_healthy).cloned().collect()
    }

    /// Prunes models belonging to nodes that have not sent a heartbeat within `ttl_secs`.
    pub fn prune_stale_nodes(&mut self, ttl_secs: i64) {
        let now = Utc::now().timestamp();
        for m in self.models.values_mut() {
            if now - m.last_seen_epoch > ttl_secs {
                m.is_healthy = false;
            }
        }
    }
}

impl ModelManifest {
    fn parameter_tier_rank(&self) -> u8 {
        match self.parameter_tier {
            ParameterTier::Small => 1,
            ParameterTier::Medium => 2,
            ParameterTier::Large => 3,
            ParameterTier::Frontier => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_derive_qwen_27b() {
        let (caps, tier) = auto_derive_capabilities(
            "qwen3.8:27b",
            Some("27.3B"),
            Some("qwen35"),
            &["thinking".to_string(), "tools".to_string()],
        );
        assert_eq!(tier, ParameterTier::Large);
        assert!(caps.contains(&ModelCapability::ComplexReasoning));
        assert!(caps.contains(&ModelCapability::ToolCalling));
    }

    #[test]
    fn test_auto_derive_deepseek_r1_small() {
        let (caps, tier) = auto_derive_capabilities(
            "deepseek-r1:1.5b",
            Some("1.5B"),
            Some("deepseek"),
            &["thinking".to_string()],
        );
        assert_eq!(tier, ParameterTier::Small);
        assert!(caps.contains(&ModelCapability::SanityVerification));
    }

    #[test]
    fn test_dynamic_registry_routing() {
        let mut reg = ClusterModelRegistry::new();
        reg.register_single_model("PC-1", "http://127.0.0.1:11434", "qwen3.8:27b", None);
        reg.register_single_model("PC-2", "http://192.168.1.102:8642", "deepseek-r1:1.5b", None);

        // Complex reasoning should pick Qwen 3.8 on PC-1
        let best_reasoning = reg.find_best_candidate(&RoutingIntent::ComplexReasoning).unwrap();
        assert_eq!(best_reasoning.node_id, "PC-1");
        assert_eq!(best_reasoning.model_id, "qwen3.8:27b");

        // Sanity verification should pick DeepSeek R1 on PC-2
        let best_sanity = reg.find_best_candidate(&RoutingIntent::SanityVerification).unwrap();
        assert_eq!(best_sanity.node_id, "PC-2");
        assert_eq!(best_sanity.model_id, "deepseek-r1:1.5b");
    }

    #[test]
    fn test_dynamic_failover_when_node_offline() {
        let mut reg = ClusterModelRegistry::new();
        reg.register_single_model("PC-1", "http://127.0.0.1:11434", "gemma4:latest", None);
        // PC-2 is registered with DeepSeek but marked unhealthy
        let pc2_model = ModelManifest {
            model_id: "deepseek-r1:1.5b".to_string(),
            node_id: "PC-2".to_string(),
            endpoint: "http://192.168.1.102:8642".to_string(),
            capabilities: vec![ModelCapability::SanityVerification],
            parameter_tier: ParameterTier::Small,
            context_window: 32768,
            is_healthy: false, // Offline!
            last_seen_epoch: Utc::now().timestamp(),
            latency_ms: None,
        };
        reg.register_models(vec![pc2_model]);

        // When PC-2 is offline, sanity verification must gracefully fall back to PC-1
        let candidate = reg.find_best_candidate(&RoutingIntent::SanityVerification).unwrap();
        assert_eq!(candidate.node_id, "PC-1");
        assert_eq!(candidate.model_id, "gemma4:latest");
    }
}
