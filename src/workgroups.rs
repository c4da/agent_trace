//! Problem Workgroups & Markdown Rule Attachment Engine (`src/workgroups.rs`).
//!
//! Allows operators to graphically define problem-specific agent teams (e.g.,
//! "Image Generation" -> `[deepseek-r1:1.5b, copilot-cli]`, or
//! "Coding & Architecture" -> `[qwen3.8:27b, gemini-pro]`) and attach `.md`
//! rule files (by workspace path or inline/uploaded Markdown content) that are
//! automatically compiled and injected into every prompt dispatched to the workgroup.

use crate::zones::resolve_canonical_target;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

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

/// An AI agent or model assigned to a Problem Workgroup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkgroupMember {
    pub agent_id: String,
    pub display_name: String,
    pub node_id: String,
    pub role: WorkgroupRole,
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
                .map(|m| format!("{} ({} on {})", m.display_name, m.role.label(), m.node_id))
                .collect();
            out.push_str(&format!("Assigned Workgroup Roster: {}\n", roster.join(", ")));
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

    /// Maps an abstract agent ID (like `"copilot-cli"` or `"gemini-pro"`) to an executable
    /// cluster model and node so every workgroup member can participate in live inference.
    pub fn resolve_executable_target(member: &WorkgroupMember, default_upstream: &str) -> (String, String, String) {
        let id_lower = member.agent_id.to_lowercase();
        if id_lower.contains("deepseek") {
            (
                "PC-2".to_string(),
                "deepseek-r1:1.5b".to_string(),
                "http://192.168.1.102:8642".to_string(),
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

/// Metadata describing an available agent in the cluster catalog for the graphical builder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableAgentInfo {
    pub agent_id: String,
    pub display_name: String,
    pub node_id: String,
    pub badge_color: String,
    pub specialty: String,
    pub default_role: WorkgroupRole,
}

/// Metadata describing a discovered `.md` file in the repository for 1-click attachment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredMarkdownFile {
    pub path: String,
    pub title: String,
    pub size_bytes: u64,
    pub preview: String,
}

/// Complete state of all Problem Workgroups, persisted to `.agent_trace/workgroups.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkgroupRegistry {
    pub groups: HashMap<String, ProblemWorkgroup>,
    #[serde(skip)]
    persistence_path: Option<PathBuf>,
}

impl Default for WorkgroupRegistry {
    fn default() -> Self {
        Self::new_with_defaults(Some(PathBuf::from(".agent_trace/workgroups.json")))
    }
}

