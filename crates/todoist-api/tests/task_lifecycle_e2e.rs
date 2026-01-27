//! End-to-end tests for task lifecycle operations.
//!
//! These tests validate task CRUD, movement, completion, subtasks, and ordering
//! against the real Todoist API.
//!
//! Run with: cargo test --package todoist-api --features e2e --test task_lifecycle_e2e
//!
//! Tests follow the naming convention from specs/E2E_TEST_SPEC.md sections 1.1-1.5.
//!
//! ## Rate Limit Mitigation
//!
//! These tests use `TestContext` which performs ONE full sync at initialization
//! and uses partial (incremental) syncs for all subsequent operations. This
//! dramatically reduces API calls and helps stay within Todoist's rate limits:
//! - Full sync: 100 requests / 15 minutes
//! - Partial sync: 1000 requests / 15 minutes

#![cfg(feature = "e2e")]

mod test_context;

use test_context::TestContext;
use todoist_api_rs::sync::SyncCommand;

// ============================================================================
// 1.1 Basic CRUD Tests
// ============================================================================

#[tokio::test]
async fn test_create_task_minimal() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task with only content
    let task_id = ctx
        .create_task("E2E test - minimal task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Verify task appears in cached state (no API call)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");

    assert_eq!(task.content, "E2E test - minimal task");
    assert_eq!(task.priority, 1, "Default priority should be 1");
    assert!(task.due.is_none(), "Should have no due date");
    assert!(task.labels.is_empty(), "Should have no labels");
    assert!(!task.checked, "Should not be completed");
    assert!(!task.is_deleted, "Should not be deleted");

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_create_task_with_all_fields() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task with all fields
    let task_id = ctx
        .create_task(
            "E2E test - complete task",
            &inbox_id,
            Some(serde_json::json!({
                "description": "Detailed description here",
                "priority": 4,
                "due": {"date": "2025-12-25"},
                "labels": ["e2e-test-label"]
            })),
        )
        .await
        .expect("create_task failed");

    // Verify all fields persisted (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");

    assert_eq!(task.content, "E2E test - complete task");
    assert_eq!(task.description, "Detailed description here");
    assert_eq!(task.priority, 4, "Priority should be 4 (p1)");
    assert!(task.due.is_some(), "Should have due date");
    assert_eq!(task.due.as_ref().unwrap().date, "2025-12-25");
    assert!(task.labels.contains(&"e2e-test-label".to_string()));

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_update_task_content() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task
    let task_id = ctx
        .create_task("E2E test - original content", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Update content
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "content": "E2E test - modified content"
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify content changed (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.content, "E2E test - modified content");

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_update_task_description() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task with no description
    let task_id = ctx
        .create_task("E2E test - description test", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Add description
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "description": "New description"
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors());

    // Verify description added (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.description, "New description");

    // Update description
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "description": "Changed description"
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors());

    // Verify description changed (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.description, "Changed description");

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_delete_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task
    let task_id = ctx
        .create_task("E2E test - to be deleted", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Verify task exists in cache
    assert!(
        ctx.find_item(&task_id).is_some(),
        "Task should exist before deletion"
    );

    // Delete task
    ctx.delete_task(&task_id).await.expect("delete_task failed");

    // Verify task is deleted from cache (find_item filters out is_deleted)
    assert!(
        ctx.find_item(&task_id).is_none(),
        "Task should not be findable after deletion"
    );
}

// ============================================================================
// 1.2 Task Movement Tests
// ============================================================================

