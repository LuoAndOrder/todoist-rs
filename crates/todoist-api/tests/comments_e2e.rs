//! End-to-end tests for comment operations.
//!
//! These tests validate task and project comment CRUD operations against the real Todoist API.
//!
//! Run with: cargo test -p todoist-api-rs --features extended-e2e --test comments_e2e
//!
//! Tests follow the naming convention from specs/E2E_TEST_SPEC.md section 7.
//!
//! ## Rate Limit Mitigation
//!
//! These tests use `TestContext` which performs ONE full sync at initialization
//! and uses partial (incremental) syncs for all subsequent operations. This
//! dramatically reduces API calls and helps stay within Todoist's rate limits:
//! - Full sync: 100 requests / 15 minutes
//! - Partial sync: 1000 requests / 15 minutes
//!
//! ## Note on Comments
//!
//! Comments may require Todoist Pro for some features. If the test account does
//! not have Pro or comments are not available, tests will gracefully skip.

#![cfg(feature = "extended-e2e")]

mod test_context;

use test_context::TestContext;
use todoist_api_rs::sync::{SyncCommand, SyncCommandType};

/// Helper to check if task comments are available.
/// Returns true if the first comment test succeeds, false if comments are unavailable.
async fn comments_available(ctx: &mut TestContext, task_id: &str) -> bool {
    // Try to create a test comment - if it fails, comments are not available
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        SyncCommandType::NoteAdd,
        &temp_id,
        serde_json::json!({
            "item_id": task_id,
            "content": "Test comment for availability check"
        }),
    );

    match ctx.execute(vec![command]).await {
        Ok(response) => {
            if response.has_errors() {
                eprintln!(
                    "Comments not available (may require Pro): {:?}",
                    response.errors()
                );
                return false;
            }
            // Clean up the test comment
            if let Some(note_id) = response.real_id(&temp_id) {
                let _ = ctx.delete_note(note_id).await;
            }
            true
        }
        Err(e) => {
            eprintln!("Comments not available (may require Pro): {}", e);
            false
        }
    }
}

/// Helper to check if project comments are available.
/// Returns true if the first project comment test succeeds, false if not available.
async fn project_comments_available(ctx: &mut TestContext, project_id: &str) -> bool {
    // Try to create a test project comment
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        SyncCommandType::ProjectNoteAdd,
        &temp_id,
        serde_json::json!({
            "project_id": project_id,
            "content": "Test project comment for availability check"
        }),
    );

    match ctx.execute(vec![command]).await {
        Ok(response) => {
            if response.has_errors() {
                eprintln!(
                    "Project comments not available (may require Pro): {:?}",
                    response.errors()
                );
                return false;
            }
            // Clean up the test comment
            if let Some(note_id) = response.real_id(&temp_id) {
                let _ = ctx.delete_project_note(note_id).await;
            }
            true
        }
        Err(e) => {
            eprintln!("Project comments not available (may require Pro): {}", e);
            false
        }
    }
}

// ============================================================================
// 7.1 Task Comments Tests
// ============================================================================

/// Test adding a comment to a task.
///
/// Spec: 7.1 - test_add_task_comment
/// - Create task
/// - Call `note_add` with `{item_id: ..., content: "This is a comment"}`
/// - Sync and verify comment exists in `notes`
/// - Clean up: delete comment and task
#[tokio::test]
async fn test_add_task_comment() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task for the comment
    let task_id = ctx
        .create_task("E2E test - task comment task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Check if comments are available
    if !comments_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: comments may require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Add a comment to the task
    let comment_id = ctx
        .create_task_comment(&task_id, "This is a test comment")
        .await
        .expect("create_task_comment failed");

    // Verify comment exists in cache
    let note = ctx
        .find_note(&comment_id)
        .expect("Note should exist in cache");

    assert_eq!(note.item_id, task_id, "Note should be for the task");
    assert_eq!(note.content, "This is a test comment");
    assert!(!note.is_deleted, "Note should not be deleted");

    // Clean up
    ctx.batch_delete_with_notes(&[&task_id], &[], &[&comment_id], &[])
        .await
        .expect("cleanup failed");
}

/// Test adding a comment with markdown formatting.
///
/// Spec: 7.1 - test_add_comment_with_formatting
/// - Create task
/// - Add comment with `**bold** and *italic* text`
/// - Verify content preserved
/// - Clean up: delete all
#[tokio::test]
async fn test_add_comment_with_formatting() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task for the comment
    let task_id = ctx
        .create_task("E2E test - formatted comment task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Check if comments are available
    if !comments_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: comments may require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Add a comment with markdown formatting
    let formatted_content = "This has **bold** and *italic* text, plus `code`";
    let comment_id = ctx
        .create_task_comment(&task_id, formatted_content)
        .await
        .expect("create_task_comment failed");

    // Verify formatted content preserved
    let note = ctx
        .find_note(&comment_id)
        .expect("Note should exist in cache");
    assert_eq!(
        note.content, formatted_content,
        "Formatted content should be preserved"
    );

    // Clean up
    ctx.batch_delete_with_notes(&[&task_id], &[], &[&comment_id], &[])
        .await
        .expect("cleanup failed");
}

