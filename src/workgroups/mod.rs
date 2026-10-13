//! Problem Workgroups & Markdown Rule Attachment Engine (`src/workgroups/mod.rs`).
//!
//! Decomposed into modular SRP submodules:
//! - `types.rs`: Core data structures (`ProblemWorkgroup`, `WorkgroupMember`, `WorkgroupRole`, `MarkdownRuleAttachment`).
//! - `compiler.rs`: System prompt compilation logic.
//! - `presets.rs`: Built-in workgroup templates and cluster agent catalog.
//! - `registry.rs`: Thread-safe in-memory registry and disk persistence.
//! - `scanner.rs`: Workspace markdown rule discovery scanner.
//! - `yaml.rs`: Declarative YAML serialization and parsing.
//! - `handlers.rs`: Axum REST API endpoints.

mod compiler;
mod handlers;
mod presets;
mod registry;
mod scanner;
mod types;
mod yaml;

#[cfg(test)]
mod tests;

pub use handlers::{
    apply_workgroup_yaml_handler, attach_markdown_rule_handler, delete_workgroup_handler,
    get_workgroup_yaml_handler, global_workgroups, list_workgroups_handler,
    preview_workgroup_prompt_handler, remove_markdown_rule_handler, upsert_workgroup_handler,
    ApplyWorkgroupYamlPayload, AttachRulePayload, UpsertWorkgroupPayload,
};
pub use presets::{available_agents_catalog, default_workgroups};
pub use registry::WorkgroupRegistry;
pub use scanner::discover_workspace_markdown_files;
pub use types::{
    AvailableAgentInfo, DiscoveredMarkdownFile, MarkdownRuleAttachment, ProblemWorkgroup,
    WorkgroupMember, WorkgroupRole,
};
