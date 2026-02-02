//! End-to-end tests for label operations.
//!
//! These tests validate label CRUD and task labeling operations
//! against the real Todoist API.
//!
//! Run with: cargo test --package todoist-api --features e2e --test labels_e2e
//!
//! Tests follow the naming convention from specs/E2E_TEST_SPEC.md section 5.
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
use todoist_api_rs::sync::{SyncCommand, SyncCommandType};

// ============================================================================
// 5.1 Label CRUD Tests
// ============================================================================

/// Test creating a personal label.
///
/// Spec: 5.1 - test_create_label
/// - Call `label_add` with `{name: "test-label"}`
/// - Sync and verify label exists
/// - Clean up: delete label
#[tokio::test]
async fn test_create_label() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Use unique label name to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let label_name = format!("e2e-test-create-label-{}", uuid);

    // Create a label
    let label_id = ctx
        .create_label(&label_name)
        .await
        .expect("create_label failed");

    // Verify label exists in cache
    let label = ctx
        .find_label(&label_id)
        .expect("Label should exist in cache");

    assert_eq!(label.name, label_name);
    assert!(!label.is_deleted, "Label should not be deleted");

    // Clean up
    ctx.delete_label(&label_id)
        .await
        .expect("delete_label failed");
}

/// Test creating a label with a specific color.
///
/// Spec: 5.1 - test_create_label_with_color
/// - Call `label_add` with `{name: "colored-label", color: "green"}`
/// - Verify color persisted
/// - Clean up: delete label
#[tokio::test]
async fn test_create_label_with_color() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Use unique label name to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let label_name = format!("e2e-test-colored-label-{}", uuid);

    // Create label with specific color
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        SyncCommandType::LabelAdd,
        &temp_id,
        serde_json::json!({
            "name": label_name,
            "color": "green"
        }),
    );
    let response = ctx.execute(vec![command]).await.unwrap();
    assert!(!response.has_errors(), "label_add should succeed");

    let label_id = response.real_id(&temp_id).unwrap().clone();

    // Verify color persisted (from cache)
    let label = ctx
        .find_label(&label_id)
        .expect("Label should exist in cache");
    assert_eq!(label.name, label_name);
    assert_eq!(
        label.color,
        Some("green".to_string()),
        "Color should be green"
    );

    // Clean up
    ctx.delete_label(&label_id)
        .await
        .expect("delete_label failed");
}

/// Test renaming a label.
///
/// Spec: 5.1 - test_rename_label
/// - Create label "old-name"
/// - Call `label_update` with new name "new-name"
/// - Verify name changed
/// - Clean up: delete label
#[tokio::test]
async fn test_rename_label() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Use unique label names to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let old_name = format!("e2e-test-old-name-{}", uuid);
    let new_name = format!("e2e-test-new-name-{}", uuid);

    // Create a label with initial name
    let label_id = ctx
        .create_label(&old_name)
        .await
        .expect("create_label failed");

    // Verify initial name (from cache)
    let label = ctx
        .find_label(&label_id)
        .expect("Label should exist in cache");
    assert_eq!(label.name, old_name);

    // Rename the label
    let update_command = SyncCommand::new(
        SyncCommandType::LabelUpdate,
        serde_json::json!({
            "id": label_id,
            "name": new_name
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "label_update should succeed");

    // Verify name changed (from cache)
    let label = ctx
        .find_label(&label_id)
        .expect("Label should exist in cache");
    assert_eq!(label.name, new_name);

    // Clean up
    ctx.delete_label(&label_id)
        .await
        .expect("delete_label failed");
}

/// Test deleting a label and verifying task label removal.
///
/// Spec: 5.1 - test_delete_label
/// - Create label
/// - Add label to a task
/// - Delete label
/// - Verify label gone
/// - Verify task no longer has label
/// - Clean up: delete task
#[tokio::test]
async fn test_delete_label() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Use unique label name to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let label_name = format!("e2e-test-delete-label-{}", uuid);

    // Create a label
    let label_id = ctx
        .create_label(&label_name)
        .await
        .expect("create_label failed");

    // Create a task with this label
    let task_id = ctx
        .create_task(
            "E2E test - task with label to delete",
            &inbox_id,
            Some(serde_json::json!({"labels": [label_name]})),
        )
        .await
        .expect("create_task failed");

    // Verify task has the label (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(
        task.labels.contains(&label_name),
        "Task should have the label"
    );

    // Delete the label
    ctx.delete_label(&label_id)
        .await
        .expect("delete_label failed");

    // Verify label is gone
    assert!(
        ctx.find_label(&label_id).is_none(),
        "Label should not be findable after deletion"
    );

    // Refresh to get updated task state after label deletion
    ctx.refresh().await.expect("refresh failed");

    // Verify task no longer has the label
    let task = ctx.find_item(&task_id).expect("Task should still exist");
    assert!(
        !task.labels.contains(&label_name),
        "Task should no longer have the deleted label"
    );

    // Clean up
    ctx.delete_task(&task_id).await.expect("delete_task failed");
}

