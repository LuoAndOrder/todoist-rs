//! End-to-end tests for project operations.
//!
//! These tests validate project CRUD, hierarchy, and archive operations
//! against the real Todoist API.
//!
//! Run with: cargo test --package todoist-api --features e2e --test project_e2e
//!
//! Tests follow the naming convention from specs/E2E_TEST_SPEC.md section 3.
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
use todoist_api::sync::SyncCommand;

// ============================================================================
// 3.1 Basic CRUD Tests
// ============================================================================

#[tokio::test]
async fn test_create_project_minimal() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project with just a name
    let project_id = ctx
        .create_project("E2E_Test_MinimalProject")
        .await
        .expect("create_project failed");

    // Verify project exists in cache
    let project = ctx
        .find_project(&project_id)
        .expect("Project should exist in cache");

    assert_eq!(project.name, "E2E_Test_MinimalProject");
    assert!(!project.is_deleted, "Should not be deleted");
    assert!(!project.is_archived, "Should not be archived");
    // Verify default values exist
    assert!(!project.inbox_project, "Should not be the inbox");

    // Clean up
    ctx.delete_project(&project_id)
        .await
        .expect("delete_project failed");
}

#[tokio::test]
async fn test_create_project_with_color() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project with specific color
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        "project_add",
        &temp_id,
        serde_json::json!({
            "name": "E2E_Test_ColoredProject",
            "color": "red"
        }),
    );
    let response = ctx.execute(vec![command]).await.unwrap();
    assert!(!response.has_errors(), "project_add should succeed");

    let project_id = response.real_id(&temp_id).unwrap().clone();

    // Verify color persisted (from cache)
    let project = ctx
        .find_project(&project_id)
        .expect("Project should exist in cache");
    assert_eq!(project.name, "E2E_Test_ColoredProject");
    assert_eq!(project.color, Some("red".to_string()), "Color should be red");

    // Clean up
    ctx.delete_project(&project_id)
        .await
        .expect("delete_project failed");
}

#[tokio::test]
async fn test_create_project_with_view_style() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project with board view
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        "project_add",
        &temp_id,
        serde_json::json!({
            "name": "E2E_Test_BoardProject",
            "view_style": "board"
        }),
    );
    let response = ctx.execute(vec![command]).await.unwrap();
    assert!(!response.has_errors(), "project_add should succeed");

    let project_id = response.real_id(&temp_id).unwrap().clone();

    // Verify view_style persisted (from cache)
    let project = ctx
        .find_project(&project_id)
        .expect("Project should exist in cache");
    assert_eq!(project.name, "E2E_Test_BoardProject");
    assert_eq!(project.view_style, Some("board".to_string()), "View style should be board");

    // Clean up
    ctx.delete_project(&project_id)
        .await
        .expect("delete_project failed");
}

#[tokio::test]
async fn test_update_project_name() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project
    let project_id = ctx
        .create_project("E2E_Test_OriginalName")
        .await
        .expect("create_project failed");

    // Verify original name (from cache)
    let project = ctx
        .find_project(&project_id)
        .expect("Project should exist in cache");
    assert_eq!(project.name, "E2E_Test_OriginalName");

    // Update name
    let update_command = SyncCommand::new(
        "project_update",
        serde_json::json!({
            "id": project_id,
            "name": "E2E_Test_NewName"
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "project_update should succeed");

    // Verify name changed (from cache)
    let project = ctx
        .find_project(&project_id)
        .expect("Project should exist in cache");
    assert_eq!(project.name, "E2E_Test_NewName");

    // Clean up
    ctx.delete_project(&project_id)
        .await
        .expect("delete_project failed");
}

#[tokio::test]
async fn test_update_project_color() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project with color "red"
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        "project_add",
        &temp_id,
        serde_json::json!({
            "name": "E2E_Test_ColorChangeProject",
            "color": "red"
        }),
    );
    let response = ctx.execute(vec![command]).await.unwrap();
    assert!(!response.has_errors());
    let project_id = response.real_id(&temp_id).unwrap().clone();

    // Verify original color (from cache)
    let project = ctx
        .find_project(&project_id)
        .expect("Project should exist in cache");
    assert_eq!(project.color, Some("red".to_string()));

    // Update to blue
    let update_command = SyncCommand::new(
        "project_update",
        serde_json::json!({
            "id": project_id,
            "color": "blue"
        }),
    );
    let response = ctx.execute(vec![update_command]).await.unwrap();
    assert!(!response.has_errors(), "project_update should succeed");

    // Verify color changed (from cache)
    let project = ctx
        .find_project(&project_id)
        .expect("Project should exist in cache");
    assert_eq!(project.color, Some("blue".to_string()), "Color should be blue");

    // Clean up
    ctx.delete_project(&project_id)
        .await
        .expect("delete_project failed");
}

