//! Background tasks subsystem coordinator module.

pub mod handlers;
pub mod manager;
pub mod types;

#[cfg(test)]
mod tests;

pub use handlers::{cancel_task_handler, get_task_handler, list_tasks_handler};
pub use manager::{global_tasks, TaskManager};
pub use types::{TaskRecord, TaskStatus};
