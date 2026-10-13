//! Unit tests for Problem Workgroups, YAML roundtrip, rule attachment, and prompt matching.

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

    let docs = reg
        .get("wg-documentation")
        .expect("wg-documentation should exist");
    assert_eq!(docs.problem_domain, "documentation");
    assert!(docs.members.iter().any(|m| m.agent_id.contains("deepseek")));
    assert_eq!(docs.markdown_rules.len(), 1);
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
                parent_agent_id: None,
            },
            WorkgroupMember {
                agent_id: "deepseek-r1:1.5b".to_string(),
                display_name: "DeepSeek-R1".to_string(),
                node_id: "PC-2".to_string(),
                role: WorkgroupRole::SanityVerifier,
                parent_agent_id: Some("qwen3.8:27b".to_string()),
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

    let matched_docs = reg
        .match_workgroup_by_prompt("Update the documentation and README with step-by-step tutorial guides")
        .expect("should match documentation workgroup");
    assert_eq!(matched_docs.id, "wg-documentation");
}

#[test]
fn test_workgroup_yaml_roundtrip() {
    let reg = WorkgroupRegistry::new_with_defaults(None);
    let coding = reg.get("wg-coding").expect("wg-coding should exist");
    let yaml_str = coding.to_yaml();
    assert!(yaml_str.contains("id: wg-coding"));
    assert!(yaml_str.contains("parent: gemini-pro"));

    let parsed = ProblemWorkgroup::from_yaml(&yaml_str).expect("should parse valid YAML");
    assert_eq!(parsed.id, "wg-coding");
    assert_eq!(parsed.members.len(), 3);
    assert_eq!(parsed.members[0].agent_id, "gemini-pro");
    assert_eq!(parsed.members[0].parent_agent_id, None);
    assert_eq!(parsed.members[1].agent_id, "qwen3.5:9b");
    assert_eq!(
        parsed.members[1].parent_agent_id.as_deref(),
        Some("gemini-pro")
    );
    assert_eq!(parsed.markdown_rules.len(), 1);
}