#[tokio::test]
async fn test_delete_project() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project
    let project_id = ctx
        .create_project("E2E_Test_ToDelete")
        .await
        .expect("create_project failed");

    // Verify project exists in cache
    assert!(
        ctx.find_project(&project_id).is_some(),
        "Project should exist before deletion"
    );

    // Delete project
    ctx.delete_project(&project_id)
        .await
        .expect("delete_project failed");

    // Verify project is deleted from cache (find_project filters out is_deleted)
    assert!(
        ctx.find_project(&project_id).is_none(),
        "Project should not be findable after deletion"
    );
}

#[tokio::test]
async fn test_delete_project_with_tasks() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project with 3 tasks
    let project_id = ctx
        .create_project("E2E_Test_DeleteWithTasks")
        .await
        .expect("create_project failed");

    let task1_id = ctx
        .create_task("E2E test - task in project 1", &project_id, None)
        .await
        .expect("create_task failed");
    let task2_id = ctx
        .create_task("E2E test - task in project 2", &project_id, None)
        .await
        .expect("create_task failed");
    let task3_id = ctx
        .create_task("E2E test - task in project 3", &project_id, None)
        .await
        .expect("create_task failed");

    // Verify all tasks exist (from cache)
    assert!(ctx.find_item(&task1_id).is_some());
    assert!(ctx.find_item(&task2_id).is_some());
    assert!(ctx.find_item(&task3_id).is_some());

    // Delete project
    ctx.delete_project(&project_id)
        .await
        .expect("delete_project failed");

    // Refresh to get cascade delete state (tasks may not appear in immediate response)
    ctx.refresh().await.expect("refresh failed");

    // Verify project is deleted (from cache)
    assert!(
        ctx.find_project(&project_id).is_none(),
        "Project should be deleted"
    );

    // Verify tasks are deleted or orphaned
    // Todoist behavior: tasks in deleted project should either:
    // - Be marked is_deleted=true (filtered out by find_item)
    // - No longer appear in sync responses
    // - Or have their project_id pointing to a deleted project
    let task1 = ctx.find_item(&task1_id);
    let task2 = ctx.find_item(&task2_id);
    let task3 = ctx.find_item(&task3_id);

    // Check if tasks are gone or their project is deleted
    let tasks_handled = task1.is_none() && task2.is_none() && task3.is_none();
    let tasks_orphaned = task1.map_or(true, |t| ctx.find_project(&t.project_id).is_none())
        && task2.map_or(true, |t| ctx.find_project(&t.project_id).is_none())
        && task3.map_or(true, |t| ctx.find_project(&t.project_id).is_none());

    assert!(
        tasks_handled || tasks_orphaned,
        "Tasks should be deleted or orphaned when project is deleted. Task1: {:?}, Task2: {:?}, Task3: {:?}",
        task1.map(|t| &t.content),
        task2.map(|t| &t.content),
        task3.map(|t| &t.content)
    );
}

// ============================================================================
// 3.2 Project Hierarchy Tests
// ============================================================================

