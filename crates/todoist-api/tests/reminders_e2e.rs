//! End-to-end tests for reminder operations.
//!
//! These tests validate reminder CRUD operations against the real Todoist API.
//!
//! Run with: cargo test --package todoist-api --features e2e --test reminders_e2e
//!
//! Tests follow the naming convention from specs/E2E_TEST_SPEC.md section 6.
//!
//! ## Rate Limit Mitigation
//!
//! These tests use `TestContext` which performs ONE full sync at initialization
//! and uses partial (incremental) syncs for all subsequent operations. This
//! dramatically reduces API calls and helps stay within Todoist's rate limits:
//! - Full sync: 100 requests / 15 minutes
//! - Partial sync: 1000 requests / 15 minutes
//!
//! ## Note on Reminders
//!
//! Reminders require Todoist Pro. If the test account does not have Pro,
//! reminder operations will fail with an error. Tests handle this gracefully
//! by checking for errors and skipping if reminders are not available.

#![cfg(feature = "e2e")]

mod test_context;

use test_context::TestContext;
use todoist_api_rs::sync::SyncCommand;

/// Helper to check if reminders are available (requires Pro).
/// Returns true if the first reminder test succeeds, false if reminders are unavailable.
async fn reminders_available(ctx: &mut TestContext, task_id: &str) -> bool {
    // Try to create a test reminder - if it fails, reminders are not available
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        "reminder_add",
        &temp_id,
        serde_json::json!({
            "item_id": task_id,
            "type": "absolute",
            "due": {
                "date": "2030-12-31T23:59:00"
            }
        }),
    );

    match ctx.execute(vec![command]).await {
        Ok(response) => {
            if response.has_errors() {
                eprintln!(
                    "Reminders not available (Pro required): {:?}",
                    response.errors()
                );
                return false;
            }
            // Clean up the test reminder
            if let Some(reminder_id) = response.real_id(&temp_id) {
                let _ = ctx.delete_reminder(reminder_id).await;
            }
            true
        }
        Err(e) => {
            eprintln!("Reminders not available (Pro required): {}", e);
            false
        }
    }
}

// ============================================================================
// 6. Reminder Operations Tests
// ============================================================================

/// Test creating an absolute reminder at a specific datetime.
///
/// Spec: 6 - test_create_absolute_reminder
/// - Create task
/// - Call `reminder_add` with `{item_id: ..., due: {date: "2025-06-15T09:00:00"}}`
/// - Sync and verify reminder exists
/// - Clean up: delete reminder and task
#[tokio::test]
async fn test_create_absolute_reminder() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task for the reminder
    let task_id = ctx
        .create_task("E2E test - absolute reminder task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Check if reminders are available (Pro required)
    if !reminders_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: reminders require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Create an absolute reminder
    let reminder_id = ctx
        .create_absolute_reminder(&task_id, "2030-06-15T09:00:00")
        .await
        .expect("create_absolute_reminder failed");

    // Verify reminder exists in cache
    let reminder = ctx
        .find_reminder(&reminder_id)
        .expect("Reminder should exist in cache");

    assert_eq!(reminder.item_id, task_id, "Reminder should be for the task");
    assert_eq!(
        reminder.reminder_type, "absolute",
        "Reminder type should be absolute"
    );
    assert!(reminder.due.is_some(), "Absolute reminder should have due");
    assert!(!reminder.is_deleted, "Reminder should not be deleted");

    // Clean up
    ctx.batch_delete_with_reminders(&[&task_id], &[], &[], &[], &[&reminder_id])
        .await
        .expect("cleanup failed");
}

/// Test creating a relative reminder (minutes before task due time).
///
/// Spec: 6 - test_create_relative_reminder
/// - Create task with due datetime
/// - Call `reminder_add` with `{item_id: ..., minute_offset: 30}` (30 min before)
/// - Verify reminder created
/// - Clean up: delete all
#[tokio::test]
async fn test_create_relative_reminder() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task WITH due time (required for relative reminders)
    let task_id = ctx
        .create_task(
            "E2E test - relative reminder task",
            &inbox_id,
            Some(serde_json::json!({
                "due": {
                    "date": "2030-07-01T14:00:00"
                }
            })),
        )
        .await
        .expect("create_task failed");

    // Check if reminders are available (Pro required)
    if !reminders_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: reminders require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Create a relative reminder (30 minutes before)
    let reminder_id = ctx
        .create_relative_reminder(&task_id, 30)
        .await
        .expect("create_relative_reminder failed");

    // Verify reminder exists in cache
    let reminder = ctx
        .find_reminder(&reminder_id)
        .expect("Reminder should exist in cache");

    assert_eq!(reminder.item_id, task_id, "Reminder should be for the task");
    assert_eq!(
        reminder.reminder_type, "relative",
        "Reminder type should be relative"
    );
    assert_eq!(
        reminder.minute_offset,
        Some(30),
        "Minute offset should be 30"
    );
    assert!(!reminder.is_deleted, "Reminder should not be deleted");

    // Clean up
    ctx.batch_delete_with_reminders(&[&task_id], &[], &[], &[], &[&reminder_id])
        .await
        .expect("cleanup failed");
}

