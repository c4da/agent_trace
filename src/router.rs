//! Smart Intent-Based Prompt Router for AgentTrace Gateway.
//!
//! Provides zero-overhead, sub-millisecond lexical heuristic routing for distributed
//! AI agent clusters. Completely opt-in: explicit model requests bypass heuristic analysis.

use serde::{Deserialize, Serialize};

/// High-level classified purpose/intent of the prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingIntent {
    /// Complex reasoning, architecture, physics, math, multi-step planning.
    ComplexReasoning,
    /// Fast code generation, boilerplate, unit tests, parsers, docstrings.
    FastCodeGeneration,
    /// Sanity verification, regex validation, assertions, linting, syntax audits.
    SanityVerification,
    /// General instruction execution / fallback.
    GeneralExecution,
}

/// Outcome of the routing classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// Whether smart intent routing was activated (true for "auto"/"smart", false for direct model names).
    pub enabled: bool,
    /// Classified intent of the prompt.
    pub intent: RoutingIntent,
    /// Original model identifier requested by the client.
    pub requested_model: String,
    /// Final model chosen for upstream execution.
    pub selected_model: String,
    /// Target upstream endpoint base URL.
    pub target_upstream: String,
    /// Assigned node identifier (e.g., "PC-1" or "PC-2").
    pub node_id: String,
    /// Human-readable explanation of why this route was selected.
    pub rationale: String,
    /// Flag indicating if dynamic fallback occurred because preferred model was unavailable.
    pub fallback_applied: bool,
}

/// Lexically classifies prompt intent based on semantic indicators and length.
pub fn classify_intent(prompt: &str) -> RoutingIntent {
    let prompt_lower = prompt.to_lowercase();
    let prompt_len = prompt.len();

    if prompt_lower.contains("architect")
        || prompt_lower.contains("refactor")
        || prompt_lower.contains("proof")
        || prompt_lower.contains("physics")
        || prompt_lower.contains("math")
        || prompt_lower.contains("algorithm")
        || prompt_lower.contains("invariant")
        || prompt_lower.contains("plan")
        || prompt_len > 1000
    {
        RoutingIntent::ComplexReasoning
    } else if prompt_lower.contains("unit test")
        || prompt_lower.contains("cargo test")
        || prompt_lower.contains("boilerplate")
        || prompt_lower.contains("docstring")
        || prompt_lower.contains("parser")
        || prompt_lower.contains("generate")
        || prompt_lower.contains("scaffold")
    {
        RoutingIntent::FastCodeGeneration
    } else if prompt_lower.contains("verify")
        || prompt_lower.contains("regex")
        || prompt_lower.contains("assertion")
        || prompt_lower.contains("sanity")
        || prompt_lower.contains("validate")
        || prompt_lower.contains("lint")
        || (prompt_len > 0 && prompt_len < 60)
    {
        RoutingIntent::SanityVerification
    } else {
        RoutingIntent::GeneralExecution
    }
}

