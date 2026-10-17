//! Compiles workgroup member hierarchies and attached Markdown governance rules
//! into structured system prompts for LLM injection.

use super::types::ProblemWorkgroup;

impl ProblemWorkgroup {
    /// Compiles all attached `.md` rule files and member role assignments into a
    /// structured system prompt ready for injection into LLM calls.
    pub fn compile_system_prompt(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "You are operating inside AgentTrace Problem Workgroup: {} ({}) [Domain: {}].\n",
            self.name, self.id, self.problem_domain
        ));
        out.push_str(&format!("Problem Focus: {}\n", self.description));

        if !self.members.is_empty() {
            let roster: Vec<String> = self
                .members
                .iter()
                .map(|m| {
                    let parent_desc = match m.parent_agent_id.as_deref() {
                        Some(p) if !p.is_empty() && p != "hub" => format!("chained after {}", p),
                        _ => "under Top Hub (parallel root)".to_string(),
                    };
                    format!("{} ({} on {}, {})", m.display_name, m.role.label(), m.node_id, parent_desc)
                })
                .collect();
            out.push_str(&format!("Assigned Workgroup Tree Roster: {}\n", roster.join(" -> ")));
        }

        if !self.markdown_rules.is_empty() {
            out.push_str("\n=== ATTACHED MARKDOWN (.MD) GOVERNANCE & CODING RULES ===\n");
            for (idx, rule) in self.markdown_rules.iter().enumerate() {
                let enforcement_tag = if rule.enforce_strict {
                    "[STRICT MANDATORY RULE]"
                } else {
                    "[GUIDELINE]"
                };
                out.push_str(&format!(
                    "\n--- Rule #{}: {} {} ---\n{}\n",
                    idx + 1,
                    rule.title,
                    enforcement_tag,
                    rule.resolve_content(2400)
                ));
            }
            out.push_str("\n=== END OF ATTACHED WORKGROUP RULES ===\n");
        }

        out
    }

    /// Resolves the actual execution node, model identifier, and upstream URL for a workgroup member.
    pub fn resolve_executable_target(member: &super::types::WorkgroupMember, default_upstream: &str) -> (String, String, String) {
        let id_lower = member.agent_id.to_lowercase();
        if id_lower.contains("deepseek") {
            (
                "PC-2".to_string(),
                "deepseek-r1:1.5b".to_string(),
                "http://127.0.0.1:8642".to_string(),
            )
        } else if id_lower.contains("27b") {
            (
                "PC-1".to_string(),
                "qwen3.8:27b".to_string(),
                default_upstream.to_string(),
            )
        } else if id_lower.contains("qwen") {
            (
                "PC-1".to_string(),
                "qwen3.5:9b".to_string(),
                default_upstream.to_string(),
            )
        } else if id_lower.contains("gemma") {
            (
                "PC-1".to_string(),
                "gemma4:latest".to_string(),
                default_upstream.to_string(),
            )
        } else if id_lower.contains("copilot") {
            // Copilot CLI synthesis backed by fast local code model when invoked via HTTP Hub
            (
                "PC-1-Copilot".to_string(),
                "qwen3.5:9b".to_string(),
                default_upstream.to_string(),
            )
        } else if id_lower.contains("gemini") || id_lower.contains("antigravity") {
            // Architectural synthesis backed by lead node reasoning engine
            (
                "PC-1".to_string(),
                "qwen3.5:9b".to_string(),
                default_upstream.to_string(),
            )
        } else {
            (
                member.node_id.clone(),
                "qwen3.5:9b".to_string(),
                default_upstream.to_string(),
            )
        }
    }
}