/// Test updating an existing reminder.
///
/// Spec: 6 - test_update_reminder
/// - Create task and reminder
/// - Call `reminder_update` to change time
/// - Verify change persisted
/// - Clean up: delete all
#[tokio::test]
async fn test_update_reminder() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task for the reminder
    let task_id = ctx
        .create_task("E2E test - update reminder task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Check if reminders are available (Pro required)
    if !reminders_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: reminders require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Create an absolute reminder with initial time
    let reminder_id = ctx
        .create_absolute_reminder(&task_id, "2030-06-15T09:00:00")
        .await
        .expect("create_absolute_reminder failed");

    // Verify initial state
    let reminder = ctx
        .find_reminder(&reminder_id)
        .expect("Reminder should exist in cache");
    let initial_due = reminder.due.clone();

    // Update the reminder with a new time
    let update_command = SyncCommand::new(
        "reminder_update",
        serde_json::json!({
            "id": reminder_id,
            "due": {
                "date": "2030-08-20T15:00:00"
            }
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "reminder_update should succeed");

    // Verify the change persisted
    let updated_reminder = ctx
        .find_reminder(&reminder_id)
        .expect("Reminder should still exist");
    assert_ne!(
        updated_reminder.due, initial_due,
        "Reminder due should have changed"
    );

    // Check the new due date contains the updated date
    if let Some(due) = &updated_reminder.due {
        assert!(
            due.date.contains("2030-08-20"),
            "Due date should be updated to 2030-08-20"
        );
    }

    // Clean up
    ctx.batch_delete_with_reminders(&[&task_id], &[], &[], &[], &[&reminder_id])
        .await
        .expect("cleanup failed");
}

/// Test deleting a reminder.
///
/// Spec: 6 - test_delete_reminder
/// - Create task and reminder
/// - Call `reminder_delete`
/// - Verify reminder gone, task still exists
/// - Clean up: delete task
#[tokio::test]
async fn test_delete_reminder() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task for the reminder
    let task_id = ctx
        .create_task("E2E test - delete reminder task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Check if reminders are available (Pro required)
    if !reminders_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: reminders require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Create a reminder
    let reminder_id = ctx
        .create_absolute_reminder(&task_id, "2030-06-15T09:00:00")
        .await
        .expect("create_absolute_reminder failed");

    // Verify reminder exists
    assert!(
        ctx.find_reminder(&reminder_id).is_some(),
        "Reminder should exist before deletion"
    );

    // Delete the reminder
    ctx.delete_reminder(&reminder_id)
        .await
        .expect("delete_reminder failed");

    // Verify reminder is gone
    assert!(
        ctx.find_reminder(&reminder_id).is_none(),
        "Reminder should not be findable after deletion"
    );

    // Verify task still exists
    assert!(
        ctx.find_item(&task_id).is_some(),
        "Task should still exist after reminder deletion"
    );

    // Clean up
    ctx.delete_task(&task_id).await.expect("cleanup failed");
}

/// Test adding multiple reminders to one task.
///
/// Spec: 6 - test_multiple_reminders_on_task
/// - Create task
/// - Add 3 reminders at different times
/// - Verify all 3 exist
/// - Clean up: delete all
#[tokio::test]
async fn test_multiple_reminders_on_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task with due time (required for relative reminders)
    let task_id = ctx
        .create_task(
            "E2E test - multiple reminders task",
            &inbox_id,
            Some(serde_json::json!({
                "due": {
                    "date": "2030-07-15T10:00:00"
                }
            })),
        )
        .await
        .expect("create_task failed");

    // Check if reminders are available (Pro required)
    if !reminders_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: reminders require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Create 3 reminders with different configurations
    let reminder1_id = ctx
        .create_absolute_reminder(&task_id, "2030-07-14T09:00:00")
        .await
        .expect("create reminder 1 failed");

    let reminder2_id = ctx
        .create_absolute_reminder(&task_id, "2030-07-15T08:00:00")
        .await
        .expect("create reminder 2 failed");

    let reminder3_id = ctx
        .create_relative_reminder(&task_id, 60) // 60 minutes before
        .await
        .expect("create reminder 3 failed");

    // Verify all 3 reminders exist
    assert!(
        ctx.find_reminder(&reminder1_id).is_some(),
        "Reminder 1 should exist"
    );
    assert!(
        ctx.find_reminder(&reminder2_id).is_some(),
        "Reminder 2 should exist"
    );
    assert!(
        ctx.find_reminder(&reminder3_id).is_some(),
        "Reminder 3 should exist"
    );

    // Verify all reminders are for the same task
    let reminders_for_task = ctx.find_reminders_for_task(&task_id);
    assert!(
        reminders_for_task.len() >= 3,
        "Task should have at least 3 reminders, found {}",
        reminders_for_task.len()
    );

    // Verify different types exist
    let has_absolute = reminders_for_task
        .iter()
        .any(|r| r.reminder_type == "absolute");
    let has_relative = reminders_for_task
        .iter()
        .any(|r| r.reminder_type == "relative");
    assert!(has_absolute, "Should have at least one absolute reminder");
    assert!(has_relative, "Should have at least one relative reminder");

    // Clean up
    ctx.batch_delete_with_reminders(
        &[&task_id],
        &[],
        &[],
        &[],
        &[&reminder1_id, &reminder2_id, &reminder3_id],
    )
    .await
    .expect("cleanup failed");
}

/// Test reminder behavior with recurring task.
///
/// Spec: 6 - test_reminder_on_recurring_task
/// - Create recurring task
/// - Add reminder
/// - Complete task (advances recurrence)
/// - Verify reminder behavior (reset or persisted)
/// - Clean up: delete all
#[tokio::test]
async fn test_reminder_on_recurring_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a recurring task with due time
    let task_id = ctx
        .create_task(
            "E2E test - recurring task with reminder",
            &inbox_id,
            Some(serde_json::json!({
                "due": {
                    "string": "every day at 10am",
                    "lang": "en"
                }
            })),
        )
        .await
        .expect("create_task failed");

    // Verify task is recurring
    let task = ctx.find_item(&task_id).expect("Task should exist");
    assert!(
        task.due.as_ref().map(|d| d.is_recurring).unwrap_or(false),
        "Task should be recurring"
    );

    // Check if reminders are available (Pro required)
    if !reminders_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: reminders require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Add a relative reminder (30 min before)
    let reminder_id = ctx
        .create_relative_reminder(&task_id, 30)
        .await
        .expect("create_relative_reminder failed");

    // Verify reminder exists
    assert!(
        ctx.find_reminder(&reminder_id).is_some(),
        "Reminder should exist"
    );

    // Complete the recurring task (this advances to next occurrence)
    let complete_command = SyncCommand::new("item_close", serde_json::json!({"id": task_id}));
    let response = ctx.execute(vec![complete_command]).await.unwrap();
    assert!(!response.has_errors(), "item_close should succeed");

    // After completing a recurring task, the task should still exist with next due date
    // and the reminder should be preserved (Todoist keeps reminders on recurring tasks)
    let task_after = ctx.find_item(&task_id).expect("Task should still exist");
    assert!(
        !task_after.checked,
        "Recurring task should be unchecked after advancing"
    );

    // The reminder should still exist for the task
    // Note: Todoist's behavior may vary, so we just document what happens
    let reminder_still_exists = ctx.find_reminder(&reminder_id).is_some();
    let reminders_for_task = ctx.find_reminders_for_task(&task_id);

    // Document the behavior - either the original reminder persists or a new one is created
    if reminder_still_exists {
        eprintln!("Note: Original reminder persisted after completing recurring task");
    } else if !reminders_for_task.is_empty() {
        eprintln!(
            "Note: Reminder was recreated after completing recurring task ({} reminders)",
            reminders_for_task.len()
        );
    } else {
        eprintln!(
            "Note: No reminders after completing recurring task - behavior may vary by plan"
        );
    }

    // Clean up - collect any reminder IDs that exist
    let reminder_ids_to_delete: Vec<String> = ctx
        .find_reminders_for_task(&task_id)
        .iter()
        .map(|r| r.id.clone())
        .collect();

    let reminder_refs: Vec<&str> = reminder_ids_to_delete.iter().map(String::as_str).collect();
    ctx.batch_delete_with_reminders(&[&task_id], &[], &[], &[], &reminder_refs)
        .await
        .expect("cleanup failed");
}

