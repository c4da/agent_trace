//! Core data structures and role definitions for Problem Workgroups.

use crate::zones::resolve_canonical_target;
use serde::{Deserialize, Serialize};
use std::fs;

/// Role assigned to an agent inside a Problem Workgroup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkgroupRole {
    LeadArchitect,
    PrimaryBuilder,
    SanityVerifier,
    ToolExecutor,
}

impl WorkgroupRole {
    pub fn label(&self) -> &'static str {
        match self {
            WorkgroupRole::LeadArchitect => "Lead Architect",
            WorkgroupRole::PrimaryBuilder => "Primary Builder",
            WorkgroupRole::SanityVerifier => "Verifier & Critic",
            WorkgroupRole::ToolExecutor => "CLI / Tool Executor",
        }
    }
}

/// An AI agent or model assigned to a Problem Workgroup tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkgroupMember {
    pub agent_id: String,
    pub display_name: String,
    pub node_id: String,
    pub role: WorkgroupRole,
    /// Optional parent `agent_id` in the workgroup tree.
    /// `None` means the agent is connected directly under the Top Workgroup Hub (parallel root tier).
    /// `Some(parent_id)` means the agent is chained sequentially after `parent_id`.
    #[serde(default)]
    pub parent_agent_id: Option<String>,
}

/// A Markdown (`.md`) rule file attached to a Problem Workgroup.
/// Can reference a workspace `.md` file path, carry inline/uploaded Markdown text, or both.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkdownRuleAttachment {
    pub id: String,
    pub title: String,
    pub file_path: Option<String>,
    pub inline_markdown: Option<String>,
    pub enforce_strict: bool,
}

impl MarkdownRuleAttachment {
    /// Safely resolves the Markdown rule content, reading from disk if `file_path` is set
    /// and permitted (blocking any `..` traversal into `.secrets` or `.ssh`), and merging
    /// with `inline_markdown`.
    pub fn resolve_content(&self, max_chars: usize) -> String {
        let mut sections = Vec::new();

        if let Some(ref raw_path) = self.file_path {
            let trimmed = raw_path.trim();
            if !trimmed.is_empty() {
                let norm = resolve_canonical_target(trimmed);
                let lower = norm.to_lowercase();
                // Never allow reading from sensitive vault directories via rule attachment
                let is_forbidden = lower.contains(".secrets")
                    || lower.contains(".ssh")
                    || lower.ends_with(".env")
                    || lower.contains("node_identity.key");

                if is_forbidden {
                    sections.push(format!(
                        "<!-- BLOCKED BY SPATIAL VAULT POLICY: {} -->",
                        norm
                    ));
                } else if let Ok(disk_text) = fs::read_to_string(trimmed) {
                    let snippet = if disk_text.len() > max_chars {
                        format!("{}...\n[Truncated at {} chars]", &disk_text[..max_chars], max_chars)
                    } else {
                        disk_text
                    };
                    sections.push(format!("### Source File: `{}`\n{}", norm, snippet.trim()));
                }
            }
        }

        if let Some(ref inline) = self.inline_markdown {
            let trimmed = inline.trim();
            if !trimmed.is_empty() {
                let snippet = if trimmed.len() > max_chars {
                    format!("{}...", &trimmed[..max_chars])
                } else {
                    trimmed.to_string()
                };
                sections.push(snippet);
            }
        }

        if sections.is_empty() {
            format!("Rule `{}` (no content found)", self.title)
        } else {
            sections.join("\n\n")
        }
    }
}

/// A user-defined or preset Problem Workgroup binding a problem domain to
/// specific agents and attached Markdown rule files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemWorkgroup {
    pub id: String,
    pub name: String,
    pub problem_domain: String,
    pub icon: String,
    pub color: String,
    pub description: String,
    /// `"pipeline"` (Builder -> Verifier), `"swarm"` (parallel multi-agent), or `"auto"` (single best member).
    pub execution_strategy: String,
    pub members: Vec<WorkgroupMember>,
    pub markdown_rules: Vec<MarkdownRuleAttachment>,
    pub trigger_keywords: Vec<String>,
    pub tasks_completed: u64,
}

/// Describes an agent/model that can be assigned to a workgroup in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableAgentInfo {
    pub agent_id: String,
    pub display_name: String,
    pub node_id: String,
    pub badge_color: String,
    pub specialty: String,
    pub default_role: WorkgroupRole,
}

/// Discovered Markdown rule files found in workspace root and `docs/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredMarkdownFile {
    pub path: String,
    pub title: String,
    pub size_bytes: u64,
    pub preview: String,
}