#[tokio::test]
async fn test_move_task_between_projects() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create Project A and Project B
    let project_a_id = ctx
        .create_project("E2E_Test_ProjectA")
        .await
        .expect("create_project failed");
    let project_b_id = ctx
        .create_project("E2E_Test_ProjectB")
        .await
        .expect("create_project failed");

    // Create task in Project A
    let task_id = ctx
        .create_task("E2E test - moveable task", &project_a_id, None)
        .await
        .expect("create_task failed");

    // Verify task is in Project A
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.project_id, project_a_id);

    // Move task to Project B
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": task_id,
            "project_id": project_b_id
        }),
    );
    let response = ctx.execute(vec![move_command]).await.unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify task is now in Project B (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.project_id, project_b_id);

    // Clean up
    ctx.batch_delete(&[&task_id], &[&project_a_id, &project_b_id], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_move_task_to_section() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project with section
    let project_id = ctx
        .create_project("E2E_Test_SectionProject")
        .await
        .expect("create_project failed");
    let section_id = ctx
        .create_section("In Progress", &project_id)
        .await
        .expect("create_section failed");

    // Create task in project (no section)
    let task_id = ctx
        .create_task("E2E test - section move", &project_id, None)
        .await
        .expect("create_task failed");

    // Verify task has no section
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(
        task.section_id.is_none(),
        "Task should have no section initially"
    );

    // Move task to section
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": task_id,
            "section_id": section_id
        }),
    );
    let response = ctx.execute(vec![move_command]).await.unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify task is in section (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.section_id, Some(section_id.clone()));

    // Clean up
    ctx.batch_delete(&[&task_id], &[&project_id], &[&section_id], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_move_task_out_of_section() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project with section
    let project_id = ctx
        .create_project("E2E_Test_SectionOutProject")
        .await
        .expect("create_project failed");
    let section_id = ctx
        .create_section("Backlog", &project_id)
        .await
        .expect("create_section failed");

    // Create task in section
    let task_id = ctx
        .create_task(
            "E2E test - section move out",
            &project_id,
            Some(serde_json::json!({"section_id": section_id})),
        )
        .await
        .expect("create_task failed");

    // Verify task is in section
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.section_id, Some(section_id.clone()));

    // Move task out of section (to project root)
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": task_id,
            "project_id": project_id
        }),
    );
    let response = ctx.execute(vec![move_command]).await.unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify task has no section (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(
        task.section_id.is_none(),
        "Task should have no section after move"
    );

    // Clean up
    ctx.batch_delete(&[&task_id], &[&project_id], &[&section_id], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_move_task_to_section_in_different_project() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create Project A and Project B with section
    let project_a_id = ctx
        .create_project("E2E_Test_CrossProjectA")
        .await
        .expect("create_project failed");
    let project_b_id = ctx
        .create_project("E2E_Test_CrossProjectB")
        .await
        .expect("create_project failed");
    let section_b_id = ctx
        .create_section("Done", &project_b_id)
        .await
        .expect("create_section failed");

    // Create task in Project A
    let task_id = ctx
        .create_task("E2E test - cross project move", &project_a_id, None)
        .await
        .expect("create_task failed");

    // Verify task is in Project A
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.project_id, project_a_id);

    // Move task to section in Project B
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": task_id,
            "section_id": section_b_id
        }),
    );
    let response = ctx.execute(vec![move_command]).await.unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify task is in Project B's section (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.project_id, project_b_id);
    assert_eq!(task.section_id, Some(section_b_id.clone()));

    // Clean up
    ctx.batch_delete(
        &[&task_id],
        &[&project_a_id, &project_b_id],
        &[&section_b_id],
        &[],
    )
    .await
    .expect("cleanup failed");
}

// ============================================================================
// 1.3 Task Completion Tests
// ============================================================================

