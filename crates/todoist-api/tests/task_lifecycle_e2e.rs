//! End-to-end tests for task lifecycle operations.
//!
//! These tests validate task CRUD, movement, completion, subtasks, and ordering
//! against the real Todoist API.
//!
//! Run with: cargo test --package todoist-api --features e2e --test task_lifecycle_e2e
//!
//! Tests follow the naming convention from specs/E2E_TEST_SPEC.md sections 1.1-1.5.

#![cfg(feature = "e2e")]

use std::fs;
use todoist_api::client::TodoistClient;
use todoist_api::sync::{SyncCommand, SyncRequest};

// ============================================================================
// Test Helpers
// ============================================================================

fn get_test_token() -> Option<String> {
    // Try to read from .env.local at workspace root
    for path in &[".env.local", "../../.env.local", "../../../.env.local"] {
        if let Ok(contents) = fs::read_to_string(path) {
            for line in contents.lines() {
                if let Some(token) = line
                    .strip_prefix("TODOIST_TEST_API_TOKEN=")
                    .or_else(|| line.strip_prefix("todoist_test_api_key="))
                {
                    return Some(token.trim().to_string());
                }
            }
        }
    }

    std::env::var("TODOIST_TEST_API_TOKEN")
        .or_else(|_| std::env::var("TODOIST_TEST_API_KEY"))
        .ok()
}

/// Helper to create a task and return its real ID
async fn create_task(
    client: &TodoistClient,
    content: &str,
    project_id: &str,
    extra_args: Option<serde_json::Value>,
) -> String {
    let temp_id = uuid::Uuid::new_v4().to_string();
    let mut args = serde_json::json!({
        "content": content,
        "project_id": project_id
    });

    if let Some(extra) = extra_args {
        if let Some(obj) = args.as_object_mut() {
            if let Some(extra_obj) = extra.as_object() {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
    }

    let command = SyncCommand::with_temp_id("item_add", &temp_id, args);
    let response = client
        .sync(SyncRequest::with_commands(vec![command]))
        .await
        .expect("item_add failed");

    assert!(
        !response.has_errors(),
        "item_add should succeed: {:?}",
        response.errors()
    );

    response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone()
}

/// Helper to create a project and return its real ID
async fn create_project(client: &TodoistClient, name: &str) -> String {
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        "project_add",
        &temp_id,
        serde_json::json!({ "name": name }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![command]))
        .await
        .expect("project_add failed");

    assert!(
        !response.has_errors(),
        "project_add should succeed: {:?}",
        response.errors()
    );

    response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone()
}

/// Helper to create a section and return its real ID
async fn create_section(client: &TodoistClient, name: &str, project_id: &str) -> String {
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        "section_add",
        &temp_id,
        serde_json::json!({
            "name": name,
            "project_id": project_id
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![command]))
        .await
        .expect("section_add failed");

    assert!(
        !response.has_errors(),
        "section_add should succeed: {:?}",
        response.errors()
    );

    response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone()
}

/// Helper to delete a task
async fn delete_task(client: &TodoistClient, task_id: &str) {
    let command = SyncCommand::new("item_delete", serde_json::json!({"id": task_id}));
    let response = client
        .sync(SyncRequest::with_commands(vec![command]))
        .await
        .expect("item_delete failed");

    assert!(
        !response.has_errors(),
        "item_delete should succeed: {:?}",
        response.errors()
    );
}

/// Helper to delete a project
async fn delete_project(client: &TodoistClient, project_id: &str) {
    let command = SyncCommand::new("project_delete", serde_json::json!({"id": project_id}));
    let response = client
        .sync(SyncRequest::with_commands(vec![command]))
        .await
        .expect("project_delete failed");

    assert!(
        !response.has_errors(),
        "project_delete should succeed: {:?}",
        response.errors()
    );
}

/// Helper to delete a section
async fn delete_section(client: &TodoistClient, section_id: &str) {
    let command = SyncCommand::new("section_delete", serde_json::json!({"id": section_id}));
    let response = client
        .sync(SyncRequest::with_commands(vec![command]))
        .await
        .expect("section_delete failed");

    assert!(
        !response.has_errors(),
        "section_delete should succeed: {:?}",
        response.errors()
    );
}

/// Helper to get the inbox project ID
async fn get_inbox_id(client: &TodoistClient) -> String {
    let response = client.sync(SyncRequest::full_sync()).await.unwrap();
    response
        .projects
        .iter()
        .find(|p| p.inbox_project)
        .expect("Should have inbox project")
        .id
        .clone()
}

/// Helper to sync and find a task by ID
async fn find_task(
    client: &TodoistClient,
    task_id: &str,
) -> Option<todoist_api::sync::Item> {
    let response = client.sync(SyncRequest::full_sync()).await.unwrap();
    response.items.into_iter().find(|i| i.id == task_id)
}

/// Helper to sync and find a section by ID
#[allow(dead_code)]
async fn find_section(
    client: &TodoistClient,
    section_id: &str,
) -> Option<todoist_api::sync::Section> {
    let response = client.sync(SyncRequest::full_sync()).await.unwrap();
    response.sections.into_iter().find(|s| s.id == section_id)
}

// ============================================================================
// 1.1 Basic CRUD Tests
// ============================================================================

#[tokio::test]
async fn test_create_task_minimal() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create task with only content
    let task_id = create_task(&client, "E2E test - minimal task", &inbox_id, None).await;

    // Verify task appears in sync response
    let task = find_task(&client, &task_id).await.expect("Task should exist");

    assert_eq!(task.content, "E2E test - minimal task");
    assert_eq!(task.priority, 1, "Default priority should be 1");
    assert!(task.due.is_none(), "Should have no due date");
    assert!(task.labels.is_empty(), "Should have no labels");
    assert!(!task.checked, "Should not be completed");
    assert!(!task.is_deleted, "Should not be deleted");

    // Clean up
    delete_task(&client, &task_id).await;
}