// ============================================================================
// 5.2 Task Labeling Tests
// ============================================================================

/// Test adding a single label to a task.
///
/// Spec: 5.2 - test_add_single_label_to_task
/// - Create task and label
/// - Update task with `labels: ["label-name"]`
/// - Verify task has label
/// - Clean up: delete both
#[tokio::test]
async fn test_add_single_label_to_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Use unique label name to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let label_name = format!("e2e-test-single-label-{}", uuid);

    // Create a label
    let label_id = ctx
        .create_label(&label_name)
        .await
        .expect("create_label failed");

    // Create a task without labels
    let task_id = ctx
        .create_task("E2E test - add single label", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Add the label to the task
    let update_command = SyncCommand::new(
        SyncCommandType::ItemUpdate,
        serde_json::json!({
            "id": task_id,
            "labels": [label_name]
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify task has the label (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(
        task.labels.contains(&label_name),
        "Task should have the label"
    );

    // Clean up
    ctx.batch_delete(&[&task_id], &[], &[], &[&label_id])
        .await
        .expect("batch_delete failed");
}

/// Test adding multiple labels to a task at once.
///
/// Spec: 5.2 - test_add_multiple_labels_to_task
/// - Create task and 3 labels
/// - Update task with all 3 labels
/// - Verify task has all 3
/// - Clean up: delete all
#[tokio::test]
async fn test_add_multiple_labels_to_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Use unique label names to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let label_name_a = format!("e2e-test-multi-label-a-{}", uuid);
    let label_name_b = format!("e2e-test-multi-label-b-{}", uuid);
    let label_name_c = format!("e2e-test-multi-label-c-{}", uuid);

    // Create 3 labels
    let label1_id = ctx
        .create_label(&label_name_a)
        .await
        .expect("create_label failed");
    let label2_id = ctx
        .create_label(&label_name_b)
        .await
        .expect("create_label failed");
    let label3_id = ctx
        .create_label(&label_name_c)
        .await
        .expect("create_label failed");

    // Create a task without labels
    let task_id = ctx
        .create_task("E2E test - add multiple labels", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Add all labels to the task
    let update_command = SyncCommand::new(
        SyncCommandType::ItemUpdate,
        serde_json::json!({
            "id": task_id,
            "labels": [label_name_a, label_name_b, label_name_c]
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify task has all 3 labels (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(
        task.labels.contains(&label_name_a),
        "Task should have label a"
    );
    assert!(
        task.labels.contains(&label_name_b),
        "Task should have label b"
    );
    assert!(
        task.labels.contains(&label_name_c),
        "Task should have label c"
    );
    assert_eq!(task.labels.len(), 3, "Task should have exactly 3 labels");

    // Clean up
    ctx.batch_delete(&[&task_id], &[], &[], &[&label1_id, &label2_id, &label3_id])
        .await
        .expect("batch_delete failed");
}

/// Test removing one label from a task while keeping others.
///
/// Spec: 5.2 - test_remove_one_label_from_task
/// - Create task with labels ["a", "b", "c"]
/// - Update task with `labels: ["a", "c"]`
/// - Verify only "a" and "c" remain
/// - Clean up: delete all
#[tokio::test]
async fn test_remove_one_label_from_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Use unique label names to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let label_name_a = format!("e2e-test-remove-label-a-{}", uuid);
    let label_name_b = format!("e2e-test-remove-label-b-{}", uuid);
    let label_name_c = format!("e2e-test-remove-label-c-{}", uuid);

    // Create 3 labels
    let label1_id = ctx
        .create_label(&label_name_a)
        .await
        .expect("create_label failed");
    let label2_id = ctx
        .create_label(&label_name_b)
        .await
        .expect("create_label failed");
    let label3_id = ctx
        .create_label(&label_name_c)
        .await
        .expect("create_label failed");

    // Create a task with all 3 labels
    let task_id = ctx
        .create_task(
            "E2E test - remove one label",
            &inbox_id,
            Some(serde_json::json!({
                "labels": [label_name_a, label_name_b, label_name_c]
            })),
        )
        .await
        .expect("create_task failed");

    // Verify task has all 3 labels initially (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.labels.len(), 3, "Task should have 3 labels initially");

    // Remove label "b" by updating with only "a" and "c"
    let update_command = SyncCommand::new(
        SyncCommandType::ItemUpdate,
        serde_json::json!({
            "id": task_id,
            "labels": [label_name_a, label_name_c]
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify only "a" and "c" remain (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(
        task.labels.contains(&label_name_a),
        "Task should still have label a"
    );
    assert!(
        !task.labels.contains(&label_name_b),
        "Task should NOT have label b anymore"
    );
    assert!(
        task.labels.contains(&label_name_c),
        "Task should still have label c"
    );
    assert_eq!(task.labels.len(), 2, "Task should have exactly 2 labels");

    // Clean up
    ctx.batch_delete(&[&task_id], &[], &[], &[&label1_id, &label2_id, &label3_id])
        .await
        .expect("batch_delete failed");
}

/// Test replacing entire label set on a task.
///
/// Spec: 5.2 - test_replace_all_labels
/// - Create task with labels ["old1", "old2"]
/// - Update with `labels: ["new1", "new2"]`
/// - Verify only new labels present
/// - Clean up: delete all
#[tokio::test]
async fn test_replace_all_labels() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Use unique label names to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let label_name_old1 = format!("e2e-test-replace-old1-{}", uuid);
    let label_name_old2 = format!("e2e-test-replace-old2-{}", uuid);
    let label_name_new1 = format!("e2e-test-replace-new1-{}", uuid);
    let label_name_new2 = format!("e2e-test-replace-new2-{}", uuid);

    // Create old labels
    let old1_id = ctx
        .create_label(&label_name_old1)
        .await
        .expect("create_label failed");
    let old2_id = ctx
        .create_label(&label_name_old2)
        .await
        .expect("create_label failed");

    // Create new labels
    let new1_id = ctx
        .create_label(&label_name_new1)
        .await
        .expect("create_label failed");
    let new2_id = ctx
        .create_label(&label_name_new2)
        .await
        .expect("create_label failed");

    // Create a task with old labels
    let task_id = ctx
        .create_task(
            "E2E test - replace all labels",
            &inbox_id,
            Some(serde_json::json!({
                "labels": [label_name_old1, label_name_old2]
            })),
        )
        .await
        .expect("create_task failed");

    // Verify task has old labels initially (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.labels.contains(&label_name_old1));
    assert!(task.labels.contains(&label_name_old2));

    // Replace all labels with new ones
    let update_command = SyncCommand::new(
        SyncCommandType::ItemUpdate,
        serde_json::json!({
            "id": task_id,
            "labels": [label_name_new1, label_name_new2]
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify only new labels present (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(
        !task.labels.contains(&label_name_old1),
        "Task should NOT have old1"
    );
    assert!(
        !task.labels.contains(&label_name_old2),
        "Task should NOT have old2"
    );
    assert!(
        task.labels.contains(&label_name_new1),
        "Task should have new1"
    );
    assert!(
        task.labels.contains(&label_name_new2),
        "Task should have new2"
    );
    assert_eq!(task.labels.len(), 2, "Task should have exactly 2 labels");

    // Clean up
    ctx.batch_delete(
        &[&task_id],
        &[],
        &[],
        &[&old1_id, &old2_id, &new1_id, &new2_id],
    )
    .await
    .expect("batch_delete failed");
}

/// Test removing all labels from a task.
///
/// Spec: 5.2 - test_clear_all_labels
/// - Create task with labels
/// - Update with `labels: []`
/// - Verify task has no labels
/// - Clean up: delete all
#[tokio::test]
async fn test_clear_all_labels() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Use unique label names to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let label_name_a = format!("e2e-test-clear-label-a-{}", uuid);
    let label_name_b = format!("e2e-test-clear-label-b-{}", uuid);

    // Create labels
    let label1_id = ctx
        .create_label(&label_name_a)
        .await
        .expect("create_label failed");
    let label2_id = ctx
        .create_label(&label_name_b)
        .await
        .expect("create_label failed");

    // Create a task with labels
    let task_id = ctx
        .create_task(
            "E2E test - clear all labels",
            &inbox_id,
            Some(serde_json::json!({
                "labels": [label_name_a, label_name_b]
            })),
        )
        .await
        .expect("create_task failed");

    // Verify task has labels initially (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert_eq!(task.labels.len(), 2, "Task should have 2 labels initially");

    // Clear all labels
    let update_command = SyncCommand::new(
        SyncCommandType::ItemUpdate,
        serde_json::json!({
            "id": task_id,
            "labels": []
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify task has no labels (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(task.labels.is_empty(), "Task should have no labels");

    // Clean up
    ctx.batch_delete(&[&task_id], &[], &[], &[&label1_id, &label2_id])
        .await
        .expect("batch_delete failed");
}

/// Test that labels are case-insensitive.
///
/// Spec: 5.2 - test_label_case_insensitivity
/// - Create label "MyLabel"
/// - Create task with label "mylabel" (lowercase)
/// - Verify task has the label (normalized)
/// - Clean up: delete all
#[tokio::test]
async fn test_label_case_insensitivity() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Use unique label name to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let label_name_mixed = format!("e2e-test-CaseSensitive-{}", uuid);
    let label_name_lower = label_name_mixed.to_lowercase();

    // Create label with mixed case
    let label_id = ctx
        .create_label(&label_name_mixed)
        .await
        .expect("create_label failed");

    // Create a task using lowercase version of label name
    let task_id = ctx
        .create_task(
            "E2E test - label case insensitivity",
            &inbox_id,
            Some(serde_json::json!({
                "labels": [label_name_lower]
            })),
        )
        .await
        .expect("create_task failed");

    // Verify task has the label (Todoist normalizes label names)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    // The label name should be normalized to match the created label
    assert!(task.labels.len() == 1, "Task should have exactly 1 label");
    // Case-insensitive comparison - either form should work
    let has_label = task
        .labels
        .iter()
        .any(|l| l.eq_ignore_ascii_case(&label_name_mixed));
    assert!(
        has_label,
        "Task should have the label (case-insensitive match)"
    );

    // Clean up
    ctx.batch_delete(&[&task_id], &[], &[], &[&label_id])
        .await
        .expect("batch_delete failed");
}

/// Test adding a label using item_update command.
///
/// Spec: 5.2 - test_add_label_via_item_update
/// - Create task with no labels
/// - Get current labels, append new one
/// - Call `item_update` with new labels array
/// - Verify label added
/// - Clean up: delete all
#[tokio::test]
async fn test_add_label_via_item_update() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Use unique label name to avoid conflicts with leftover data from previous runs
    let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let label_name = format!("e2e-test-update-add-label-{}", uuid);

    // Create a label
    let label_id = ctx
        .create_label(&label_name)
        .await
        .expect("create_label failed");

    // Create a task without any labels
    let task_id = ctx
        .create_task("E2E test - add label via item_update", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Verify task has no labels initially (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(
        task.labels.is_empty(),
        "Task should have no labels initially"
    );

    // Get current labels (empty) and append new one
    let mut labels = task.labels.clone();
    labels.push(label_name.clone());

    // Update via item_update
    let update_command = SyncCommand::new(
        SyncCommandType::ItemUpdate,
        serde_json::json!({
            "id": task_id,
            "labels": labels
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "item_update should succeed");

    // Verify label was added (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist in cache");
    assert!(
        task.labels.contains(&label_name),
        "Task should have the label after item_update"
    );
    assert_eq!(task.labels.len(), 1, "Task should have exactly 1 label");

    // Clean up
    ctx.batch_delete(&[&task_id], &[], &[], &[&label_id])
        .await
        .expect("batch_delete failed");
}
