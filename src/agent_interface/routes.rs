//! Direct REST API endpoints for agent delegation and override configuration.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::{json, Value};

use super::types::AgentInterfaceState;
use crate::delegation_engine::{
    AgentDelegationDecision, AgentDelegationRequest, DelegationConfig, RouteOverrideRule,
};

/// Evaluates a routing decision for a given prompt and context.
pub async fn handle_delegate_decide(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    Json(req): Json<AgentDelegationRequest>,
) -> Result<Json<AgentDelegationDecision>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let decision = state.delegation_engine.decide(&req).await;
    Ok(Json(decision))
}

/// Creates or updates a dynamic route override rule.
pub async fn handle_delegate_override(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    Json(rule): Json<RouteOverrideRule>,
) -> Result<Json<RouteOverrideRule>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut store = state.override_store.write().await;
    let _ = store.set_rule(rule.clone());
    Ok(Json(rule))
}

/// Lists all active route overrides.
pub async fn handle_delegate_list_overrides(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let store = state.override_store.read().await;
    let rules = store.list_rules();
    Ok(Json(json!({ "count": rules.len(), "rules": rules })))
}

/// Deletes a route override rule by its unique ID.
pub async fn handle_delegate_remove_override(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    Path(rule_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut store = state.override_store.write().await;
    let removed = store.remove_rule(&rule_id);
    Ok(Json(json!({ "rule_id": rule_id, "removed": removed })))
}

/// Returns current delegation engine configuration.
pub async fn handle_delegate_get_config(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
) -> Result<Json<DelegationConfig>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let cfg = state.delegation_engine.get_config().await;
    Ok(Json(cfg))
}

/// Updates delegation engine configuration.
pub async fn handle_delegate_set_config(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    Json(cfg): Json<DelegationConfig>,
) -> Result<Json<DelegationConfig>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    state.delegation_engine.set_config(cfg.clone()).await;
    Ok(Json(cfg))
}