impl WorkgroupRegistry {
    pub fn new_with_defaults(persistence_path: Option<PathBuf>) -> Self {
        let mut groups = HashMap::new();

        // 1. Coding & Systems Architecture Workgroup (Qwen + Gemini + Anti-Monolith MD rules)
        let wg_coding = ProblemWorkgroup {
            id: "wg-coding".to_string(),
            name: "Coding & Architecture".to_string(),
            problem_domain: "coding".to_string(),
            icon: "💻".to_string(),
            color: "#58a6ff".to_string(),
            description: "Multi-agent software implementation, refactoring, and unit test generation with strict Anti-Monolith & SRP Markdown rules.".to_string(),
            execution_strategy: "pipeline".to_string(),
            members: vec![
                WorkgroupMember {
                    agent_id: "qwen3.5:9b".to_string(),
                    display_name: "Qwen 3.5 (9B Code)".to_string(),
                    node_id: "PC-1".to_string(),
                    role: WorkgroupRole::PrimaryBuilder,
                },
                WorkgroupMember {
                    agent_id: "gemini-pro".to_string(),
                    display_name: "Gemini / Antigravity".to_string(),
                    node_id: "PC-1".to_string(),
                    role: WorkgroupRole::LeadArchitect,
                },
                WorkgroupMember {
                    agent_id: "qwen3.8:27b".to_string(),
                    display_name: "Qwen 3.8 (27B Heavy)".to_string(),
                    node_id: "PC-1".to_string(),
                    role: WorkgroupRole::SanityVerifier,
                },
            ],
            markdown_rules: vec![
                MarkdownRuleAttachment {
                    id: "rule-srp-coding".to_string(),
                    title: "Anti-Monolith & Feature-per-File Standard".to_string(),
                    file_path: Some("docs/RUST_ENGINEERING_GUIDE_CHANNELS_AND_SSRF.md".to_string()),
                    inline_markdown: Some(
                        "- Never generate monolithic 1,000+ line files.\n- Keep functions concise (<= 3 arguments where practical).\n- Always include unit tests targeting >= 85% coverage.".to_string(),
                    ),
                    enforce_strict: true,
                },
            ],
            trigger_keywords: vec![
                "code".to_string(),
                "rust".to_string(),
                "python".to_string(),
                "refactor".to_string(),
                "implement".to_string(),
                "function".to_string(),
                "struct".to_string(),
                "unit test".to_string(),
            ],
            tasks_completed: 14,
        };

        // 2. Image & Visual Asset Generation Workgroup (DeepSeek + Copilot CLI)
        let wg_image = ProblemWorkgroup {
            id: "wg-image-gen".to_string(),
            name: "Image & SVG Generation".to_string(),
            problem_domain: "image_generation".to_string(),
            icon: "🎨".to_string(),
            color: "#bc8cff".to_string(),
            description: "Visual asset, SVG diagram, and UI mockup synthesis pairing DeepSeek-R1 prompt composition with GitHub Copilot CLI automation.".to_string(),
            execution_strategy: "pipeline".to_string(),
            members: vec![
                WorkgroupMember {
                    agent_id: "deepseek-r1:1.5b".to_string(),
                    display_name: "DeepSeek-R1 (1.5B CUDA)".to_string(),
                    node_id: "PC-2".to_string(),
                    role: WorkgroupRole::LeadArchitect,
                },
                WorkgroupMember {
                    agent_id: "copilot-cli".to_string(),
                    display_name: "GitHub Copilot CLI".to_string(),
                    node_id: "PC-1-Copilot".to_string(),
                    role: WorkgroupRole::PrimaryBuilder,
                },
            ],
            markdown_rules: vec![
                MarkdownRuleAttachment {
                    id: "rule-visual-assets".to_string(),
                    title: "Work-First Visual & Image Generation Rules".to_string(),
                    file_path: Some("docs/UI_DESIGN_PHILOSOPHY_AND_PROMPT.md".to_string()),
                    inline_markdown: Some(
                        "- Generate only the interface or crisp vector asset itself without surrounding laptop/phone device frames.\n- Use high-contrast dark operator palette (#0d1117 background, #58a6ff cyan, #3fb950 green, #bc8cff purple).\n- Prefer clean SVG geometry and deterministic prompts.".to_string(),
                    ),
                    enforce_strict: true,
                },
            ],
            trigger_keywords: vec![
                "image".to_string(),
                "svg".to_string(),
                "icon".to_string(),
                "diagram".to_string(),
                "visual".to_string(),
                "mockup".to_string(),
                "draw".to_string(),
                "illustration".to_string(),
            ],
            tasks_completed: 6,
        };

        // 3. Security Audit & Spatial Containment Workgroup (DeepSeek + Qwen + Copilot CLI)
        let wg_security = ProblemWorkgroup {
            id: "wg-security-audit".to_string(),
            name: "Security & Zone Audit".to_string(),
            problem_domain: "security".to_string(),
            icon: "🛡️".to_string(),
            color: "#3fb950".to_string(),
            description: "Audits shell commands, path traversals, SSRF origins, and Safe Zone vs. Playground containment rules.".to_string(),
            execution_strategy: "swarm".to_string(),
            members: vec![
                WorkgroupMember {
                    agent_id: "deepseek-r1:1.5b".to_string(),
                    display_name: "DeepSeek-R1 (1.5B CUDA)".to_string(),
                    node_id: "PC-2".to_string(),
                    role: WorkgroupRole::SanityVerifier,
                },
                WorkgroupMember {
                    agent_id: "qwen3.5:9b".to_string(),
                    display_name: "Qwen 3.5 (9B Code)".to_string(),
                    node_id: "PC-1".to_string(),
                    role: WorkgroupRole::PrimaryBuilder,
                },
                WorkgroupMember {
                    agent_id: "copilot-cli".to_string(),
                    display_name: "GitHub Copilot CLI".to_string(),
                    node_id: "PC-1-Copilot".to_string(),
                    role: WorkgroupRole::ToolExecutor,
                },
            ],
            markdown_rules: vec![
                MarkdownRuleAttachment {
                    id: "rule-safe-zones".to_string(),
                    title: "Safe Zones & Vault Containment Policy".to_string(),
                    file_path: Some("docs/HOW_TO_SAFE_ZONES_AND_PLAYGROUNDS.md".to_string()),
                    inline_markdown: Some(
                        "- Block all access to VaultNoAccess (.secrets, .ssh, .env).\n- Collapse lexical .. path traversals before prefix evaluation.\n- Enforce RFC 3986 origin tuple comparison against SSRF.".to_string(),
                    ),
                    enforce_strict: true,
                },
            ],
            trigger_keywords: vec![
                "security".to_string(),
                "audit".to_string(),
                "ssrf".to_string(),
                "traversal".to_string(),
                "vault".to_string(),
                "playground".to_string(),
                "zone".to_string(),
                "secret".to_string(),
            ],
            tasks_completed: 9,
        };

        // 4. Work-First UI & TUI Engineering Workgroup
        let wg_ui = ProblemWorkgroup {
            id: "wg-ui-workfirst".to_string(),
            name: "Work-First UI & TUI".to_string(),
            problem_domain: "ui_design".to_string(),
            icon: "🖥️".to_string(),
            color: "#d29922".to_string(),
            description: "Builds high-density terminal consoles (Ghostty/k9s style), scenario toolbars, and keyboard-first forensic workflows.".to_string(),
            execution_strategy: "pipeline".to_string(),
            members: vec![
                WorkgroupMember {
                    agent_id: "gemini-pro".to_string(),
                    display_name: "Gemini / Antigravity".to_string(),
                    node_id: "PC-1".to_string(),
                    role: WorkgroupRole::LeadArchitect,
                },
                WorkgroupMember {
                    agent_id: "qwen3.5:9b".to_string(),
                    display_name: "Qwen 3.5 (9B Code)".to_string(),
                    node_id: "PC-1".to_string(),
                    role: WorkgroupRole::PrimaryBuilder,
                },
                WorkgroupMember {
                    agent_id: "deepseek-r1:1.5b".to_string(),
                    display_name: "DeepSeek-R1 (1.5B CUDA)".to_string(),
                    node_id: "PC-2".to_string(),
                    role: WorkgroupRole::SanityVerifier,
                },
            ],
            markdown_rules: vec![
                MarkdownRuleAttachment {
                    id: "rule-work-first-ui".to_string(),
                    title: "Work-First UI Blueprint (6 Principles)".to_string(),
                    file_path: Some("docs/UI_DESIGN_PHILOSOPHY_AND_PROMPT.md".to_string()),
                    inline_markdown: Some(
                        "- High density: 30-50 records visible without scrolling.\n- Scenario-first toolbar & 3-part explainer cards.\n- Manual-first operator toolbox (j/k navigation, P pause, M sandbox, F flag).".to_string(),
                    ),
                    enforce_strict: true,
                },
            ],
            trigger_keywords: vec![
                "ui".to_string(),
                "dashboard".to_string(),
                "console".to_string(),
                "tui".to_string(),
                "html".to_string(),
                "css".to_string(),
                "keyboard".to_string(),
            ],
            tasks_completed: 8,
        };

        // 5. Reasoning & Coding Issue Debugging Workgroup (DeepSeek-R1 + Qwen 27B + Copilot CLI)
        let wg_reasoning = ProblemWorkgroup {
            id: "wg-reasoning-debug".to_string(),
            name: "Reasoning & Coding Issues".to_string(),
            problem_domain: "reasoning_debugging".to_string(),
            icon: "🧠".to_string(),
            color: "#f0883e".to_string(),
            description: "Root-cause bug diagnosis, compiler/borrow-checker errors, async deadlock/race analysis, and mathematical invariant verification.".to_string(),
            execution_strategy: "pipeline".to_string(),
            members: vec![
                WorkgroupMember {
                    agent_id: "deepseek-r1:1.5b".to_string(),
                    display_name: "DeepSeek-R1 (1.5B CUDA)".to_string(),
                    node_id: "PC-2".to_string(),
                    role: WorkgroupRole::SanityVerifier,
                },
                WorkgroupMember {
                    agent_id: "qwen3.5:9b".to_string(),
                    display_name: "Qwen 3.5 (9B Code & Debug)".to_string(),
                    node_id: "PC-1".to_string(),
                    role: WorkgroupRole::LeadArchitect,
                },
                WorkgroupMember {
                    agent_id: "copilot-cli".to_string(),
                    display_name: "GitHub Copilot CLI".to_string(),
                    node_id: "PC-1-Copilot".to_string(),
                    role: WorkgroupRole::ToolExecutor,
                },
            ],
            markdown_rules: vec![
                MarkdownRuleAttachment {
                    id: "rule-reasoning-debug".to_string(),
                    title: "Root-Cause Debugging & Reasoning Protocol".to_string(),
                    file_path: Some("docs/RUST_ENGINEERING_GUIDE_CHANNELS_AND_SSRF.md".to_string()),
                    inline_markdown: Some(
                        "- Isolate the minimal reproducible failure (compiler error, panic backtrace, or violated invariant) before proposing edits.\n- Inspect async lock contention (prefer bounded mpsc channels over shared Mutex locks).\n- Pair every bug fix with a deterministic regression test.".to_string(),
                    ),
                    enforce_strict: true,
                },
            ],
            trigger_keywords: vec![
                "bug".to_string(),
                "issue".to_string(),
                "error".to_string(),
                "panic".to_string(),
                "debug".to_string(),
                "reason".to_string(),
                "why".to_string(),
                "deadlock".to_string(),
                "borrow".to_string(),
                "compile".to_string(),
                "fix".to_string(),
            ],
            tasks_completed: 11,
        };

        groups.insert(wg_coding.id.clone(), wg_coding);
        groups.insert(wg_image.id.clone(), wg_image);
        groups.insert(wg_security.id.clone(), wg_security);
        groups.insert(wg_ui.id.clone(), wg_ui);
        groups.insert(wg_reasoning.id.clone(), wg_reasoning);

        if let Some(ref path) = persistence_path {
            if let Ok(raw) = fs::read_to_string(path) {
                if let Ok(loaded) = serde_json::from_str::<WorkgroupRegistry>(&raw) {
                    for (k, v) in loaded.groups {
                        groups.insert(k, v);
                    }
                }
            }
        }

        let reg = Self {
            groups,
            persistence_path,
        };
        reg.save_to_disk();
        reg
    }

