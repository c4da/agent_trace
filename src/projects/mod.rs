//! Projects, Workspaces, and Persistent Conversation Context subsystem.

pub mod grounding;
pub mod handlers;
pub mod scanner;
pub mod storage;
pub mod types;

#[cfg(test)]
mod tests;

pub use handlers::{
    append_message_handler, create_conversation_handler, delete_conversation_handler,
    get_conversation_handler, list_project_conversations_handler, list_projects_handler,
    register_project_handler, set_active_project_handler, update_conversation_handler,
    update_project_handler,
};

pub use scanner::discover_projects;
pub use storage::{global_project_store, ProjectStore};
pub use types::{
    AppendMessagePayload, ConversationMessage, ConversationRecord, CreateConversationPayload,
    ProjectRecord, SetActiveProjectPayload, UpdateConversationPayload,
};