#[tokio::test]
async fn test_create_task_with_all_fields() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create task with all fields
    let task_id = create_task(
        &client,
        "E2E test - complete task",
        &inbox_id,
        Some(serde_json::json!({
            "description": "Detailed description here",
            "priority": 4,
            "due": {"date": "2025-12-25"},
            "labels": ["e2e-test-label"]
        })),
    )
    .await;

    // Verify all fields persisted
    let task = find_task(&client, &task_id).await.expect("Task should exist");

    assert_eq!(task.content, "E2E test - complete task");
    assert_eq!(task.description, "Detailed description here");
    assert_eq!(task.priority, 4, "Priority should be 4 (p1)");
    assert!(task.due.is_some(), "Should have due date");
    assert_eq!(task.due.as_ref().unwrap().date, "2025-12-25");
    assert!(task.labels.contains(&"e2e-test-label".to_string()));

    // Clean up
    delete_task(&client, &task_id).await;
}

#[tokio::test]
async fn test_update_task_content() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create task
    let task_id = create_task(&client, "E2E test - original content", &inbox_id, None).await;

    // Update content
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "content": "E2E test - modified content"
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![update_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify content changed
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert_eq!(task.content, "E2E test - modified content");

    // Clean up
    delete_task(&client, &task_id).await;
}

#[tokio::test]
async fn test_update_task_description() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create task with no description
    let task_id = create_task(&client, "E2E test - description test", &inbox_id, None).await;

    // Add description
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "description": "New description"
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![update_command]))
        .await
        .unwrap();
    assert!(!response.has_errors());

    // Verify description added
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert_eq!(task.description, "New description");

    // Update description
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "description": "Changed description"
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![update_command]))
        .await
        .unwrap();
    assert!(!response.has_errors());

    // Verify description changed
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert_eq!(task.description, "Changed description");

    // Clean up
    delete_task(&client, &task_id).await;
}

#[tokio::test]
async fn test_delete_task() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create task
    let task_id = create_task(&client, "E2E test - to be deleted", &inbox_id, None).await;

    // Verify task exists
    let task = find_task(&client, &task_id).await;
    assert!(task.is_some(), "Task should exist before deletion");

    // Delete task
    delete_task(&client, &task_id).await;

    // Verify task is deleted (either is_deleted: true or absent)
    let task = find_task(&client, &task_id).await;
    match task {
        Some(t) => assert!(t.is_deleted, "Task should be marked as deleted"),
        None => {} // Task is absent, which is also valid
    }
}

// ============================================================================
// 1.2 Task Movement Tests
// ============================================================================

