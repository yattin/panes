//! Tests for Tools Module

use claude_code_rs::tools::ToolRegistry;
use serde_json::json;

#[tokio::test]
async fn test_tool_registry_creation() {
    let registry = ToolRegistry::new();
    let tools = registry.list();

    // Should have 9 tools now (6 original + 3 new)
    assert!(tools.len() >= 6);
}

#[tokio::test]
async fn test_file_read_tool() {
    let registry = ToolRegistry::new();
    let tool = registry
        .get("file_read")
        .expect("file_read tool should exist");

    assert_eq!(tool.name(), "file_read");
    assert!(!tool.description().is_empty());
}

#[tokio::test]
async fn test_file_read_offset_limit() {
    let registry = ToolRegistry::new();
    let temp = tempfile::tempdir().expect("tempdir should be created");
    let file_path = temp.path().join("sample.txt");
    std::fs::write(&file_path, "one\ntwo\nthree\nfour\n").expect("sample file should be written");

    let result = registry
        .execute(
            "file_read",
            json!({
                "file_path": file_path.to_string_lossy(),
                "offset": 2,
                "limit": 2
            }),
        )
        .await
        .expect("file_read should succeed");

    assert!(result.content.contains("     2\ttwo"));
    assert!(result.content.contains("     3\tthree"));
    assert!(!result.content.contains("     4\tfour"));
    assert_eq!(result.metadata["returned_lines"], json!(2));
}

#[tokio::test]
async fn test_file_edit_rejects_empty_old_content() {
    let registry = ToolRegistry::new();
    let temp = tempfile::tempdir().expect("tempdir should be created");
    let file_path = temp.path().join("sample.txt");
    std::fs::write(&file_path, "abc").expect("sample file should be written");

    let error = registry
        .execute(
            "file_edit",
            json!({
                "file_path": file_path.to_string_lossy(),
                "old_content": "",
                "new_content": "x",
                "replace_all": true
            }),
        )
        .await
        .expect_err("empty old_content should be rejected");

    assert_eq!(error.code.as_deref(), Some("empty_old_content"));
    assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "abc");
}

#[tokio::test]
async fn test_git_operations_tool() {
    let registry = ToolRegistry::new();
    let tool = registry
        .get("git_operations")
        .expect("git_operations tool should exist");

    assert_eq!(tool.name(), "git_operations");
    assert!(tool.description().contains("Git"));
}

#[tokio::test]
async fn test_task_management_tool() {
    let registry = ToolRegistry::new();
    let tool = registry
        .get("task_management")
        .expect("task_management tool should exist");

    assert_eq!(tool.name(), "task_management");
    assert!(tool.description().contains("task"));
}

#[tokio::test]
async fn test_note_edit_tool() {
    let registry = ToolRegistry::new();
    let tool = registry
        .get("note_edit")
        .expect("note_edit tool should exist");

    assert_eq!(tool.name(), "note_edit");
    assert!(tool.description().contains("note"));
}

#[tokio::test]
async fn test_task_create_and_list() {
    let registry = ToolRegistry::new();

    // Create a task
    let create_result = registry
        .execute(
            "task_management",
            json!({
                "operation": "create",
                "subject": "Test Task",
                "description": "This is a test task"
            }),
        )
        .await;

    assert!(create_result.is_ok());

    // List tasks
    let list_result = registry
        .execute(
            "task_management",
            json!({
                "operation": "list"
            }),
        )
        .await;

    assert!(list_result.is_ok());
}

#[tokio::test]
async fn test_note_create_and_search() {
    let registry = ToolRegistry::new();

    // Create a note
    let create_result = registry
        .execute(
            "note_edit",
            json!({
                "operation": "create",
                "title": "Test Note",
                "content": "This is a test note content",
                "tags": ["test", "example"]
            }),
        )
        .await;

    assert!(create_result.is_ok());

    // Search notes
    let search_result = registry
        .execute(
            "note_edit",
            json!({
                "operation": "search",
                "search_query": "test"
            }),
        )
        .await;

    assert!(search_result.is_ok());
}
