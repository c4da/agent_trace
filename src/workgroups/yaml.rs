//! Declarative YAML configuration serialization and parsing for Problem Workgroups.

use super::presets::available_agents_catalog;
use super::types::{MarkdownRuleAttachment, ProblemWorkgroup, WorkgroupMember, WorkgroupRole};

impl ProblemWorkgroup {
    /// Serializes this Problem Workgroup into a concise, human-editable YAML configuration string.
    pub fn to_yaml(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("id: {}\n", self.id));
        out.push_str(&format!("name: {}\n", self.name));
        out.push_str(&format!("problem_domain: {}\n", self.problem_domain));
        out.push_str(&format!("execution_strategy: {}\n", self.execution_strategy));
        out.push_str(&format!(
            "trigger_keywords: [{}]\n",
            self.trigger_keywords.join(", ")
        ));

        out.push_str("rules:\n");
        if self.markdown_rules.is_empty() {
            out.push_str("  # - path: docs/HOW_TO_SAFE_ZONES_AND_PLAYGROUNDS.md\n");
        } else {
            for r in &self.markdown_rules {
                if let Some(ref p) = r.file_path {
                    out.push_str(&format!("  - path: {}\n", p));
                    out.push_str(&format!("    title: {}\n", r.title));
                } else {
                    out.push_str(&format!("  - title: {}\n", r.title));
                    if let Some(ref inline) = r.inline_markdown {
                        let single_line = inline.lines().next().unwrap_or("").trim();
                        if !single_line.is_empty() {
                            out.push_str(&format!("    inline: {}\n", single_line));
                        }
                    }
                }
                out.push_str(&format!("    strict: {}\n", r.enforce_strict));
            }
        }

        out.push_str("tree:\n");
        for m in &self.members {
            let role_str = match m.role {
                WorkgroupRole::LeadArchitect => "LeadArchitect",
                WorkgroupRole::PrimaryBuilder => "PrimaryBuilder",
                WorkgroupRole::SanityVerifier => "SanityVerifier",
                WorkgroupRole::ToolExecutor => "ToolExecutor",
            };
            let parent_str = m.parent_agent_id.as_deref().unwrap_or("hub");
            out.push_str(&format!("  - agent: {}\n", m.agent_id));
            out.push_str(&format!("    role: {}\n", role_str));
            out.push_str(&format!("    parent: {}\n", parent_str));
        }