/// Test modifying an existing comment.
///
/// Spec: 7.1 - test_update_task_comment
/// - Create task and comment
/// - Call `note_update` with new content
/// - Verify content changed
/// - Clean up: delete all
#[tokio::test]
async fn test_update_task_comment() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task for the comment
    let task_id = ctx
        .create_task("E2E test - update comment task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Check if comments are available
    if !comments_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: comments may require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Create initial comment
    let comment_id = ctx
        .create_task_comment(&task_id, "Original comment content")
        .await
        .expect("create_task_comment failed");

    // Verify initial content
    let note = ctx.find_note(&comment_id).expect("Note should exist");
    assert_eq!(note.content, "Original comment content");

    // Update the comment
    let update_command = SyncCommand::new(
        SyncCommandType::NoteUpdate,
        serde_json::json!({
            "id": comment_id,
            "content": "Updated comment content"
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "note_update should succeed");

    // Verify updated content
    let updated_note = ctx.find_note(&comment_id).expect("Note should still exist");
    assert_eq!(updated_note.content, "Updated comment content");

    // Clean up
    ctx.batch_delete_with_notes(&[&task_id], &[], &[&comment_id], &[])
        .await
        .expect("cleanup failed");
}

/// Test deleting a comment from a task.
///
/// Spec: 7.1 - test_delete_task_comment
/// - Create task and comment
/// - Call `note_delete`
/// - Verify comment gone, task still exists
/// - Clean up: delete task
#[tokio::test]
async fn test_delete_task_comment() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task for the comment
    let task_id = ctx
        .create_task("E2E test - delete comment task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Check if comments are available
    if !comments_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: comments may require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Create a comment
    let comment_id = ctx
        .create_task_comment(&task_id, "Comment to be deleted")
        .await
        .expect("create_task_comment failed");

    // Verify comment exists
    assert!(
        ctx.find_note(&comment_id).is_some(),
        "Comment should exist before deletion"
    );

    // Delete the comment
    ctx.delete_note(&comment_id)
        .await
        .expect("delete_note failed");

    // Verify comment is gone
    assert!(
        ctx.find_note(&comment_id).is_none(),
        "Comment should not be findable after deletion"
    );

    // Verify task still exists
    assert!(
        ctx.find_item(&task_id).is_some(),
        "Task should still exist after comment deletion"
    );

    // Clean up
    ctx.delete_task(&task_id).await.expect("cleanup failed");
}

/// Test adding multiple comments to one task.
///
/// Spec: 7.1 - test_multiple_comments_on_task
/// - Create task
/// - Add 3 comments
/// - Verify all exist and maintain order
/// - Clean up: delete all
#[tokio::test]
async fn test_multiple_comments_on_task() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a task for the comments
    let task_id = ctx
        .create_task("E2E test - multiple comments task", &inbox_id, None)
        .await
        .expect("create_task failed");

    // Check if comments are available
    if !comments_available(&mut ctx, &task_id).await {
        eprintln!("Skipping test: comments may require Todoist Pro");
        ctx.delete_task(&task_id).await.expect("cleanup failed");
        return;
    }

    // Create 3 comments
    let comment1_id = ctx
        .create_task_comment(&task_id, "First comment")
        .await
        .expect("create comment 1 failed");

    let comment2_id = ctx
        .create_task_comment(&task_id, "Second comment")
        .await
        .expect("create comment 2 failed");

    let comment3_id = ctx
        .create_task_comment(&task_id, "Third comment")
        .await
        .expect("create comment 3 failed");

    // Verify all 3 comments exist
    assert!(
        ctx.find_note(&comment1_id).is_some(),
        "Comment 1 should exist"
    );
    assert!(
        ctx.find_note(&comment2_id).is_some(),
        "Comment 2 should exist"
    );
    assert!(
        ctx.find_note(&comment3_id).is_some(),
        "Comment 3 should exist"
    );

    // Verify all comments are for the same task
    let comments_for_task = ctx.find_notes_for_task(&task_id);
    assert!(
        comments_for_task.len() >= 3,
        "Task should have at least 3 comments, found {}",
        comments_for_task.len()
    );

    // Verify different contents exist
    let contents: Vec<&str> = comments_for_task
        .iter()
        .map(|n| n.content.as_str())
        .collect();
    assert!(contents.contains(&"First comment"));
    assert!(contents.contains(&"Second comment"));
    assert!(contents.contains(&"Third comment"));

    // Clean up
    ctx.batch_delete_with_notes(
        &[&task_id],
        &[],
        &[&comment1_id, &comment2_id, &comment3_id],
        &[],
    )
    .await
    .expect("cleanup failed");
}

// ============================================================================
// 7.2 Project Comments Tests
// ============================================================================

/// Test adding a comment to a project.
///
/// Spec: 7.2 - test_add_project_comment
/// - Create project
/// - Call `project_note_add` with `{project_id: ..., content: "Project note"}`
/// - Sync and verify in `project_notes`
/// - Clean up: delete all
#[tokio::test]
async fn test_add_project_comment() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create a project for the comment
    let project_id = ctx
        .create_project("E2E_Test_ProjectComment")
        .await
        .expect("create_project failed");

    // Check if project comments are available
    if !project_comments_available(&mut ctx, &project_id).await {
        eprintln!("Skipping test: project comments may require Todoist Pro");
        ctx.delete_project(&project_id)
            .await
            .expect("cleanup failed");
        return;
    }

    // Add a comment to the project
    let comment_id = ctx
        .create_project_comment(&project_id, "This is a project comment")
        .await
        .expect("create_project_comment failed");

    // Verify comment exists in cache
    let note = ctx
        .find_project_note(&comment_id)
        .expect("Project note should exist in cache");

    assert_eq!(
        note.project_id, project_id,
        "Note should be for the project"
    );
    assert_eq!(note.content, "This is a project comment");
    assert!(!note.is_deleted, "Note should not be deleted");

    // Clean up
    ctx.batch_delete_with_notes(&[], &[&project_id], &[], &[&comment_id])
        .await
        .expect("cleanup failed");
}