    pub fn save_to_disk(&self) {
        if let Some(ref path) = self.persistence_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(path, json);
            }
        }
    }

    pub fn list_sorted(&self) -> Vec<ProblemWorkgroup> {
        let mut list: Vec<ProblemWorkgroup> = self.groups.values().cloned().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    pub fn get(&self, id: &str) -> Option<ProblemWorkgroup> {
        self.groups.get(id).cloned()
    }

    pub fn upsert(&mut self, mut wg: ProblemWorkgroup) -> ProblemWorkgroup {
        if wg.id.trim().is_empty() {
            let slug: String = wg
                .name
                .to_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            wg.id = format!("wg-{}", slug.trim_matches('-'));
        }
        if wg.icon.trim().is_empty() {
            wg.icon = "⚡".to_string();
        }
        if wg.color.trim().is_empty() {
            wg.color = "#58a6ff".to_string();
        }
        if wg.execution_strategy.trim().is_empty() {
            wg.execution_strategy = "pipeline".to_string();
        }
        self.groups.insert(wg.id.clone(), wg.clone());
        self.save_to_disk();
        wg
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let removed = self.groups.remove(id).is_some();
        if removed {
            self.save_to_disk();
        }
        removed
    }

    pub fn attach_rule(&mut self, workgroup_id: &str, rule: MarkdownRuleAttachment) -> Option<ProblemWorkgroup> {
        let wg = self.groups.get_mut(workgroup_id)?;
        if let Some(existing) = wg.markdown_rules.iter_mut().find(|r| r.id == rule.id) {
            *existing = rule;
        } else {
            wg.markdown_rules.push(rule);
        }
        let updated = wg.clone();
        self.save_to_disk();
        Some(updated)
    }

    pub fn increment_tasks(&mut self, workgroup_id: &str) {
        if let Some(wg) = self.groups.get_mut(workgroup_id) {
            wg.tasks_completed += 1;
            self.save_to_disk();
        }
    }

    /// Automatically matches a prompt to a Problem Workgroup by checking `trigger_keywords`.
    pub fn match_workgroup_by_prompt(&self, prompt: &str) -> Option<ProblemWorkgroup> {
        let lower = prompt.to_lowercase();
        let mut best: Option<(usize, &ProblemWorkgroup)> = None;
        for wg in self.groups.values() {
            let hits = wg
                .trigger_keywords
                .iter()
                .filter(|kw| !kw.is_empty() && lower.contains(&kw.to_lowercase()))
                .count();
            if hits > 0 {
                if best.map_or(true, |(best_hits, _)| hits > best_hits) {
                    best = Some((hits, wg));
                }
            }
        }
        best.map(|(_, wg)| wg.clone())
    }
}