        out
    }

    /// Parses a concise YAML configuration into a `ProblemWorkgroup`.
    pub fn from_yaml(yaml_str: &str) -> Result<Self, String> {
        let mut id = String::new();
        let mut name = String::new();
        let mut problem_domain = "custom".to_string();
        let mut execution_strategy = "pipeline".to_string();
        let mut trigger_keywords: Vec<String> = Vec::new();
        let mut rules: Vec<MarkdownRuleAttachment> = Vec::new();
        let mut members: Vec<WorkgroupMember> = Vec::new();

        #[derive(PartialEq)]
        enum Section {
            Top,
            Rules,
            Tree,
        }
        let mut section = Section::Top;
        let catalog = available_agents_catalog();

        for raw_line in yaml_str.lines() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if trimmed == "rules:" {
                section = Section::Rules;
                continue;
            } else if trimmed == "tree:" || trimmed == "members:" {
                section = Section::Tree;
                continue;
            }

            match section {
                Section::Top => {
                    if let Some((k, v)) = trimmed.split_once(':') {
                        let key = k.trim();
                        let val = v.trim().trim_matches('"').trim_matches('\'');
                        match key {
                            "id" => id = val.to_string(),
                            "name" => name = val.to_string(),
                            "problem_domain" => problem_domain = val.to_string(),
                            "execution_strategy" => execution_strategy = val.to_string(),
                            "trigger_keywords" => {
                                let inner = val.trim_start_matches('[').trim_end_matches(']');
                                trigger_keywords = inner
                                    .split(',')
                                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                            }
                            _ => {}
                        }
                    }
                }
                Section::Rules => {
                    let is_new_item = trimmed.starts_with("- ");
                    let line_body = trimmed.trim_start_matches("- ").trim();
                    if is_new_item {
                        rules.push(MarkdownRuleAttachment {
                            id: format!("rule-{}", rules.len() + 1),
                            title: "Attached Rule".to_string(),
                            file_path: None,
                            inline_markdown: None,
                            enforce_strict: true,
                        });
                    }
                    if let Some((k, v)) = line_body.split_once(':') {
                        if let Some(last_rule) = rules.last_mut() {
                            let key = k.trim();
                            let val = v.trim().trim_matches('"').trim_matches('\'');
                            match key {
                                "path" | "file_path" => {
                                    last_rule.file_path = Some(val.to_string());
                                    if last_rule.title == "Attached Rule" {
                                        last_rule.title = val
                                            .rsplit('/')
                                            .next()
                                            .unwrap_or(val)
                                            .to_string();
                                    }
                                    let slug: String = val
                                        .to_lowercase()
                                        .chars()
                                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                                        .collect();
                                    last_rule.id = format!("rule-{}", slug.trim_matches('-'));
                                }
                                "title" => last_rule.title = val.to_string(),
                                "inline" | "inline_markdown" => {
                                    last_rule.inline_markdown = Some(val.to_string())
                                }
                                "strict" | "enforce_strict" => {
                                    last_rule.enforce_strict = val != "false";
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Section::Tree => {
                    let is_new_item = trimmed.starts_with("- ");
                    let line_body = trimmed.trim_start_matches("- ").trim();
                    if is_new_item {
                        members.push(WorkgroupMember {
                            agent_id: String::new(),
                            display_name: String::new(),
                            node_id: "PC-1".to_string(),
                            role: WorkgroupRole::PrimaryBuilder,
                            parent_agent_id: None,
                        });
                    }
                    if let Some((k, v)) = line_body.split_once(':') {
                        if let Some(last_member) = members.last_mut() {
                            let key = k.trim();
                            let val = v.trim().trim_matches('"').trim_matches('\'');
                            match key {
                                "agent" | "agent_id" => {
                                    last_member.agent_id = val.to_string();
                                    if let Some(cat) = catalog.iter().find(|c| c.agent_id == val) {
                                        last_member.display_name = cat.display_name.clone();
                                        last_member.node_id = cat.node_id.clone();
                                        last_member.role = cat.default_role.clone();
                                    } else {
                                        last_member.display_name = val.to_string();
                                    }
                                }
                                "role" => {
                                    last_member.role = match val {
                                        "LeadArchitect" => WorkgroupRole::LeadArchitect,
                                        "SanityVerifier" => WorkgroupRole::SanityVerifier,
                                        "ToolExecutor" => WorkgroupRole::ToolExecutor,
                                        _ => WorkgroupRole::PrimaryBuilder,
                                    };
                                }
                                "parent" | "parent_agent_id" => {
                                    if val.is_empty()
                                        || val.eq_ignore_ascii_case("hub")
                                        || val.eq_ignore_ascii_case("null")
                                        || val.eq_ignore_ascii_case("none")
                                        || val.eq_ignore_ascii_case("root")
                                    {
                                        last_member.parent_agent_id = None;
                                    } else {
                                        last_member.parent_agent_id = Some(val.to_string());
                                    }
                                }
                                "node" | "node_id" => {
                                    last_member.node_id = val.to_string();
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        members.retain(|m| !m.agent_id.is_empty());
        if name.is_empty() && !id.is_empty() {
            name = id.clone();
        }
        if name.is_empty() {
            return Err("YAML config must specify at least `name:` or `id:`".to_string());
        }

        Ok(ProblemWorkgroup {
            id,
            name,
            problem_domain,
            icon: "🌳".to_string(),
            color: "#3b82f6".to_string(),
            description: "Configured via declarative YAML".to_string(),
            execution_strategy,
            members,
            markdown_rules: rules,
            trigger_keywords,
            tasks_completed: 0,
        })
    }
}
