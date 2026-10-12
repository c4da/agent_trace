//! In-memory and cache store for active route override rules.

use super::{
    pattern::matches_pattern_str,
    types::{default_rule_id, AgentDelegationRequest, OverrideCriteria, RouteOverrideRule},
};
use std::collections::HashMap;

/// In-memory store for active route overrides set by AI agents.
#[derive(Debug, Clone, Default)]
pub struct RouteOverrideStore {
    rules: HashMap<String, RouteOverrideRule>,
}

impl RouteOverrideStore {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    /// Sets or replaces an override rule. Returns the assigned rule ID.
    pub fn set_rule(&mut self, mut rule: RouteOverrideRule) -> String {
        if rule.rule_id.is_empty() {
            rule.rule_id = default_rule_id();
        }
        let id = rule.rule_id.clone();
        self.rules.insert(id.clone(), rule);
        id
    }

    /// Removes an override rule by ID.
    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        self.rules.remove(rule_id).is_some()
    }

    /// Lists all currently active override rules.
    pub fn list_rules(&self) -> Vec<RouteOverrideRule> {
        self.rules.values().cloned().collect()
    }

    /// Evaluates whether an override rule matches the incoming request context.
    pub fn find_matching_override(&self, req: &AgentDelegationRequest) -> Option<RouteOverrideRule> {
        for rule in self.rules.values() {
            if self.rule_matches(rule, req) {
                return Some(rule.clone());
            }
        }
        None
    }

    /// Checks if a single rule matches the delegation request.
    fn rule_matches(&self, rule: &RouteOverrideRule, req: &AgentDelegationRequest) -> bool {
        match &rule.criteria {
            OverrideCriteria::Session(s) => req
                .session_id
                .as_deref()
                .map(|session| matches_pattern_str(s, session))
                .unwrap_or(false),
            OverrideCriteria::Agent(a) => req
                .agent_id
                .as_deref()
                .map(|agent| matches_pattern_str(a, agent))
                .unwrap_or(false),
            OverrideCriteria::Tag(t) => req
                .tags
                .iter()
                .any(|req_tag| matches_pattern_str(t, req_tag)),
            OverrideCriteria::Model(m) => req
                .requested_model
                .as_deref()
                .map(|model| matches_pattern_str(m, model))
                .unwrap_or(false),
            OverrideCriteria::PromptPattern(p) => matches_pattern_str(p, &req.prompt),
        }
    }
}