/// Classifies prompt intent and determines optimal cluster node routing.
///
/// # Arguments
/// * `prompt` - User prompt text.
/// * `requested_model` - Model requested in request payload (e.g. "auto", "qwen3.8:27b", etc.).
/// * `smart_header` - Whether `X-Smart-Route: true` header was sent.
/// * `available_models` - List of live models currently detected in the cluster.
/// * `pc1_upstream` - Upstream base URL for PC-1 (e.g. "http://127.0.0.1:11434").
/// * `pc2_upstream` - Upstream base URL for PC-2 (e.g. "http://192.168.1.102:8642").
#[allow(dead_code)]
pub fn classify_and_route(
    prompt: &str,
    requested_model: &str,
    smart_header: bool,
    available_models: &[String],
    pc1_upstream: &str,
    pc2_upstream: &str,
) -> RoutingDecision {
    let req_lower = requested_model.to_lowercase();
    let is_auto = req_lower == "auto"
        || req_lower == "cluster-router"
        || req_lower == "cluster-smart"
        || req_lower == "smart"
        || smart_header;

    // --- 1. Explicit Direct Route Bypass ---
    if !is_auto && !requested_model.is_empty() {
        let (target_upstream, node_id) = if req_lower.contains("deepseek")
            || req_lower.contains("r1")
            || req_lower.contains("hermes")
        {
            (pc2_upstream.to_string(), "PC-2".to_string())
        } else {
            (pc1_upstream.to_string(), "PC-1".to_string())
        };

        return RoutingDecision {
            enabled: false,
            intent: RoutingIntent::GeneralExecution,
            requested_model: requested_model.to_string(),
            selected_model: requested_model.to_string(),
            target_upstream,
            node_id,
            rationale: format!("Direct explicit routing for model: {}", requested_model),
            fallback_applied: false,
        };
    }

    // --- 2. Lexical Intent Classification ---
    let intent = classify_intent(prompt);
    let (preferred_model, target_upstream_hint, preferred_node) = match intent {
        RoutingIntent::ComplexReasoning => ("qwen3.8:27b".to_string(), pc1_upstream, "PC-1"),
        RoutingIntent::FastCodeGeneration => ("qwen3:14b".to_string(), pc1_upstream, "PC-1"),
        RoutingIntent::SanityVerification => ("deepseek-r1:1.5b".to_string(), pc2_upstream, "PC-2"),
        RoutingIntent::GeneralExecution => ("gemma4:latest".to_string(), pc1_upstream, "PC-1"),
    };

    // --- 3. Dynamic Availability & Fallback Check ---
    let model_is_available = available_models.is_empty()
        || available_models.iter().any(|m| m.contains(&preferred_model) || preferred_model.contains(m));

    let (selected_model, target_upstream, node_id, fallback_applied, rationale) = if model_is_available {
        (
            preferred_model.clone(),
            target_upstream_hint.to_string(),
            preferred_node.to_string(),
            false,
            format!(
                "Classified as {:?}: routed to preferred model '{}' on {}",
                intent, preferred_model, preferred_node
            ),
        )
    } else {
        // Preferred model is not currently online in cluster catalog -> apply dynamic fallback
        let (fb_model, fb_upstream, fb_node) = if preferred_node == "PC-2" {
            // PC-2 preferred missing: try hermes-agent on PC-2 first, else fallback to PC-1
            if available_models.iter().any(|m| m.contains("hermes")) {
                ("hermes-agent".to_string(), pc2_upstream.to_string(), "PC-2".to_string())
            } else if available_models.iter().any(|m| m.contains("qwen3.8")) {
                ("qwen3.8:27b".to_string(), pc1_upstream.to_string(), "PC-1".to_string())
            } else {
                ("gemma4:latest".to_string(), pc1_upstream.to_string(), "PC-1".to_string())
            }
        } else {
            // PC-1 preferred missing (e.g. qwen3:14b still downloading): fallback to qwen3.8:27b or gemma4:latest
            if available_models.iter().any(|m| m.contains("qwen3.8")) {
                ("qwen3.8:27b".to_string(), pc1_upstream.to_string(), "PC-1".to_string())
            } else if available_models.iter().any(|m| m.contains("gemma4")) {
                ("gemma4:latest".to_string(), pc1_upstream.to_string(), "PC-1".to_string())
            } else {
                ("qwen3.8:27b".to_string(), pc1_upstream.to_string(), "PC-1".to_string())
            }
        };

        (
            fb_model.clone(),
            fb_upstream,
            fb_node.clone(),
            true,
            format!(
                "Classified as {:?}: preferred model '{}' unavailable, dynamically fell back to '{}' on {}",
                intent, preferred_model, fb_model, fb_node
            ),
        )
    };

    RoutingDecision {
        enabled: true,
        intent,
        requested_model: requested_model.to_string(),
        selected_model,
        target_upstream,
        node_id,
        rationale,
        fallback_applied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PC1_URL: &str = "http://127.0.0.1:11434";
    const PC2_URL: &str = "http://192.168.1.102:8642";

    #[test]
    fn test_explicit_routing_bypass() {
        let available = vec!["qwen3.8:27b".to_string(), "hermes-agent".to_string()];
        
        let decision_pc1 = classify_and_route("refactor system architecture", "qwen3.8:27b", false, &available, PC1_URL, PC2_URL);
        assert!(!decision_pc1.enabled);
        assert_eq!(decision_pc1.selected_model, "qwen3.8:27b");
        assert_eq!(decision_pc1.node_id, "PC-1");

        let decision_pc2 = classify_and_route("run heavy tests", "hermes-agent", false, &available, PC1_URL, PC2_URL);
        assert!(!decision_pc2.enabled);
        assert_eq!(decision_pc2.selected_model, "hermes-agent");
        assert_eq!(decision_pc2.node_id, "PC-2");
    }

    #[test]
    fn test_complex_reasoning_intent() {
        let available = vec!["qwen3.8:27b".to_string(), "gemma4:latest".to_string()];
        let prompt = "Please design the formal math invariants and architectural proof for the distributed ledger";
        
        let decision = classify_and_route(prompt, "auto", false, &available, PC1_URL, PC2_URL);
        assert!(decision.enabled);
        assert_eq!(decision.intent, RoutingIntent::ComplexReasoning);
        assert_eq!(decision.selected_model, "qwen3.8:27b");
        assert_eq!(decision.node_id, "PC-1");
        assert!(!decision.fallback_applied);
    }

    #[test]
    fn test_fast_code_generation_intent_with_fallback() {
        // qwen3:14b is NOT in available models (still downloading)
        let available = vec!["qwen3.8:27b".to_string(), "gemma4:latest".to_string()];
        let prompt = "Generate unit tests and boilerplate parser for MAVLink telemetry";
        
        let decision = classify_and_route(prompt, "auto", false, &available, PC1_URL, PC2_URL);
        assert!(decision.enabled);
        assert_eq!(decision.intent, RoutingIntent::FastCodeGeneration);
        // Preferred qwen3:14b missing, falls back to qwen3.8:27b
        assert_eq!(decision.selected_model, "qwen3.8:27b");
        assert_eq!(decision.node_id, "PC-1");
        assert!(decision.fallback_applied);
    }

    #[test]
    fn test_sanity_verification_intent_with_fallback() {
        // deepseek-r1:1.5b is NOT in available models, but hermes-agent is online on PC-2
        let available = vec!["hermes-agent".to_string(), "gemma4:latest".to_string()];
        let prompt = "Quick sanity check: verify regex against RFC-3339 timestamp format";
        
        let decision = classify_and_route(prompt, "auto", false, &available, PC1_URL, PC2_URL);
        assert!(decision.enabled);
        assert_eq!(decision.intent, RoutingIntent::SanityVerification);
        // Falls back to hermes-agent on PC-2
        assert_eq!(decision.selected_model, "hermes-agent");
        assert_eq!(decision.node_id, "PC-2");
        assert!(decision.fallback_applied);
    }

    #[test]
    fn test_smart_header_activation() {
        let available = vec!["qwen3.8:27b".to_string(), "gemma4:latest".to_string()];
        // requested_model is empty/default, but header is true
        let decision = classify_and_route("Refactor memory model algorithm", "", true, &available, PC1_URL, PC2_URL);
        assert!(decision.enabled);
        assert_eq!(decision.intent, RoutingIntent::ComplexReasoning);
        assert_eq!(decision.selected_model, "qwen3.8:27b");
    }

    #[test]
    fn test_dynamic_registry_routing() {
        let mut reg = crate::registry::ClusterModelRegistry::new();
        reg.register_single_model("WORKER-GPU", "http://10.0.0.5:8642", "custom-r1:1.5b", Some(vec![crate::registry::ModelCapability::SanityVerification]));
        reg.register_single_model("LEAD-BOX", "http://127.0.0.1:11434", "custom-coder:32b", Some(vec![crate::registry::ModelCapability::ComplexReasoning]));

        let decision = classify_and_route_dynamic("Sanity test regex string", "auto", false, &reg, "http://127.0.0.1:11434");
        assert!(decision.enabled);
        assert_eq!(decision.selected_model, "custom-r1:1.5b");
        assert_eq!(decision.node_id, "WORKER-GPU");
        assert_eq!(decision.target_upstream, "http://10.0.0.5:8642");
    }
}

/// Dynamically routes requests using live capability discovery in ClusterModelRegistry.
/// Eliminates hardcoded model names and static node mappings.
pub fn classify_and_route_dynamic(
    prompt: &str,
    requested_model: &str,
    smart_header: bool,
    registry: &crate::registry::ClusterModelRegistry,
    default_upstream: &str,
) -> RoutingDecision {
    let req_lower = requested_model.to_lowercase();
    let is_auto = req_lower == "auto"
        || req_lower == "cluster-router"
        || req_lower == "cluster-smart"
        || req_lower == "smart"
        || smart_header;

    // --- 1. Explicit Model Request ---
    if !is_auto && !requested_model.is_empty() {
        if let Some(m) = registry.find_model(requested_model) {
            return RoutingDecision {
                enabled: false,
                intent: RoutingIntent::GeneralExecution,
                requested_model: requested_model.to_string(),
                selected_model: m.model_id,
                target_upstream: m.endpoint,
                node_id: m.node_id.clone(),
                rationale: format!("Direct dynamic route to node {}", m.node_id),
                fallback_applied: false,
            };
        } else {
            return RoutingDecision {
                enabled: false,
                intent: RoutingIntent::GeneralExecution,
                requested_model: requested_model.to_string(),
                selected_model: requested_model.to_string(),
                target_upstream: default_upstream.trim_end_matches('/').to_string(),
                node_id: "DEFAULT".to_string(),
                rationale: format!("Direct request for '{}'; dispatched to default upstream", requested_model),
                fallback_applied: false,
            };
        }
    }

    // --- 2. Smart Capability Intent Classification ---
    let intent = classify_intent(prompt);

    if let Some(candidate) = registry.find_best_candidate(&intent) {
        RoutingDecision {
            enabled: true,
            intent: intent.clone(),
            requested_model: requested_model.to_string(),
            selected_model: candidate.model_id.clone(),
            target_upstream: candidate.endpoint,
            node_id: candidate.node_id.clone(),
            rationale: format!(
                "Classified as {:?}: dynamically matched capability on node {} (model '{}')",
                intent, candidate.node_id, candidate.model_id
            ),
            fallback_applied: false,
        }
    } else {
        RoutingDecision {
            enabled: true,
            intent,
            requested_model: requested_model.to_string(),
            selected_model: requested_model.to_string(),
            target_upstream: default_upstream.trim_end_matches('/').to_string(),
            node_id: "LOCAL".to_string(),
            rationale: "No active candidate matching capability; falling back to default upstream".to_string(),
            fallback_applied: true,
        }
    }
}