/// Returns the catalog of cluster agents that can be graphically assigned to any Problem Workgroup.
pub fn available_agents_catalog() -> Vec<AvailableAgentInfo> {
    vec![
        AvailableAgentInfo {
            agent_id: "qwen3.5:9b".to_string(),
            display_name: "Qwen 3.5 (9B Fast Code)".to_string(),
            node_id: "PC-1".to_string(),
            badge_color: "#58a6ff".to_string(),
            specialty: "Fast Rust/Python coding, unit tests, boilerplate & telemetry".to_string(),
            default_role: WorkgroupRole::PrimaryBuilder,
        },
        AvailableAgentInfo {
            agent_id: "qwen3.8:27b".to_string(),
            display_name: "Qwen 3.8 (27B Heavy Reasoning)".to_string(),
            node_id: "PC-1".to_string(),
            badge_color: "#388bfd".to_string(),
            specialty: "Deep architecture refactoring, math invariants & complex logic".to_string(),
            default_role: WorkgroupRole::LeadArchitect,
        },
        AvailableAgentInfo {
            agent_id: "deepseek-r1:1.5b".to_string(),
            display_name: "DeepSeek-R1 (1.5B CUDA)".to_string(),
            node_id: "PC-2".to_string(),
            badge_color: "#3fb950".to_string(),
            specialty: "Ultra-fast GPU chain-of-thought verification, prompt synthesis & regex audit".to_string(),
            default_role: WorkgroupRole::SanityVerifier,
        },
        AvailableAgentInfo {
            agent_id: "copilot-cli".to_string(),
            display_name: "GitHub Copilot CLI".to_string(),
            node_id: "PC-1-Copilot".to_string(),
            badge_color: "#bc8cff".to_string(),
            specialty: "Terminal command synthesis, SVG/asset scripts & shell automation".to_string(),
            default_role: WorkgroupRole::ToolExecutor,
        },
        AvailableAgentInfo {
            agent_id: "gemini-pro".to_string(),
            display_name: "Gemini / Antigravity Lead".to_string(),
            node_id: "PC-1".to_string(),
            badge_color: "#f0883e".to_string(),
            specialty: "System architecture, multi-file planning & UI/UX orchestration".to_string(),
            default_role: WorkgroupRole::LeadArchitect,
        },
        AvailableAgentInfo {
            agent_id: "hermes-agent".to_string(),
            display_name: "Hermes Cluster Daemon".to_string(),
            node_id: "PC-2".to_string(),
            badge_color: "#d29922".to_string(),
            specialty: "Remote worker execution, background watchdog & LAN telemetry".to_string(),
            default_role: WorkgroupRole::ToolExecutor,
        },
        AvailableAgentInfo {
            agent_id: "gemma4:latest".to_string(),
            display_name: "Gemma 4 (Local General)".to_string(),
            node_id: "PC-1".to_string(),
            badge_color: "#a371f7".to_string(),
            specialty: "Offline documentation, summarization & general Q&A".to_string(),
            default_role: WorkgroupRole::SanityVerifier,
        },
    ]
}