#[tokio::test]
async fn test_move_task_between_projects() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // Create Project A and Project B
    let project_a_id = create_project(&client, "E2E_Test_ProjectA").await;
    let project_b_id = create_project(&client, "E2E_Test_ProjectB").await;

    // Create task in Project A
    let task_id = create_task(&client, "E2E test - moveable task", &project_a_id, None).await;

    // Verify task is in Project A
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert_eq!(task.project_id, project_a_id);

    // Move task to Project B
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": task_id,
            "project_id": project_b_id
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![move_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify task is now in Project B
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert_eq!(task.project_id, project_b_id);

    // Clean up
    delete_task(&client, &task_id).await;
    delete_project(&client, &project_a_id).await;
    delete_project(&client, &project_b_id).await;
}

#[tokio::test]
async fn test_move_task_to_section() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // Create project with section
    let project_id = create_project(&client, "E2E_Test_SectionProject").await;
    let section_id = create_section(&client, "In Progress", &project_id).await;

    // Create task in project (no section)
    let task_id = create_task(&client, "E2E test - section move", &project_id, None).await;

    // Verify task has no section
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert!(task.section_id.is_none(), "Task should have no section initially");

    // Move task to section
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": task_id,
            "section_id": section_id
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![move_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify task is in section
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert_eq!(task.section_id, Some(section_id.clone()));

    // Clean up
    delete_task(&client, &task_id).await;
    delete_section(&client, &section_id).await;
    delete_project(&client, &project_id).await;
}

#[tokio::test]
async fn test_move_task_out_of_section() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // Create project with section
    let project_id = create_project(&client, "E2E_Test_SectionOutProject").await;
    let section_id = create_section(&client, "Backlog", &project_id).await;

    // Create task in section
    let task_id = create_task(
        &client,
        "E2E test - section move out",
        &project_id,
        Some(serde_json::json!({"section_id": section_id})),
    )
    .await;

    // Verify task is in section
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert_eq!(task.section_id, Some(section_id.clone()));

    // Move task out of section (to project root)
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": task_id,
            "project_id": project_id
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![move_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify task has no section
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert!(
        task.section_id.is_none(),
        "Task should have no section after move"
    );

    // Clean up
    delete_task(&client, &task_id).await;
    delete_section(&client, &section_id).await;
    delete_project(&client, &project_id).await;
}

#[tokio::test]
async fn test_move_task_to_section_in_different_project() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // Create Project A and Project B with section
    let project_a_id = create_project(&client, "E2E_Test_CrossProjectA").await;
    let project_b_id = create_project(&client, "E2E_Test_CrossProjectB").await;
    let section_b_id = create_section(&client, "Done", &project_b_id).await;

    // Create task in Project A
    let task_id = create_task(&client, "E2E test - cross project move", &project_a_id, None).await;

    // Verify task is in Project A
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert_eq!(task.project_id, project_a_id);

    // Move task to section in Project B
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": task_id,
            "section_id": section_b_id
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![move_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify task is in Project B's section
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert_eq!(task.project_id, project_b_id);
    assert_eq!(task.section_id, Some(section_b_id.clone()));

    // Clean up
    delete_task(&client, &task_id).await;
    delete_section(&client, &section_b_id).await;
    delete_project(&client, &project_a_id).await;
    delete_project(&client, &project_b_id).await;
}

// ============================================================================
// 1.3 Task Completion Tests
// ============================================================================