/// Test modifying a project comment.
///
/// Spec: 7.2 - test_update_project_comment
/// - Create project and comment
/// - Call `project_note_update`
/// - Verify change
/// - Clean up: delete all
#[tokio::test]
async fn test_update_project_comment() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create a project for the comment
    let project_id = ctx
        .create_project("E2E_Test_UpdateProjectComment")
        .await
        .expect("create_project failed");

    // Check if project comments are available
    if !project_comments_available(&mut ctx, &project_id).await {
        eprintln!("Skipping test: project comments may require Todoist Pro");
        ctx.delete_project(&project_id)
            .await
            .expect("cleanup failed");
        return;
    }

    // Create initial comment
    let comment_id = ctx
        .create_project_comment(&project_id, "Original project note")
        .await
        .expect("create_project_comment failed");

    // Verify initial content
    let note = ctx
        .find_project_note(&comment_id)
        .expect("Note should exist");
    assert_eq!(note.content, "Original project note");

    // Update the comment
    let update_command = SyncCommand::new(
        SyncCommandType::ProjectNoteUpdate,
        serde_json::json!({
            "id": comment_id,
            "content": "Updated project note"
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "project_note_update should succeed");

    // Verify updated content
    let updated_note = ctx
        .find_project_note(&comment_id)
        .expect("Note should still exist");
    assert_eq!(updated_note.content, "Updated project note");

    // Clean up
    ctx.batch_delete_with_notes(&[], &[&project_id], &[], &[&comment_id])
        .await
        .expect("cleanup failed");
}

/// Test deleting a project comment.
///
/// Spec: 7.2 - test_delete_project_comment
/// - Create project and comment
/// - Call `project_note_delete`
/// - Verify comment gone
/// - Clean up: delete project
#[tokio::test]
async fn test_delete_project_comment() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create a project for the comment
    let project_id = ctx
        .create_project("E2E_Test_DeleteProjectComment")
        .await
        .expect("create_project failed");

    // Check if project comments are available
    if !project_comments_available(&mut ctx, &project_id).await {
        eprintln!("Skipping test: project comments may require Todoist Pro");
        ctx.delete_project(&project_id)
            .await
            .expect("cleanup failed");
        return;
    }

    // Create a comment
    let comment_id = ctx
        .create_project_comment(&project_id, "Project comment to be deleted")
        .await
        .expect("create_project_comment failed");

    // Verify comment exists
    assert!(
        ctx.find_project_note(&comment_id).is_some(),
        "Comment should exist before deletion"
    );

    // Delete the comment
    ctx.delete_project_note(&comment_id)
        .await
        .expect("delete_project_note failed");

    // Verify comment is gone
    assert!(
        ctx.find_project_note(&comment_id).is_none(),
        "Comment should not be findable after deletion"
    );

    // Verify project still exists
    assert!(
        ctx.find_project(&project_id).is_some(),
        "Project should still exist after comment deletion"
    );

    // Clean up
    ctx.delete_project(&project_id)
        .await
        .expect("cleanup failed");
}