/// Scans the workspace root and `docs/` directory for `.md` files so the user can
/// attach existing Markdown rule files with a single click in the UI.
pub fn discover_workspace_markdown_files() -> Vec<DiscoveredMarkdownFile> {
    let mut results = Vec::new();
    let candidate_dirs = [".", "docs"];

    for dir in candidate_dirs {
        let dir_path = Path::new(dir);
        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if ext.eq_ignore_ascii_case("md") {
                            let rel_path = path
                                .to_string_lossy()
                                .trim_start_matches(".\\")
                                .trim_start_matches("./")
                                .replace('\\', "/");
                            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            let raw_text = fs::read_to_string(&path).unwrap_or_default();
                            let title = raw_text
                                .lines()
                                .find(|l| l.trim_start().starts_with('#'))
                                .map(|l| l.trim_start_matches('#').trim().to_string())
                                .unwrap_or_else(|| rel_path.clone());
                            let preview: String = raw_text.chars().take(240).collect();

                            results.push(DiscoveredMarkdownFile {
                                path: rel_path,
                                title,
                                size_bytes,
                                preview,
                            });
                        }
                    }
                }
            }
        }
    }

    results.sort_by(|a, b| a.path.cmp(&b.path));
    results
}

use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

static GLOBAL_WORKGROUPS: OnceLock<Arc<RwLock<WorkgroupRegistry>>> = OnceLock::new();