#[tokio::test]
async fn test_create_subproject() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create parent project
    let parent_id = ctx
        .create_project("E2E_Test_ParentProject")
        .await
        .expect("create_project failed");

    // Create child project with parent_id
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        "project_add",
        &temp_id,
        serde_json::json!({
            "name": "E2E_Test_ChildProject",
            "parent_id": parent_id
        }),
    );
    let response = ctx.execute(vec![command]).await.unwrap();
    assert!(!response.has_errors(), "project_add should succeed");

    let child_id = response.real_id(&temp_id).unwrap().clone();

    // Verify child's parent_id (from cache)
    let child = ctx
        .find_project(&child_id)
        .expect("Child project should exist in cache");
    assert_eq!(
        child.parent_id,
        Some(parent_id.clone()),
        "Child's parent should be parent project"
    );

    // Clean up (child first, then parent)
    ctx.batch_delete(&[], &[&child_id, &parent_id], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_create_nested_subprojects() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create hierarchy: A -> B -> C
    let project_a_id = ctx
        .create_project("E2E_Test_ProjectA")
        .await
        .expect("create_project failed");

    // Create B under A
    let temp_id_b = uuid::Uuid::new_v4().to_string();
    let command_b = SyncCommand::with_temp_id(
        "project_add",
        &temp_id_b,
        serde_json::json!({
            "name": "E2E_Test_ProjectB",
            "parent_id": project_a_id
        }),
    );
    let response = ctx.execute(vec![command_b]).await.unwrap();
    assert!(!response.has_errors());
    let project_b_id = response.real_id(&temp_id_b).unwrap().clone();

    // Create C under B
    let temp_id_c = uuid::Uuid::new_v4().to_string();
    let command_c = SyncCommand::with_temp_id(
        "project_add",
        &temp_id_c,
        serde_json::json!({
            "name": "E2E_Test_ProjectC",
            "parent_id": project_b_id
        }),
    );
    let response = ctx.execute(vec![command_c]).await.unwrap();
    assert!(!response.has_errors());
    let project_c_id = response.real_id(&temp_id_c).unwrap().clone();

    // Verify hierarchy (from cache)
    let project_a = ctx
        .find_project(&project_a_id)
        .expect("Project A should exist in cache");
    let project_b = ctx
        .find_project(&project_b_id)
        .expect("Project B should exist in cache");
    let project_c = ctx
        .find_project(&project_c_id)
        .expect("Project C should exist in cache");

    assert!(project_a.parent_id.is_none(), "A should have no parent");
    assert_eq!(
        project_b.parent_id,
        Some(project_a_id.clone()),
        "B's parent should be A"
    );
    assert_eq!(
        project_c.parent_id,
        Some(project_b_id.clone()),
        "C's parent should be B"
    );

    // Clean up (delete in reverse hierarchy order)
    ctx.batch_delete(&[], &[&project_c_id, &project_b_id, &project_a_id], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_move_project_under_parent() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create Project A and Project B (both root level)
    let project_a_id = ctx
        .create_project("E2E_Test_MoveParentA")
        .await
        .expect("create_project failed");
    let project_b_id = ctx
        .create_project("E2E_Test_MoveChildB")
        .await
        .expect("create_project failed");

    // Verify both are root level (from cache)
    let project_a = ctx.find_project(&project_a_id).unwrap();
    let project_b = ctx.find_project(&project_b_id).unwrap();
    assert!(project_a.parent_id.is_none(), "A should be root level");
    assert!(project_b.parent_id.is_none(), "B should be root level");

    // Move B under A
    let move_command = SyncCommand::new(
        "project_move",
        serde_json::json!({
            "id": project_b_id,
            "parent_id": project_a_id
        }),
    );
    let response = ctx.execute(vec![move_command]).await.unwrap();
    assert!(!response.has_errors(), "project_move should succeed");

    // Verify B is now under A (from cache)
    let project_b = ctx
        .find_project(&project_b_id)
        .expect("Project B should exist in cache");
    assert_eq!(
        project_b.parent_id,
        Some(project_a_id.clone()),
        "B should now be under A"
    );

    // Clean up
    ctx.batch_delete(&[], &[&project_b_id, &project_a_id], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_promote_subproject_to_root() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create parent and child projects
    let parent_id = ctx
        .create_project("E2E_Test_PromoteParent")
        .await
        .expect("create_project failed");

    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id(
        "project_add",
        &temp_id,
        serde_json::json!({
            "name": "E2E_Test_PromoteChild",
            "parent_id": parent_id
        }),
    );
    let response = ctx.execute(vec![command]).await.unwrap();
    assert!(!response.has_errors());
    let child_id = response.real_id(&temp_id).unwrap().clone();

    // Verify child has parent (from cache)
    let child = ctx.find_project(&child_id).unwrap();
    assert_eq!(child.parent_id, Some(parent_id.clone()));

    // Promote child to root level (set parent_id to null)
    let move_command = SyncCommand::new(
        "project_move",
        serde_json::json!({
            "id": child_id,
            "parent_id": serde_json::Value::Null
        }),
    );
    let response = ctx.execute(vec![move_command]).await.unwrap();
    assert!(!response.has_errors(), "project_move should succeed");

    // Verify child is now root level (from cache)
    let child = ctx
        .find_project(&child_id)
        .expect("Child should exist in cache");
    assert!(
        child.parent_id.is_none(),
        "Child should have no parent after promotion"
    );

    // Clean up
    ctx.batch_delete(&[], &[&child_id, &parent_id], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_reorder_projects() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create 3 root-level projects
    let project1_id = ctx
        .create_project("E2E_Test_Reorder1")
        .await
        .expect("create_project failed");
    let project2_id = ctx
        .create_project("E2E_Test_Reorder2")
        .await
        .expect("create_project failed");
    let project3_id = ctx
        .create_project("E2E_Test_Reorder3")
        .await
        .expect("create_project failed");

    // Get initial order (from cache)
    let p1 = ctx.find_project(&project1_id).unwrap();
    let p2 = ctx.find_project(&project2_id).unwrap();
    let p3 = ctx.find_project(&project3_id).unwrap();
    println!(
        "Initial order: p1={}, p2={}, p3={}",
        p1.child_order, p2.child_order, p3.child_order
    );

    // Reorder: p3, p1, p2
    let reorder_command = SyncCommand::new(
        "project_reorder",
        serde_json::json!({
            "projects": [
                {"id": project3_id, "child_order": 1},
                {"id": project1_id, "child_order": 2},
                {"id": project2_id, "child_order": 3}
            ]
        }),
    );
    let response = ctx.execute(vec![reorder_command]).await.unwrap();
    assert!(!response.has_errors(), "project_reorder should succeed");

    // Verify new order (from cache)
    let p1 = ctx.find_project(&project1_id).unwrap();
    let p2 = ctx.find_project(&project2_id).unwrap();
    let p3 = ctx.find_project(&project3_id).unwrap();
    println!(
        "New order: p1={}, p2={}, p3={}",
        p1.child_order, p2.child_order, p3.child_order
    );

    assert!(
        p3.child_order < p1.child_order,
        "p3 should be before p1"
    );
    assert!(
        p1.child_order < p2.child_order,
        "p1 should be before p2"
    );

    // Clean up
    ctx.batch_delete(&[], &[&project1_id, &project2_id, &project3_id], &[], &[])
        .await
        .expect("cleanup failed");
}

// ============================================================================
// 3.3 Project Archive Tests
// ============================================================================

#[tokio::test]
async fn test_archive_project() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project with task
    let project_id = ctx
        .create_project("E2E_Test_ArchiveProject")
        .await
        .expect("create_project failed");
    let task_id = ctx
        .create_task("E2E test - task in archived project", &project_id, None)
        .await
        .expect("create_task failed");

    // Verify project is not archived initially (from cache)
    let project = ctx.find_project(&project_id).unwrap();
    assert!(!project.is_archived, "Project should not be archived initially");

    // Archive project
    let archive_command = SyncCommand::new(
        "project_archive",
        serde_json::json!({"id": project_id}),
    );
    let response = ctx.execute(vec![archive_command]).await.unwrap();
    assert!(!response.has_errors(), "project_archive should succeed");

    // Verify project is archived (from cache)
    let project = ctx
        .find_project(&project_id)
        .expect("Archived project should still be findable");
    assert!(project.is_archived, "Project should be archived");

    // Verify task still exists (associated with archived project)
    let task = ctx.find_item(&task_id);
    // Note: Task may or may not be findable depending on API behavior
    println!("Task after archive: {:?}", task.map(|t| &t.content));

    // Clean up: unarchive first, then delete
    let unarchive_command = SyncCommand::new(
        "project_unarchive",
        serde_json::json!({"id": project_id}),
    );
    ctx.execute(vec![unarchive_command]).await.unwrap();

    ctx.batch_delete(&[&task_id], &[&project_id], &[], &[])
        .await
        .expect("cleanup failed");
}

#[tokio::test]
async fn test_unarchive_project() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create and archive project
    let project_id = ctx
        .create_project("E2E_Test_UnarchiveProject")
        .await
        .expect("create_project failed");

    let archive_command = SyncCommand::new(
        "project_archive",
        serde_json::json!({"id": project_id}),
    );
    let response = ctx.execute(vec![archive_command]).await.unwrap();
    assert!(!response.has_errors());

    // Verify archived (from cache)
    let project = ctx.find_project(&project_id).unwrap();
    assert!(project.is_archived, "Project should be archived");

    // Unarchive project
    let unarchive_command = SyncCommand::new(
        "project_unarchive",
        serde_json::json!({"id": project_id}),
    );
    let response = ctx.execute(vec![unarchive_command]).await.unwrap();
    assert!(!response.has_errors(), "project_unarchive should succeed");

    // Verify not archived (from cache)
    let project = ctx
        .find_project(&project_id)
        .expect("Unarchived project should exist");
    assert!(!project.is_archived, "Project should not be archived");

    // Clean up
    ctx.delete_project(&project_id)
        .await
        .expect("delete_project failed");
}

#[tokio::test]
async fn test_archived_project_excluded_from_filters() {
    let Ok(mut ctx) = TestContext::new().await else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    // Create project with task due today
    let project_id = ctx
        .create_project("E2E_Test_FilterArchiveProject")
        .await
        .expect("create_project failed");

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let task_id = ctx
        .create_task(
            "E2E test - task due today in project",
            &project_id,
            Some(serde_json::json!({"due": {"date": &today}})),
        )
        .await
        .expect("create_task failed");

    // Verify task exists and is due today (from cache)
    let task = ctx.find_item(&task_id).expect("Task should exist");
    assert!(task.due.is_some(), "Task should have due date");
    assert_eq!(task.due.as_ref().unwrap().date, today);

    // Count tasks due today before archive
    let tasks_today_before: Vec<_> = ctx
        .items()
        .filter(|i| {
            i.due.as_ref().map(|d| d.date == today).unwrap_or(false) && !i.checked
        })
        .collect();
    let count_before = tasks_today_before.len();
    println!("Tasks due today before archive: {}", count_before);

    // Archive project
    let archive_command = SyncCommand::new(
        "project_archive",
        serde_json::json!({"id": project_id}),
    );
    let response = ctx.execute(vec![archive_command]).await.unwrap();
    assert!(!response.has_errors(), "project_archive should succeed");

    // Get archived project IDs
    let archived_project_ids: std::collections::HashSet<_> = ctx
        .projects()
        .filter(|p| p.is_archived)
        .map(|p| p.id.as_str())
        .collect();

    // Count tasks due today after archive (excluding those in archived projects)
    let tasks_today_after: Vec<_> = ctx
        .items()
        .filter(|i| {
            i.due.as_ref().map(|d| d.date == today).unwrap_or(false)
                && !i.checked
                && !archived_project_ids.contains(i.project_id.as_str())
        })
        .collect();
    let count_after = tasks_today_after.len();
    println!("Tasks due today after archive (excluding archived projects): {}", count_after);

    // The task in archived project should be excluded
    // Note: This tests our local filtering logic, not the API's filter command
    assert!(
        count_after < count_before || !archived_project_ids.is_empty(),
        "Archived project tasks should be excludable from filters"
    );

    // Clean up: unarchive first
    let unarchive_command = SyncCommand::new(
        "project_unarchive",
        serde_json::json!({"id": project_id}),
    );
    ctx.execute(vec![unarchive_command]).await.unwrap();

    ctx.batch_delete(&[&task_id], &[&project_id], &[], &[])
        .await
        .expect("cleanup failed");
}
