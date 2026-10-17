//! Preset Problem Workgroups and available cluster agent catalog.

use super::types::{
    AvailableAgentInfo, MarkdownRuleAttachment, ProblemWorkgroup, WorkgroupMember, WorkgroupRole,
};
use std::collections::HashMap;

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

/// Constructs the 7 built-in default Problem Workgroups with their attached Markdown rules.
pub fn default_workgroups() -> HashMap<String, ProblemWorkgroup> {
    let mut groups = HashMap::new();

    // 1. Coding & Multi-File Architecture Workgroup
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
                agent_id: "gemini-pro".to_string(),
                display_name: "Gemini / Antigravity".to_string(),
                node_id: "PC-1".to_string(),
                role: WorkgroupRole::LeadArchitect,
                parent_agent_id: None,
            },
            WorkgroupMember {
                agent_id: "qwen3.5:9b".to_string(),
                display_name: "Qwen 3.5 (9B Code)".to_string(),
                node_id: "PC-1".to_string(),
                role: WorkgroupRole::PrimaryBuilder,
                parent_agent_id: Some("gemini-pro".to_string()),
            },
            WorkgroupMember {
                agent_id: "qwen3.8:27b".to_string(),
                display_name: "Qwen 3.8 (27B Heavy)".to_string(),
                node_id: "PC-1".to_string(),
                role: WorkgroupRole::SanityVerifier,
                parent_agent_id: Some("qwen3.5:9b".to_string()),
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

    // 2. Image & Visual Asset Generation Workgroup
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
                parent_agent_id: None,
            },
            WorkgroupMember {
                agent_id: "copilot-cli".to_string(),
                display_name: "GitHub Copilot CLI".to_string(),
                node_id: "PC-1-Copilot".to_string(),
                role: WorkgroupRole::PrimaryBuilder,
                parent_agent_id: Some("deepseek-r1:1.5b".to_string()),
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

    // 3. Security Audit & Spatial Containment Workgroup
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
                parent_agent_id: None,
            },
            WorkgroupMember {
                agent_id: "qwen3.5:9b".to_string(),
                display_name: "Qwen 3.5 (9B Code)".to_string(),
                node_id: "PC-1".to_string(),
                role: WorkgroupRole::PrimaryBuilder,
                parent_agent_id: None,
            },
            WorkgroupMember {
                agent_id: "copilot-cli".to_string(),
                display_name: "GitHub Copilot CLI".to_string(),
                node_id: "PC-1-Copilot".to_string(),
                role: WorkgroupRole::ToolExecutor,
                parent_agent_id: None,
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
        id: "wg-ui-development".to_string(),
        name: "UI Development & Ergonomics".to_string(),
        problem_domain: "ui_design".to_string(),
        icon: "🖥️".to_string(),
        color: "#d29922".to_string(),
        description: "Builds high-density terminal consoles (Ghostty/k9s style), scenario toolbars, and keyboard-first forensic workflows.".to_string(),
        execution_strategy: "pipeline".to_string(),
        members: vec![
            WorkgroupMember {
                agent_id: "deepseek-r1:1.5b".to_string(),
                display_name: "DeepSeek-R1 (1.5B CUDA)".to_string(),
                node_id: "PC-2".to_string(),
                role: WorkgroupRole::SanityVerifier,
                parent_agent_id: None,
            },
            WorkgroupMember {
                agent_id: "qwen3.5:9b".to_string(),
                display_name: "Qwen 3.5 (9B Fast Code)".to_string(),
                node_id: "PC-1".to_string(),
                role: WorkgroupRole::PrimaryBuilder,
                parent_agent_id: Some("deepseek-r1:1.5b".to_string()),
            },
        ],
        markdown_rules: vec![
            MarkdownRuleAttachment {
                id: "rule-work-first-ui".to_string(),
                title: "Work-First UI Blueprint & Ergonomics Standard".to_string(),
                file_path: Some("docs/UI_DESIGN_PHILOSOPHY_AND_PROMPT.md".to_string()),
                inline_markdown: Some(
                    "- High density: 30-50 records visible without scrolling.\n- Scenario-first toolbar & 3-part explainer cards.\n- Manual-first operator toolbox (j/k navigation, P pause, M sandbox, F flag).".to_string(),
                ),
                enforce_strict: true,
            },
        ],
        trigger_keywords: vec![
            "ui".to_string(),
            "frontend".to_string(),
            "dashboard".to_string(),
            "console".to_string(),
            "tui".to_string(),
            "html".to_string(),
            "css".to_string(),
            "layout".to_string(),
            "widget".to_string(),
            "view".to_string(),
            "button".to_string(),
            "toolbar".to_string(),
            "keyboard".to_string(),
        ],
        tasks_completed: 8,
    };

    // 5. Reasoning & Coding Issue Debugging Workgroup
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
                agent_id: "qwen3.5:9b".to_string(),
                display_name: "Qwen 3.5 (9B Code & Debug)".to_string(),
                node_id: "PC-1".to_string(),
                role: WorkgroupRole::LeadArchitect,
                parent_agent_id: None,
            },
            WorkgroupMember {
                agent_id: "deepseek-r1:1.5b".to_string(),
                display_name: "DeepSeek-R1 (1.5B CUDA)".to_string(),
                node_id: "PC-2".to_string(),
                role: WorkgroupRole::SanityVerifier,
                parent_agent_id: Some("qwen3.5:9b".to_string()),
            },
            WorkgroupMember {
                agent_id: "copilot-cli".to_string(),
                display_name: "GitHub Copilot CLI".to_string(),
                node_id: "PC-1-Copilot".to_string(),
                role: WorkgroupRole::ToolExecutor,
                parent_agent_id: Some("qwen3.5:9b".to_string()),
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

    // 6. Code Refactoring & Anti-Monolith Architecture Workgroup
    let wg_refactor = ProblemWorkgroup {
        id: "wg-code-refactor".to_string(),
        name: "Code Refactoring & Architecture".to_string(),
        problem_domain: "code_refactoring".to_string(),
        icon: "🔨".to_string(),
        color: "#58a6ff".to_string(),
        description: "Anti-monolith decomposition, Single Responsibility Principle modularization, pure math crate extraction, and feature-per-file refactoring.".to_string(),
        execution_strategy: "pipeline".to_string(),
        members: vec![
            WorkgroupMember {
                agent_id: "deepseek-r1:1.5b".to_string(),
                display_name: "DeepSeek-R1 (1.5B CUDA)".to_string(),
                node_id: "PC-2".to_string(),
                role: WorkgroupRole::LeadArchitect,
                parent_agent_id: None,
            },
            WorkgroupMember {
                agent_id: "qwen3.8:27b".to_string(),
                display_name: "Qwen 3.8 (27B Heavy Reasoning)".to_string(),
                node_id: "PC-1".to_string(),
                role: WorkgroupRole::PrimaryBuilder,
                parent_agent_id: Some("deepseek-r1:1.5b".to_string()),
            },
        ],
        markdown_rules: vec![
            MarkdownRuleAttachment {
                id: "rule-code-refactor".to_string(),
                title: "Anti-Monolith & Feature-per-File Refactoring Rules".to_string(),
                file_path: Some("docs/CODE_REFACTOR_AND_ENGINEERING_RULES.md".to_string()),
                inline_markdown: Some(
                    "- Decompose files exceeding 800 lines into cohesive, single-responsibility modules.\n- Top-level mod.rs/lib.rs acts strictly as coordinators, never hoarding raw logic.\n- Pure math crates depend ONLY on lightweight math (glam, serde) with ZERO UI dependencies.\n- Pair all refactors with headless unit & invariant tests.".to_string(),
                ),
                enforce_strict: true,
            },
        ],
        trigger_keywords: vec![
            "refactor".to_string(),
            "clean code".to_string(),
            "anti-monolith".to_string(),
            "modularize".to_string(),
            "split file".to_string(),
            "srp".to_string(),
            "architecture".to_string(),
            "decompose".to_string(),
            "restructure".to_string(),
            "god object".to_string(),
        ],
        tasks_completed: 14,
    };

    // 7. Documentation & Technical Writing Workgroup
    let wg_docs = ProblemWorkgroup {
        id: "wg-documentation".to_string(),
        name: "Documentation & Technical Writing".to_string(),
        problem_domain: "documentation".to_string(),
        icon: "📚".to_string(),
        color: "#f59e0b".to_string(),
        description: "Authors, audits, and maintains modular documentation, scenario walkthroughs, API references, and root README with 'Line 0 How-To' index.".to_string(),
        execution_strategy: "auto".to_string(),
        members: vec![
            WorkgroupMember {
                agent_id: "deepseek-r1:1.5b".to_string(),
                display_name: "DeepSeek-R1 (1.5B CUDA)".to_string(),
                node_id: "PC-2".to_string(),
                role: WorkgroupRole::LeadArchitect,
                parent_agent_id: None,
            },
        ],
        markdown_rules: vec![
            MarkdownRuleAttachment {
                id: "rule-documentation-standards".to_string(),
                title: "Documentation Standards & Line 0 How-To Rules".to_string(),
                file_path: Some("docs/DOCUMENTATION_STANDARDS.md".to_string()),
                inline_markdown: Some(
                    "- Root README must place the How-To table and 30s quick recipes on Line 0.\n- Feature-per-file modular docs in docs/ (no monolithic multi-thousand line files).\n- Never hardcode personal paths; use repo-relative paths.\n- Every guide must provide Problem -> Command -> Concrete Proof.\n- Synchronize badge test counts with real cargo test results.".to_string(),
                ),
                enforce_strict: true,
            },
        ],
        trigger_keywords: vec![
            "documentation".to_string(),
            "docs".to_string(),
            "readme".to_string(),
            "guide".to_string(),
            "tutorial".to_string(),
            "use case".to_string(),
            "how to".to_string(),
            "manual".to_string(),
            "api doc".to_string(),
            "markdown".to_string(),
        ],
        tasks_completed: 5,
    };

    groups.insert(wg_coding.id.clone(), wg_coding);
    groups.insert(wg_image.id.clone(), wg_image);
    groups.insert(wg_security.id.clone(), wg_security);
    groups.insert(wg_ui.id.clone(), wg_ui);
    groups.insert(wg_reasoning.id.clone(), wg_reasoning);
    groups.insert(wg_refactor.id.clone(), wg_refactor);
    groups.insert(wg_docs.id.clone(), wg_docs);

    groups
}