#[tokio::test]
async fn test_complete_task_with_item_close() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create task
    let task_id = create_task(&client, "E2E test - to be closed", &inbox_id, None).await;

    // Verify task is not completed
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert!(!task.checked, "Task should not be completed initially");

    // Complete task with item_close
    let close_command = SyncCommand::new("item_close", serde_json::json!({"id": task_id}));
    let response = client
        .sync(SyncRequest::with_commands(vec![close_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_close should succeed");

    // Verify task is completed
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert!(task.checked, "Task should be completed");

    // Clean up
    delete_task(&client, &task_id).await;
}

#[tokio::test]
async fn test_complete_task_with_item_complete() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create task
    let task_id = create_task(&client, "E2E test - to be completed", &inbox_id, None).await;

    // Complete task with item_complete and completed_at timestamp
    let complete_command = SyncCommand::new(
        "item_complete",
        serde_json::json!({
            "id": task_id,
            "completed_at": "2025-01-26T12:00:00Z"
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![complete_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_complete should succeed");

    // Verify task is completed with timestamp
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert!(task.checked, "Task should be completed");
    // Note: completed_at may or may not be returned in sync response depending on API version

    // Clean up
    delete_task(&client, &task_id).await;
}

#[tokio::test]
async fn test_uncomplete_task() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create and complete task
    let task_id = create_task(&client, "E2E test - to be uncompleted", &inbox_id, None).await;
    let close_command = SyncCommand::new("item_close", serde_json::json!({"id": task_id}));
    let response = client
        .sync(SyncRequest::with_commands(vec![close_command]))
        .await
        .unwrap();
    assert!(!response.has_errors());

    // Verify task is completed
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert!(task.checked, "Task should be completed");

    // Uncomplete task
    let uncomplete_command = SyncCommand::new("item_uncomplete", serde_json::json!({"id": task_id}));
    let response = client
        .sync(SyncRequest::with_commands(vec![uncomplete_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_uncomplete should succeed");

    // Verify task is uncompleted
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert!(!task.checked, "Task should be uncompleted");

    // Clean up
    delete_task(&client, &task_id).await;
}

#[tokio::test]
async fn test_complete_recurring_task() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create recurring task
    let task_id = create_task(
        &client,
        "E2E test - recurring task",
        &inbox_id,
        Some(serde_json::json!({
            "due": {"string": "every day"}
        })),
    )
    .await;

    // Verify task is recurring
    let task = find_task(&client, &task_id).await.expect("Task should exist");
    assert!(task.due.is_some(), "Task should have due date");
    let original_due = task.due.as_ref().unwrap().date.clone();
    assert!(
        task.due.as_ref().unwrap().is_recurring,
        "Task should be recurring"
    );

    // Complete recurring task
    let close_command = SyncCommand::new("item_close", serde_json::json!({"id": task_id}));
    let response = client
        .sync(SyncRequest::with_commands(vec![close_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_close should succeed");

    // Verify task still exists, due date advanced, not checked
    let task = find_task(&client, &task_id).await.expect("Recurring task should still exist");
    assert!(!task.checked, "Recurring task should not be checked after completion");
    assert!(task.due.is_some(), "Task should still have due date");
    let new_due = task.due.as_ref().unwrap().date.clone();
    assert_ne!(
        original_due, new_due,
        "Due date should have advanced to next occurrence"
    );

    // Clean up
    delete_task(&client, &task_id).await;
}

// ============================================================================
// 1.4 Subtask (Parent-Child) Tests
// ============================================================================

#[tokio::test]
async fn test_create_subtask() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create parent task
    let parent_id = create_task(&client, "E2E test - parent task", &inbox_id, None).await;

    // Create child task
    let child_id = create_task(
        &client,
        "E2E test - child task",
        &inbox_id,
        Some(serde_json::json!({"parent_id": parent_id})),
    )
    .await;

    // Verify child's parent_id
    let child = find_task(&client, &child_id).await.expect("Child should exist");
    assert_eq!(child.parent_id, Some(parent_id.clone()));

    // Clean up (delete child first, then parent)
    delete_task(&client, &child_id).await;
    delete_task(&client, &parent_id).await;
}

#[tokio::test]
async fn test_create_nested_subtasks() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create hierarchy: A -> B -> C
    let task_a_id = create_task(&client, "E2E test - task A (root)", &inbox_id, None).await;
    let task_b_id = create_task(
        &client,
        "E2E test - task B (child of A)",
        &inbox_id,
        Some(serde_json::json!({"parent_id": task_a_id})),
    )
    .await;
    let task_c_id = create_task(
        &client,
        "E2E test - task C (child of B)",
        &inbox_id,
        Some(serde_json::json!({"parent_id": task_b_id})),
    )
    .await;

    // Verify hierarchy
    let task_a = find_task(&client, &task_a_id).await.expect("Task A should exist");
    let task_b = find_task(&client, &task_b_id).await.expect("Task B should exist");
    let task_c = find_task(&client, &task_c_id).await.expect("Task C should exist");

    assert!(task_a.parent_id.is_none(), "A should have no parent");
    assert_eq!(task_b.parent_id, Some(task_a_id.clone()), "B's parent should be A");
    assert_eq!(task_c.parent_id, Some(task_b_id.clone()), "C's parent should be B");

    // Clean up (delete in reverse order)
    delete_task(&client, &task_c_id).await;
    delete_task(&client, &task_b_id).await;
    delete_task(&client, &task_a_id).await;
}

#[tokio::test]
async fn test_move_subtask_to_different_parent() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create parent A, parent B, and child C under A
    let parent_a_id = create_task(&client, "E2E test - parent A", &inbox_id, None).await;
    let parent_b_id = create_task(&client, "E2E test - parent B", &inbox_id, None).await;
    let child_c_id = create_task(
        &client,
        "E2E test - child C",
        &inbox_id,
        Some(serde_json::json!({"parent_id": parent_a_id})),
    )
    .await;

    // Verify C's parent is A
    let child_c = find_task(&client, &child_c_id).await.expect("Child should exist");
    assert_eq!(child_c.parent_id, Some(parent_a_id.clone()));

    // Move C to be a child of B
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": child_c_id,
            "parent_id": parent_b_id
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![move_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify C's parent is now B
    let child_c = find_task(&client, &child_c_id).await.expect("Child should exist");
    assert_eq!(child_c.parent_id, Some(parent_b_id.clone()));

    // Clean up
    delete_task(&client, &child_c_id).await;
    delete_task(&client, &parent_a_id).await;
    delete_task(&client, &parent_b_id).await;
}

#[tokio::test]
async fn test_promote_subtask_to_task() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create parent and child
    let parent_id = create_task(&client, "E2E test - parent to promote from", &inbox_id, None).await;
    let child_id = create_task(
        &client,
        "E2E test - child to be promoted",
        &inbox_id,
        Some(serde_json::json!({"parent_id": parent_id})),
    )
    .await;

    // Verify child has parent
    let child = find_task(&client, &child_id).await.expect("Child should exist");
    assert_eq!(child.parent_id, Some(parent_id.clone()));

    // Promote child to top-level task (move to project root)
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": child_id,
            "project_id": inbox_id
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![move_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify child has no parent
    let child = find_task(&client, &child_id).await.expect("Child should exist");
    assert!(child.parent_id.is_none(), "Child should have no parent after promotion");

    // Clean up
    delete_task(&client, &child_id).await;
    delete_task(&client, &parent_id).await;
}

#[tokio::test]
async fn test_complete_parent_with_subtasks() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create parent with 2 subtasks
    let parent_id = create_task(&client, "E2E test - parent with subtasks", &inbox_id, None).await;
    let child1_id = create_task(
        &client,
        "E2E test - subtask 1",
        &inbox_id,
        Some(serde_json::json!({"parent_id": parent_id})),
    )
    .await;
    let child2_id = create_task(
        &client,
        "E2E test - subtask 2",
        &inbox_id,
        Some(serde_json::json!({"parent_id": parent_id})),
    )
    .await;

    // Complete parent
    let close_command = SyncCommand::new("item_close", serde_json::json!({"id": parent_id}));
    let response = client
        .sync(SyncRequest::with_commands(vec![close_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_close should succeed");

    // Document the behavior (subtasks may or may not be completed)
    let parent = find_task(&client, &parent_id).await.expect("Parent should exist");
    let child1 = find_task(&client, &child1_id).await.expect("Child1 should exist");
    let child2 = find_task(&client, &child2_id).await.expect("Child2 should exist");

    println!(
        "Parent completed: {}, Child1 completed: {}, Child2 completed: {}",
        parent.checked, child1.checked, child2.checked
    );

    assert!(parent.checked, "Parent should be completed");
    // Note: Todoist behavior may or may not auto-complete subtasks

    // Clean up
    delete_task(&client, &child1_id).await;
    delete_task(&client, &child2_id).await;
    delete_task(&client, &parent_id).await;
}

#[tokio::test]
async fn test_delete_parent_cascades_to_subtasks() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create parent with 2 subtasks
    let parent_id = create_task(&client, "E2E test - parent to delete", &inbox_id, None).await;
    let child1_id = create_task(
        &client,
        "E2E test - child1 to delete",
        &inbox_id,
        Some(serde_json::json!({"parent_id": parent_id})),
    )
    .await;
    let child2_id = create_task(
        &client,
        "E2E test - child2 to delete",
        &inbox_id,
        Some(serde_json::json!({"parent_id": parent_id})),
    )
    .await;

    // Delete parent
    delete_task(&client, &parent_id).await;

    // Check subtasks (should be deleted or orphaned)
    let child1 = find_task(&client, &child1_id).await;
    let child2 = find_task(&client, &child2_id).await;

    // Document the behavior
    match (&child1, &child2) {
        (None, None) => println!("Subtasks were cascade deleted (absent from response)"),
        (Some(c1), Some(c2)) if c1.is_deleted && c2.is_deleted => {
            println!("Subtasks were cascade deleted (is_deleted: true)")
        }
        _ => println!(
            "Subtasks may be orphaned: child1={:?}, child2={:?}",
            child1.as_ref().map(|c| c.is_deleted),
            child2.as_ref().map(|c| c.is_deleted)
        ),
    }

    // Clean up any remaining tasks
    if let Some(ref c) = child1 {
        if !c.is_deleted {
            delete_task(&client, &child1_id).await;
        }
    }
    if let Some(ref c) = child2 {
        if !c.is_deleted {
            delete_task(&client, &child2_id).await;
        }
    }
}

// ============================================================================
// 1.5 Task Ordering Tests
// ============================================================================

#[tokio::test]
async fn test_reorder_tasks_in_project() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // Create project with 3 tasks
    let project_id = create_project(&client, "E2E_Test_ReorderProject").await;
    let task1_id = create_task(&client, "E2E test - task 1", &project_id, None).await;
    let task2_id = create_task(&client, "E2E test - task 2", &project_id, None).await;
    let task3_id = create_task(&client, "E2E test - task 3", &project_id, None).await;

    // Get initial order
    let task1 = find_task(&client, &task1_id).await.unwrap();
    let task2 = find_task(&client, &task2_id).await.unwrap();
    let task3 = find_task(&client, &task3_id).await.unwrap();
    println!(
        "Initial order: task1={}, task2={}, task3={}",
        task1.child_order, task2.child_order, task3.child_order
    );

    // Reorder: task3, task1, task2
    let reorder_command = SyncCommand::new(
        "item_reorder",
        serde_json::json!({
            "items": [
                {"id": task3_id, "child_order": 1},
                {"id": task1_id, "child_order": 2},
                {"id": task2_id, "child_order": 3}
            ]
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![reorder_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_reorder should succeed");

    // Verify new order
    let task1 = find_task(&client, &task1_id).await.unwrap();
    let task2 = find_task(&client, &task2_id).await.unwrap();
    let task3 = find_task(&client, &task3_id).await.unwrap();
    println!(
        "New order: task1={}, task2={}, task3={}",
        task1.child_order, task2.child_order, task3.child_order
    );

    assert!(task3.child_order < task1.child_order, "task3 should be before task1");
    assert!(task1.child_order < task2.child_order, "task1 should be before task2");

    // Clean up
    delete_task(&client, &task1_id).await;
    delete_task(&client, &task2_id).await;
    delete_task(&client, &task3_id).await;
    delete_project(&client, &project_id).await;
}

#[tokio::test]
async fn test_update_day_orders() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let inbox_id = get_inbox_id(&client).await;

    // Create 3 tasks due today
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let task1_id = create_task(
        &client,
        "E2E test - day order 1",
        &inbox_id,
        Some(serde_json::json!({"due": {"date": &today}})),
    )
    .await;
    let task2_id = create_task(
        &client,
        "E2E test - day order 2",
        &inbox_id,
        Some(serde_json::json!({"due": {"date": &today}})),
    )
    .await;
    let task3_id = create_task(
        &client,
        "E2E test - day order 3",
        &inbox_id,
        Some(serde_json::json!({"due": {"date": &today}})),
    )
    .await;

    // Update day orders
    let update_command = SyncCommand::new(
        "item_update_day_orders",
        serde_json::json!({
            "ids_to_orders": {
                task3_id.clone(): 1,
                task1_id.clone(): 2,
                task2_id.clone(): 3
            }
        }),
    );
    let response = client
        .sync(SyncRequest::with_commands(vec![update_command]))
        .await
        .unwrap();
    assert!(!response.has_errors(), "item_update_day_orders should succeed");

    // Verify day_order values
    let task1 = find_task(&client, &task1_id).await.unwrap();
    let task2 = find_task(&client, &task2_id).await.unwrap();
    let task3 = find_task(&client, &task3_id).await.unwrap();
    println!(
        "Day orders: task1={}, task2={}, task3={}",
        task1.day_order, task2.day_order, task3.day_order
    );

    // Clean up
    delete_task(&client, &task1_id).await;
    delete_task(&client, &task2_id).await;
    delete_task(&client, &task3_id).await;
}
