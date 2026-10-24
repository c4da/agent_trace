//! Unit tests for TaskManager lifecycle.

use super::manager::TaskManager;
use super::types::TaskStatus;

#[test]
fn test_task_manager_lifecycle() {
    let mut mgr = TaskManager::new();
    let record = mgr.register(
        "task-123".to_string(),
        "Test Simulation".to_string(),
        Some("PC-1-Antigravity".to_string()),
        Some("PC-1".to_string()),
    );
    assert_eq!(record.id, "task-123");
    assert_eq!(record.status, TaskStatus::Running);
    assert_eq!(record.progress, 0);

    // Append log & progress
    let updated = mgr.append_log("task-123", 50, "Halfway done...").unwrap();
    assert_eq!(updated.progress, 50);
    assert!(updated.log_tail.contains("Halfway done..."));

    // Finish
    let finished = mgr.finish("task-123", true, "All done!").unwrap();
    assert_eq!(finished.status, TaskStatus::Completed);
    assert_eq!(finished.progress, 100);
}

#[test]
fn test_task_manager_cancellation() {
    let mut mgr = TaskManager::new();
    mgr.register(
        "task-cancel".to_string(),
        "Long job".to_string(),
        None,
        None,
    );
    let cancelled = mgr.cancel("task-cancel").unwrap();
    assert_eq!(cancelled.status, TaskStatus::Cancelled);
    assert!(cancelled.log_tail.contains("cancelled by operator"));
}