#[tokio::test]
async fn test_complete_task_with_item_close() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task
    let task_id = ctx
        .create_task("E2E test - to be closed", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Verify task is not completed
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(!task.checked, "Task should not be completed initially");

    // Complete task with item_close
    let close_command = SyncCommand::new("item_close", serde_json::json!({"id": task_id}));
    let response = ctx.execute(vec![close_command]).await.unwrap();
    assert!(!response.has_errors(), "item_close should succeed");

    // Verify task is completed (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.checked, "Task should be completed");

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_complete_task_with_item_complete() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task
    let task_id = ctx
        .create_task("E2E test - to be completed", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Complete task with item_complete and completed_at timestamp
    let complete_command = SyncCommand::new(
        "item_complete",
        serde_json::json!({
            "id": task_id,
            "completed_at": "2025-01-26T12:00:00Z"
        }),
    );
    let response = ctx.execute(vec![complete_command]).await.unwrap();
    assert!(!response.has_errors(), "item_complete should succeed");

    // Verify task is completed (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.checked, "Task should be completed");

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_uncomplete_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create and complete task
    let task_id = ctx
        .create_task("E2E test - to be uncompleted", &inbox_id, None)
        .await
        .expect("create_task failed");

    let close_command = SyncCommand::new("item_close", serde_json::json!({"id": task_id}));
    let response = ctx.execute(vec![close_command]).await.unwrap();
    assert!(!response.has_errors());

    // Verify task is completed (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.checked, "Task should be completed");

    // Uncomplete task
    let uncomplete_command =
        SyncCommand::new("item_uncomplete", serde_json::json!({"id": task_id}));
    let response = ctx.execute(vec![uncomplete_command]).await.unwrap();
    assert!(!response.has_errors(), "item_uncomplete should succeed");

    // Verify task is uncompleted (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(!task.checked, "Task should be uncompleted");

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_complete_recurring_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create recurring task
    let task_id = ctx
        .create_task(
            "E2E test - recurring task",
            &inbox_id,
            Some(serde_json::json!({
                "due": {"string": "every day"}
            })),
        )
        .await
        .expect("create_task failed");

    // Verify task is recurring (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.due.is_some(), "Task should have due date");
    let original_due = task.due.as_ref().unwrap().date.clone();
    assert!(
        task.due.as_ref().unwrap().is_recurring,
        "Task should be recurring"
    );

    // Complete recurring task
    let close_command = SyncCommand::new("item_close", serde_json::json!({"id": task_id}));
    let response = ctx.execute(vec![close_command]).await.unwrap();
    assert!(!response.has_errors(), "item_close should succeed");

    // Verify task still exists, due date advanced, not checked (from cache)
    let task = ctx
        .find_item(&task_id)
        .expect("Recurring task should still exist in cache");
    assert!(
        !task.checked,
        "Recurring task should not be checked after completion"
    );
    assert!(task.due.is_some(), "Task should still have due date");
    let new_due = task.due.as_ref().unwrap().date.clone();
    assert_ne!(
        original_due, new_due,
        "Due date should have advanced to next occurrence"
    );

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

// ============================================================================
// 1.4 Subtask (Parent-Child) Tests
// ============================================================================

#[tokio::test]
async fn test_create_subtask() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create parent task
    let parent_id = ctx
        .create_task("E2E test - parent task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Create child task
    let child_id = ctx
        .create_task(
            "E2E test - child task",
            &inbox_id,
            Some(serde_json::json!({"parent_id": parent_id})),
        )
        .await
        .expect("create_task failed");

    // Verify child's parent_id (from cache)
    let child = ctx
        .find_item(&child_id)
        .expect("Child should exist in cache");
    assert_eq!(child.parent_id, Some(parent_id.clone()));

    // Clean up (delete child first, then parent)
    ctx.batch_delete(&[&child_id, &parent_id], &[], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_create_nested_subtasks() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create hierarchy: A -> B -> C
    let task_a_id = ctx
        .create_task("E2E test - task A (root)", &inbox_id, None)
        .await
        .expect("create_task failed");
    let task_b_id = ctx
        .create_task(
            "E2E test - task B (child of A)",
            &inbox_id,
            Some(serde_json::json!({"parent_id": task_a_id})),
        )
        .await
        .expect("create_task failed");
    let task_c_id = ctx
        .create_task(
            "E2E test - task C (child of B)",
            &inbox_id,
            Some(serde_json::json!({"parent_id": task_b_id})),
        )
        .await
        .expect("create_task failed");

    // Verify hierarchy (from cache)
    let task_a = ctx
        .find_item(&task_a_id)
        .expect("Task A should exist in cache");
    let task_b = ctx
        .find_item(&task_b_id)
        .expect("Task B should exist in cache");
    let task_c = ctx
        .find_item(&task_c_id)
        .expect("Task C should exist in cache");

    assert!(task_a.parent_id.is_none(), "A should have no parent");
    assert_eq!(
        task_b.parent_id,
        Some(task_a_id.clone()),
        "B's parent should be A"
    );
    assert_eq!(
        task_c.parent_id,
        Some(task_b_id.clone()),
        "C's parent should be B"
    );

    // Clean up (delete in reverse order)
    ctx.batch_delete(&[&task_c_id, &task_b_id, &task_a_id], &[], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_move_subtask_to_different_parent() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create parent A, parent B, and child C under A
    let parent_a_id = ctx
        .create_task("E2E test - parent A", &inbox_id, None)
        .await
        .expect("create_task failed");
    let parent_b_id = ctx
        .create_task("E2E test - parent B", &inbox_id, None)
        .await
        .expect("create_task failed");
    let child_c_id = ctx
        .create_task(
            "E2E test - child C",
            &inbox_id,
            Some(serde_json::json!({"parent_id": parent_a_id})),
        )
        .await
        .expect("create_task failed");

    // Verify C's parent is A (from cache)
    let child_c = ctx
        .find_item(&child_c_id)
        .expect("Child should exist in cache");
    assert_eq!(child_c.parent_id, Some(parent_a_id.clone()));

    // Move C to be a child of B
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": child_c_id,
            "parent_id": parent_b_id
        }),
    );
    let response = ctx.execute(vec![move_command]).await.unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify C's parent is now B (from cache)
    let child_c = ctx
        .find_item(&child_c_id)
        .expect("Child should exist in cache");
    assert_eq!(child_c.parent_id, Some(parent_b_id.clone()));

    // Clean up
    ctx.batch_delete(&[&child_c_id, &parent_a_id, &parent_b_id], &[], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_promote_subtask_to_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create parent and child
    let parent_id = ctx
        .create_task("E2E test - parent to promote from", &inbox_id, None)
        .await
        .expect("create_task failed");
    let child_id = ctx
        .create_task(
            "E2E test - child to be promoted",
            &inbox_id,
            Some(serde_json::json!({"parent_id": parent_id})),
        )
        .await
        .expect("create_task failed");

    // Verify child has parent (from cache)
    let child = ctx
        .find_item(&child_id)
        .expect("Child should exist in cache");
    assert_eq!(child.parent_id, Some(parent_id.clone()));

    // Promote child to top-level task (move to project root)
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": child_id,
            "project_id": inbox_id
        }),
    );
    let response = ctx.execute(vec![move_command]).await.unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify child has no parent (from cache)
    let child = ctx
        .find_item(&child_id)
        .expect("Child should exist in cache");
    assert!(
        child.parent_id.is_none(),
        "Child should have no parent after promotion"
    );

    // Clean up
    ctx.batch_delete(&[&child_id, &parent_id], &[], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_complete_parent_with_subtasks() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create parent with 2 subtasks
    let parent_id = ctx
        .create_task("E2E test - parent with subtasks", &inbox_id, None)
        .await
        .expect("create_task failed");
    let child1_id = ctx
        .create_task(
            "E2E test - subtask 1",
            &inbox_id,
            Some(serde_json::json!({"parent_id": parent_id})),
        )
        .await
        .expect("create_task failed");
    let child2_id = ctx
        .create_task(
            "E2E test - subtask 2",
            &inbox_id,
            Some(serde_json::json!({"parent_id": parent_id})),
        )
        .await
        .expect("create_task failed");

    // Complete parent
    let close_command = SyncCommand::new("item_close", serde_json::json!({"id": parent_id}));
    let response = ctx.execute(vec![close_command]).await.unwrap();
    assert!(!response.has_errors(), "item_close should succeed");

    // Document the behavior (from cache)
    let parent = ctx
        .find_item(&parent_id)
        .expect("Parent should exist in cache");
    let child1 = ctx
        .find_item(&child1_id)
        .expect("Child1 should exist in cache");
    let child2 = ctx
        .find_item(&child2_id)
        .expect("Child2 should exist in cache");

    println!(
        "Parent completed: {}, Child1 completed: {}, Child2 completed: {}",
        parent.checked, child1.checked, child2.checked
    );

    assert!(parent.checked, "Parent should be completed");
    // Note: Todoist behavior may or may not auto-complete subtasks

    // Clean up
    ctx.batch_delete(&[&child1_id, &child2_id, &parent_id], &[], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_delete_parent_cascades_to_subtasks() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create parent with 2 subtasks
    let parent_id = ctx
        .create_task("E2E test - parent to delete", &inbox_id, None)
        .await
        .expect("create_task failed");
    let child1_id = ctx
        .create_task(
            "E2E test - child1 to delete",
            &inbox_id,
            Some(serde_json::json!({"parent_id": parent_id})),
        )
        .await
        .expect("create_task failed");
    let child2_id = ctx
        .create_task(
            "E2E test - child2 to delete",
            &inbox_id,
            Some(serde_json::json!({"parent_id": parent_id})),
        )
        .await
        .expect("create_task failed");

    // Delete parent
    ctx.delete_task(&parent_id)
        .await
        .expect("delete_task failed");

    // Check subtasks (should be deleted or orphaned) - from cache
    let child1 = ctx.find_item(&child1_id);
    let child2 = ctx.find_item(&child2_id);

    // Document the behavior
    match (&child1, &child2) {
        (None, None) => println!("Subtasks were cascade deleted (filtered out by find_item)"),
        _ => println!(
            "Subtasks status: child1={:?}, child2={:?}",
            child1.is_some(),
            child2.is_some()
        ),
    }

    // Clean up any remaining tasks (batch_delete handles errors gracefully)
    let mut tasks_to_delete = Vec::new();
    if child1.is_some() {
        tasks_to_delete.push(child1_id.as_str());
    }
    if child2.is_some() {
        tasks_to_delete.push(child2_id.as_str());
    }
    if !tasks_to_delete.is_empty() {
        ctx.batch_delete(&tasks_to_delete, &[], &[], &[])
            .await
            .expect("cleanup failed");
    }
}

// ============================================================================
// 1.5 Task Ordering Tests
// ============================================================================

#[tokio::test]
async fn test_reorder_tasks_in_project() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project with 3 tasks
    let project_id = ctx
        .create_project("E2E_Test_ReorderProject")
        .await
        .expect("create_project failed");
    let task1_id = ctx
        .create_task("E2E test - task 1", &project_id, None)
        .await
        .expect("create_task failed");
    let task2_id = ctx
        .create_task("E2E test - task 2", &project_id, None)
        .await
        .expect("create_task failed");
    let task3_id = ctx
        .create_task("E2E test - task 3", &project_id, None)
        .await
        .expect("create_task failed");

    // Get initial order (from cache)
    let task1 = ctx.find_item(&task1_id).unwrap();
    let task2 = ctx.find_item(&task2_id).unwrap();
    let task3 = ctx.find_item(&task3_id).unwrap();
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
    let response = ctx.execute(vec![reorder_command]).await.unwrap();
    assert!(!response.has_errors(), "item_reorder should succeed");

    // Verify new order (from cache)
    let task1 = ctx.find_item(&task1_id).unwrap();
    let task2 = ctx.find_item(&task2_id).unwrap();
    let task3 = ctx.find_item(&task3_id).unwrap();
    println!(
        "New order: task1={}, task2={}, task3={}",
        task1.child_order, task2.child_order, task3.child_order
    );

    assert!(
        task3.child_order < task1.child_order,
        "task3 should be before task1"
    );
    assert!(
        task1.child_order < task2.child_order,
        "task1 should be before task2"
    );

    // Clean up
    ctx.batch_delete(&[&task1_id, &task2_id, &task3_id], &[&project_id], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_update_day_orders() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create 3 tasks due today
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let task1_id = ctx
        .create_task(
            "E2E test - day order 1",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": &today}})),
        )
        .await
        .expect("create_task failed");
    let task2_id = ctx
        .create_task(
            "E2E test - day order 2",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": &today}})),
        )
        .await
        .expect("create_task failed");
    let task3_id = ctx
        .create_task(
            "E2E test - day order 3",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": &today}})),
        )
        .await
        .expect("create_task failed");

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
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(
        !response.has_errors(),
        "item_update_day_orders should succeed"
    );

    // Verify day_order values (from cache)
    let task1 = ctx.find_item(&task1_id).unwrap();
    let task2 = ctx.find_item(&task2_id).unwrap();
    let task3 = ctx.find_item(&task3_id).unwrap();
    println!(
        "Day orders: task1={}, task2={}, task3={}",
        task1.day_order, task2.day_order, task3.day_order
    );

    // Clean up
    ctx.batch_delete(&[&task1_id, &task2_id, &task3_id], &[], &[], &[])
        .await
        .expect("cleanup failed");
}

// ============================================================================
// 2. Due Dates and Scheduling Tests
// ============================================================================

#[tokio::test]
async fn test_set_due_date_simple() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task without due date
    let task_id = ctx
        .create_task("E2E test - simple due date", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Verify no due date initially (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.due.is_none(), "Task should have no due date initially");

    // Update with simple due date (no time)
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "due": {"date": "2025-06-15"}
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify due date set correctly (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.due.is_some(), "Task should have due date");
    let due = task.due.as_ref().unwrap();
    assert_eq!(due.date, "2025-06-15", "Due date should be 2025-06-15");
    assert!(due.datetime.is_none(), "Should have no time component");
    assert!(!due.is_recurring, "Should not be recurring");

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_set_due_date_with_time() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task
    let task_id = ctx
        .create_task("E2E test - due date with time", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Update with due date including time
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "due": {"date": "2025-06-15T14:30:00"}
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify due date includes time (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.due.is_some(), "Task should have due date");
    let due = task.due.as_ref().unwrap();
    assert!(due.datetime.is_some(), "Should have datetime component");
    let datetime = due.datetime.as_ref().unwrap();
    assert!(
        datetime.contains("14:30"),
        "Datetime should include the specified time"
    );

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_set_due_date_with_timezone() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task
    let task_id = ctx
        .create_task("E2E test - due date with timezone", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Update with due date including timezone
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "due": {
                "date": "2025-06-15T14:30:00",
                "timezone": "America/New_York"
            }
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify timezone persisted (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.due.is_some(), "Task should have due date");
    let due = task.due.as_ref().unwrap();
    assert!(due.timezone.is_some(), "Should have timezone");
    assert_eq!(
        due.timezone.as_ref().unwrap(),
        "America/New_York",
        "Timezone should be America/New_York"
    );

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_set_recurring_due_date() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task
    let task_id = ctx
        .create_task("E2E test - recurring due date", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Update with recurring due date using natural language
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "due": {"string": "every monday at 9am"}
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify recurring flag and string (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.due.is_some(), "Task should have due date");
    let due = task.due.as_ref().unwrap();
    assert!(due.is_recurring, "Task should be recurring");
    assert!(due.string.is_some(), "Should have due string");
    let due_string = due.string.as_ref().unwrap().to_lowercase();
    assert!(
        due_string.contains("monday") || due_string.contains("mon"),
        "Due string should contain recurrence info: {}",
        due_string
    );

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_remove_due_date() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task with due date
    let task_id = ctx
        .create_task(
            "E2E test - remove due date",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": "2025-06-15"}})),
        )
        .await
        .expect("create_task failed");

    // Verify due date exists (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.due.is_some(), "Task should have due date initially");

    // Remove due date by setting to null
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "due": serde_json::Value::Null
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify due date is removed (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(
        task.due.is_none(),
        "Task should have no due date after removal"
    );

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_set_deadline() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task with due date
    let task_id = ctx
        .create_task(
            "E2E test - deadline",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": "2025-06-10"}})),
        )
        .await
        .expect("create_task failed");

    // Set deadline (distinct from due date)
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "deadline": {"date": "2025-06-15"}
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify both due date and deadline coexist (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.due.is_some(), "Task should still have due date");
    assert_eq!(
        task.due.as_ref().unwrap().date,
        "2025-06-10",
        "Due date should be unchanged"
    );
    assert!(task.deadline.is_some(), "Task should have deadline");
    assert_eq!(
        task.deadline.as_ref().unwrap().date,
        "2025-06-15",
        "Deadline should be 2025-06-15"
    );

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_overdue_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task with due date in the past
    let past_date = "2020-01-01";
    let task_id = ctx
        .create_task(
            "E2E test - overdue task",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": past_date}})),
        )
        .await
        .expect("create_task failed");

    // Verify task exists with past due date (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.due.is_some(), "Task should have due date");
    assert_eq!(
        task.due.as_ref().unwrap().date,
        past_date,
        "Due date should be in the past"
    );
    assert!(!task.checked, "Task should not be completed");

    // Verify task is considered overdue (checking date is before today)
    let due_date = chrono::NaiveDate::parse_from_str(&task.due.as_ref().unwrap().date, "%Y-%m-%d")
        .expect("Should parse due date");
    let today = chrono::Local::now().date_naive();
    assert!(
        due_date < today,
        "Due date should be before today (overdue)"
    );

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

#[tokio::test]
async fn test_due_date_preserved_on_move() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create two projects
    let project_a_id = ctx
        .create_project("E2E_Test_DueDateMoveA")
        .await
        .expect("create_project failed");
    let project_b_id = ctx
        .create_project("E2E_Test_DueDateMoveB")
        .await
        .expect("create_project failed");

    // Create task with due date in Project A
    let due_date = "2025-07-20";
    let task_id = ctx
        .create_task(
            "E2E test - due date preserved",
            &project_a_id,
            Some(serde_json::json!({"due": {"date": due_date}})),
        )
        .await
        .expect("create_task failed");

    // Verify task is in Project A with due date (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.project_id, project_a_id);
    assert!(task.due.is_some(), "Task should have due date");
    assert_eq!(task.due.as_ref().unwrap().date, due_date);

    // Move task to Project B
    let move_command = SyncCommand::new(
        "item_move",
        serde_json::json!({
            "id": task_id,
            "project_id": project_b_id
        }),
    );
    let response = ctx.execute(vec![move_command]).await.unwrap();
    assert!(!response.has_errors(), "item_move should succeed");

    // Verify task is now in Project B but due date is unchanged (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.project_id, project_b_id, "Task should be in Project B");
    assert!(task.due.is_some(), "Task should still have due date");
    assert_eq!(
        task.due.as_ref().unwrap().date,
        due_date,
        "Due date should be preserved after move"
    );

    // Clean up
    ctx.batch_delete(&[&task_id], &[&project_a_id, &project_b_id], &[], &[])
        .await
        .expect("cleanup failed");
}