/// Test that reminders sync correctly to cache.
///
/// Spec: 6 - test_reminder_appears_in_cache
/// - Create task and reminder via API
/// - Sync cache
/// - Verify reminder in cache
/// - Clean up: delete all
#[tokio::test]
async fn test_reminder_appears_in_cache() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task for the reminder
    let task_id = ctx
        .create_task("E2E test - reminder in cache task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Check if reminders are available (Pro required)
    if !reminders_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: reminders require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Create a reminder - the execute() method already updates the cache
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        "reminder_add",
        &temp_id,
        serde_json::json!({
            "item_id": task_id,
            "type": "absolute",
            "due": {
                "date": "2030-12-25T10:00:00"
            }
        }),
    );

    let response = ctx.execute(vec![command]).await.unwrap();
    assert!(!response.has_errors(), "reminder_add should succeed");

    let reminder_id = response.real_id(&temp_id).unwrap().clone();

    // Verify reminder is immediately in cache (no need to refresh - execute() updates cache)
    let reminder = ctx
        .find_reminder(&reminder_id)
        .expect("Reminder should be in cache immediately after creation");

    assert_eq!(reminder.item_id, task_id);
    assert_eq!(reminder.reminder_type, "absolute");
    assert!(!reminder.is_deleted);

    // Do a refresh to verify it persists across syncs
    ctx.refresh().await.expect("refresh failed");

    let reminder_after_refresh = ctx
        .find_reminder(&reminder_id)
        .expect("Reminder should still be in cache after refresh");
    assert_eq!(reminder_after_refresh.item_id, task_id);

    // Clean up
    ctx.batch_delete_with_reminders(&[&task_id], &[], &[], &[], &[&reminder_id])
        .await
        .expect("cleanup failed");
}