/// Returns the shared thread-safe `WorkgroupRegistry` singleton.
pub fn global_workgroups() -> Arc<RwLock<WorkgroupRegistry>> {
    GLOBAL_WORKGROUPS
        .get_or_init(|| Arc::new(RwLock::new(WorkgroupRegistry::default())))
        .clone()
}

/// `GET /api/v1/workgroups` — Lists all Problem Workgroups, available cluster agents,
/// and discovered `.md` files in the workspace.
pub async fn list_workgroups_handler() -> Result<Json<serde_json::Value>, StatusCode> {
    let reg = global_workgroups();
    let guard = reg.read().await;
    let workgroups = guard.list_sorted();
    drop(guard);

    Ok(Json(serde_json::json!({
        "workgroups": workgroups,
        "available_agents": available_agents_catalog(),
        "discovered_markdown_files": discover_workspace_markdown_files(),
    })))
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertWorkgroupPayload {
    pub id: Option<String>,
    pub name: String,
    pub problem_domain: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub execution_strategy: Option<String>,
    pub members: Vec<WorkgroupMember>,
    pub markdown_rules: Option<Vec<MarkdownRuleAttachment>>,
    pub trigger_keywords: Option<Vec<String>>,
}

/// `POST /api/v1/workgroups` — Creates or updates a Problem Workgroup.
pub async fn upsert_workgroup_handler(
    State(state): State<crate::handlers::AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpsertWorkgroupPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let reg = global_workgroups();
    let mut guard = reg.write().await;

    let existing_tasks = payload
        .id
        .as_deref()
        .and_then(|id| guard.get(id))
        .map(|w| w.tasks_completed)
        .unwrap_or(0);

    let existing_rules = payload
        .id
        .as_deref()
        .and_then(|id| guard.get(id))
        .map(|w| w.markdown_rules)
        .unwrap_or_default();

    let wg = ProblemWorkgroup {
        id: payload.id.unwrap_or_default(),
        name: payload.name,
        problem_domain: payload.problem_domain,
        icon: payload.icon.unwrap_or_else(|| "⚡".to_string()),
        color: payload.color.unwrap_or_else(|| "#58a6ff".to_string()),
        description: payload.description.unwrap_or_default(),
        execution_strategy: payload.execution_strategy.unwrap_or_else(|| "pipeline".to_string()),
        members: payload.members,
        markdown_rules: payload.markdown_rules.unwrap_or(existing_rules),
        trigger_keywords: payload.trigger_keywords.unwrap_or_default(),
        tasks_completed: existing_tasks,
    };

    let saved = guard.upsert(wg);
    drop(guard);

    Ok(Json(serde_json::json!({
        "status": "saved",
        "workgroup": saved,
        "compiled_system_prompt": saved.compile_system_prompt()
    })))
}

/// `DELETE /api/v1/workgroups/:workgroup_id` — Deletes a Problem Workgroup.
pub async fn delete_workgroup_handler(
    State(state): State<crate::handlers::AppState>,
    headers: HeaderMap,
    AxumPath(workgroup_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let reg = global_workgroups();
    let mut guard = reg.write().await;
    let deleted = guard.delete(&workgroup_id);
    Ok(Json(serde_json::json!({
        "status": if deleted { "deleted" } else { "not_found" },
        "id": workgroup_id
    })))
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttachRulePayload {
    pub id: Option<String>,
    pub title: String,
    pub file_path: Option<String>,
    pub inline_markdown: Option<String>,
    pub enforce_strict: Option<bool>,
}

/// `POST /api/v1/workgroups/:workgroup_id/rules` — Attaches a Markdown (`.md`) rule file or inline rule.
pub async fn attach_markdown_rule_handler(
    State(state): State<crate::handlers::AppState>,
    headers: HeaderMap,
    AxumPath(workgroup_id): AxumPath<String>,
    Json(payload): Json<AttachRulePayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let rule_id = payload.id.unwrap_or_else(|| {
        let slug: String = payload
            .title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        format!("rule-{}", slug.trim_matches('-'))
    });

    let rule = MarkdownRuleAttachment {
        id: rule_id,
        title: payload.title,
        file_path: payload.file_path.filter(|s| !s.trim().is_empty()),
        inline_markdown: payload.inline_markdown.filter(|s| !s.trim().is_empty()),
        enforce_strict: payload.enforce_strict.unwrap_or(true),
    };

    let reg = global_workgroups();
    let mut guard = reg.write().await;
    let updated = guard
        .attach_rule(&workgroup_id, rule)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({
        "status": "rule_attached",
        "workgroup": updated,
        "compiled_system_prompt": updated.compile_system_prompt()
    })))
}

/// `DELETE /api/v1/workgroups/:workgroup_id/rules/:rule_id` — Removes an attached Markdown rule.
pub async fn remove_markdown_rule_handler(
    State(state): State<crate::handlers::AppState>,
    headers: HeaderMap,
    AxumPath((workgroup_id, rule_id)): AxumPath<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let reg = global_workgroups();
    let mut guard = reg.write().await;
    let wg = guard.groups.get_mut(&workgroup_id).ok_or(StatusCode::NOT_FOUND)?;
    wg.markdown_rules.retain(|r| r.id != rule_id);
    let updated = wg.clone();
    guard.save_to_disk();

    Ok(Json(serde_json::json!({
        "status": "rule_removed",
        "workgroup": updated,
        "compiled_system_prompt": updated.compile_system_prompt()
    })))
}

/// `GET /api/v1/workgroups/:workgroup_id/preview` — Previews the compiled `.md` rules and agent roster.
pub async fn preview_workgroup_prompt_handler(
    AxumPath(workgroup_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let reg = global_workgroups();
    let guard = reg.read().await;
    let wg = guard.get(&workgroup_id).ok_or(StatusCode::NOT_FOUND)?;
    let resolved_rules: Vec<serde_json::Value> = wg
        .markdown_rules
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "file_path": r.file_path,
                "enforce_strict": r.enforce_strict,
                "resolved_preview": r.resolve_content(1200),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "workgroup_id": wg.id,
        "name": wg.name,
        "problem_domain": wg.problem_domain,
        "execution_strategy": wg.execution_strategy,
        "members": wg.members,
        "resolved_rules": resolved_rules,
        "compiled_system_prompt": wg.compile_system_prompt(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_workgroups_include_coding_and_image_gen() {
        let reg = WorkgroupRegistry::new_with_defaults(None);
        let coding = reg.get("wg-coding").expect("wg-coding should exist");
        assert_eq!(coding.problem_domain, "coding");
        assert!(coding.members.iter().any(|m| m.agent_id.contains("qwen")));
        assert!(coding.members.iter().any(|m| m.agent_id.contains("gemini")));

        let img = reg.get("wg-image-gen").expect("wg-image-gen should exist");
        assert_eq!(img.problem_domain, "image_generation");
        assert!(img.members.iter().any(|m| m.agent_id.contains("deepseek")));
        assert!(img.members.iter().any(|m| m.agent_id.contains("copilot")));

        let reasoning = reg
            .get("wg-reasoning-debug")
            .expect("wg-reasoning-debug should exist");
        assert_eq!(reasoning.problem_domain, "reasoning_debugging");
        assert!(reasoning.members.iter().any(|m| m.agent_id.contains("deepseek")));
        assert!(reasoning.members.iter().any(|m| m.agent_id.contains("qwen")));
    }

    #[test]
    fn test_upsert_custom_workgroup_and_attach_md_rules() {
        let mut reg = WorkgroupRegistry::new_with_defaults(None);
        let custom = ProblemWorkgroup {
            id: "".to_string(),
            name: "Audio & CAD Kinematics".to_string(),
            problem_domain: "cad_math".to_string(),
            icon: "📐".to_string(),
            color: "#3fb950".to_string(),
            description: "Pure math B-rep and kinematics solver group".to_string(),
            execution_strategy: "pipeline".to_string(),
            members: vec![
                WorkgroupMember {
                    agent_id: "qwen3.8:27b".to_string(),
                    display_name: "Qwen 3.8 27B".to_string(),
                    node_id: "PC-1".to_string(),
                    role: WorkgroupRole::PrimaryBuilder,
                },
                WorkgroupMember {
                    agent_id: "deepseek-r1:1.5b".to_string(),
                    display_name: "DeepSeek-R1".to_string(),
                    node_id: "PC-2".to_string(),
                    role: WorkgroupRole::SanityVerifier,
                },
            ],
            markdown_rules: vec![],
            trigger_keywords: vec!["kinematics".to_string(), "brep".to_string()],
            tasks_completed: 0,
        };

        let saved = reg.upsert(custom);
        assert_eq!(saved.id, "wg-audio---cad-kinematics");

        let updated = reg
            .attach_rule(
                &saved.id,
                MarkdownRuleAttachment {
                    id: "rule-euler".to_string(),
                    title: "Euler-Poincare Invariant".to_string(),
                    file_path: None,
                    inline_markdown: Some("Always verify V - E + F = 2 for closed genus-0 meshes.".to_string()),
                    enforce_strict: true,
                },
            )
            .expect("should attach rule");

        assert_eq!(updated.markdown_rules.len(), 1);
        let compiled = updated.compile_system_prompt();
        assert!(compiled.contains("Euler-Poincare Invariant"));
        assert!(compiled.contains("V - E + F = 2"));
        assert!(compiled.contains("Qwen 3.8 27B"));
    }

    #[test]
    fn test_markdown_rule_blocks_vault_traversal() {
        let malicious_rule = MarkdownRuleAttachment {
            id: "rule-evil".to_string(),
            title: "Attempt Vault Read".to_string(),
            file_path: Some("./docs/../../.secrets/agenttrace.env".to_string()),
            inline_markdown: None,
            enforce_strict: true,
        };
        let content = malicious_rule.resolve_content(1000);
        assert!(content.contains("BLOCKED BY SPATIAL VAULT POLICY"));
    }

    #[test]
    fn test_match_workgroup_by_prompt_keywords() {
        let reg = WorkgroupRegistry::new_with_defaults(None);
        let matched_img = reg
            .match_workgroup_by_prompt("Generate an SVG icon mockup for the login screen")
            .expect("should match image gen workgroup");
        assert_eq!(matched_img.id, "wg-image-gen");

        let matched_code = reg
            .match_workgroup_by_prompt("Implement a Rust struct and unit test for the parser")
            .expect("should match coding workgroup");
        assert_eq!(matched_code.id, "wg-coding");
    }
}
