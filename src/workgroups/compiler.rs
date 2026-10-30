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
        if id_lower.contains("deepseek") || id_lower.contains("hermes") || member.node_id == "PC-2" {
            (
                "PC-2".to_string(),
                "hermes-agent".to_string(),
                "http://192.168.1.102:8642".to_string(),
            )
        } else if id_lower.contains("27b") {
            (
                "PC-1".to_string(),
                "qwen3.8:27b".to_string(),
                default_upstream.to_string(),
            )
        } else if id_lower.contains("gemini")
            || id_lower.contains("antigravity")
            || id_lower.contains("qwen")
            || id_lower.contains("gemma")
        {
            // Fast, zero-OOM local execution on PC-1 (Gemma 4)
            (
                "PC-1".to_string(),
                "gemma4:latest".to_string(),
                default_upstream.to_string(),
            )
        } else if id_lower.contains("copilot") {
            (
                "PC-1".to_string(),
                "copilot-cli".to_string(),
                "cli://copilot".to_string(),
            )
        } else {
            (
                member.node_id.clone(),
                "gemma4:latest".to_string(),
                default_upstream.to_string(),
            )
        }
    }
}
