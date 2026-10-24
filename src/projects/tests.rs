//! Unit tests for Projects and Persistent Conversation Contexts.

use super::storage::ProjectStore;
use super::types::{AppendMessagePayload, CreateConversationPayload, UpdateConversationPayload};

#[test]
fn test_project_store_lifecycle() {
    let mut store = ProjectStore::new();
    let projects = store.list_projects();
    assert!(!projects.is_empty(), "Should discover projects in C:\\cada\\projects or workspace");

    let first_proj_id = projects[0].id.clone();
    assert!(store.set_active_project(&first_proj_id));
    assert_eq!(store.get_active_project_id(), first_proj_id);

    // Create a conversation
    let conv = store.create_conversation(CreateConversationPayload {
        project_id: first_proj_id.clone(),
        title: Some("Test Invariant Session".to_string()),
        active_workgroup: Some("wg-coding".to_string()),
        context_summary: Some("Do not touch RustCAD; enforce Euler chi=2".to_string()),
    });

    assert_eq!(conv.project_id, first_proj_id);
    assert_eq!(conv.title, "Test Invariant Session");
    assert!(conv.context_summary.contains("Euler chi=2"));

    // Append a message
    let with_msg = store
        .append_message(
            &conv.id,
            AppendMessagePayload {
                role: "user".to_string(),
                content: "Refactor B-rep geometry".to_string(),
                stage_info: Some("PC-1 Lead".to_string()),
                reasoning: None,
                node_id: Some("PC-1".to_string()),
                agent_id: Some("qwen3.8:27b".to_string()),
                duration_ms: Some(1250.0),
            },
        )
        .unwrap();

    assert_eq!(with_msg.messages.len(), 1);
    assert_eq!(with_msg.messages[0].content, "Refactor B-rep geometry");
    assert_eq!(with_msg.messages[0].node_id.as_deref(), Some("PC-1"));

    // Update conversation context summary
    let updated = store
        .update_conversation(
            &conv.id,
            UpdateConversationPayload {
                title: Some("Updated Session Title".to_string()),
                context_summary: Some("Updated context directives".to_string()),
                active_workgroup: None,
            },
        )
        .unwrap();

    assert_eq!(updated.title, "Updated Session Title");
    assert_eq!(updated.context_summary, "Updated context directives");

    // Delete conversation
    assert!(store.delete_conversation(&conv.id));
    assert!(store.get_conversation(&conv.id).is_none());
}

#[test]
fn test_project_upsert_and_update_root() {
    let mut store = ProjectStore::new();
    let new_proj = store.upsert_project(super::types::RegisterProjectPayload {
        name: "Custom CAD Lab".to_string(),
        path: "C:\\cada\\projects\\custom_cad_lab".to_string(),
        tech_stack: Some("Rust / Cargo CAD".to_string()),
    });

    assert_eq!(new_proj.name, "Custom CAD Lab");
    assert_eq!(new_proj.path, "C:\\cada\\projects\\custom_cad_lab");
    assert_eq!(store.get_active_project_id(), new_proj.id);

    // Update root folder path
    let updated = store
        .update_project(
            &new_proj.id,
            super::types::UpdateProjectPayload {
                name: Some("Custom CAD Lab V2".to_string()),
                path: Some("C:\\cada\\projects\\custom_cad_lab_v2".to_string()),
            },
        )
        .expect("Project should be updated");

    assert_eq!(updated.name, "Custom CAD Lab V2");
    assert_eq!(updated.path, "C:\\cada\\projects\\custom_cad_lab_v2");
}

