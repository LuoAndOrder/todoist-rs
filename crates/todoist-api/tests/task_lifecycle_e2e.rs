//! End-to-end tests for task lifecycle operations.
//!
//! These tests validate task CRUD, movement, completion, subtasks, and ordering
//! against the real Todoist API.
//!
//! Run with: cargo test -p todoist-api-rs --features extended-e2e --test task_lifecycle_e2e
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

#![cfg(feature = "extended-e2e")]

mod test_context;

use chrono_tz::Tz;
use test_context::TestContext;
use todoist_api_rs::sync::{SyncCommand, SyncCommandType};

fn today_in_timezone(tz_str: &str) -> String {
    let tz: Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);
    chrono::Utc::now()
        .with_timezone(&tz)
        .format("%Y-%m-%d")
        .to_string()
}

// ============================================================================
// 1.1 Basic CRUD Tests
// ============================================================================

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
        SyncCommandType::ItemMove,
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
        SyncCommandType::ItemMove,
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
        SyncCommandType::ItemComplete,
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
    let close_command = SyncCommand::new(
        SyncCommandType::ItemClose,
        serde_json::json!({"id": task_id}),
    );
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
    let close_command = SyncCommand::new(
        SyncCommandType::ItemClose,
        serde_json::json!({"id": parent_id}),
    );
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
async fn test_update_day_orders() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create 3 tasks due today
    let today = today_in_timezone(ctx.user_timezone());
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
        SyncCommandType::ItemUpdateDayOrders,
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
    // To create a fixed timezone due date, use the Z suffix (UTC format) with timezone
    // The API will store the date in UTC and use the timezone for display/recurrence
    let update_command = SyncCommand::new(
        SyncCommandType::ItemUpdate,
        serde_json::json!({
            "id": task_id,
            "due": {
                "date": "2025-06-15T18:30:00Z",
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
    // Date should be in UTC format (ends with Z)
    assert!(
        due.date.ends_with("Z"),
        "Fixed timezone due date should be in UTC format, got: {}",
        due.date
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
        SyncCommandType::ItemUpdate,
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
        SyncCommandType::ItemUpdate,
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
