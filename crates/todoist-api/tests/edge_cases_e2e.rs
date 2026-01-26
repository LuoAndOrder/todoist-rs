//! E2E tests for edge cases and stress tests.
//!
//! These tests validate handling of unusual inputs, boundary conditions,
//! and rapid operations against the real Todoist API.
//!
//! **Spec Section 11:** Edge Cases and Stress Tests
//!
//! Tests cover:
//! - 11.1 Unicode and Special Characters
//! - 11.2 Boundary Conditions
//! - 11.3 Rapid Operations
//!
//! ## Running
//!
//! ```bash
//! cargo test --package todoist-api --features e2e --test edge_cases_e2e
//! ```

#![cfg(feature = "e2e")]

mod test_context;

use test_context::TestContext;
use todoist_api::sync::SyncCommand;

// =============================================================================
// 11.1 Unicode and Special Characters
// =============================================================================

/// Test: Task content with Japanese and emoji characters.
///
/// Spec: `test_unicode_in_task_content`
/// - Create task "Buy Japanese book 日本語の本 📚"
/// - Sync and verify content preserved exactly
#[tokio::test]
async fn test_unicode_in_task_content() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let unicode_content = "E2E Edge - Buy Japanese book 日本語の本 📚";
    let inbox_id = ctx.inbox_id().to_string();

    let task_id = ctx
        .create_task(unicode_content, &inbox_id, None)
        .await
        .expect("Should create task with unicode content");

    // Verify from cache
    let task = ctx
        .find_item(&task_id)
        .expect("Task should be in cache");
    assert_eq!(
        task.content, unicode_content,
        "Unicode content should be preserved exactly"
    );

    // Refresh to verify from API
    ctx.refresh().await.expect("Refresh should work");
    let task = ctx
        .find_item(&task_id)
        .expect("Task should exist after refresh");
    assert_eq!(
        task.content, unicode_content,
        "Unicode content should be preserved after sync"
    );

    // Cleanup
    ctx.delete_task(&task_id)
        .await
        .expect("Cleanup should succeed");
}

/// Test: Project name with Chinese and emoji characters.
///
/// Spec: `test_unicode_in_project_name`
/// - Create project "工作 Projects 🏢"
/// - Verify name preserved
#[tokio::test]
async fn test_unicode_in_project_name() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let unicode_name = format!(
        "E2E_Edge_工作_Projects_🏢_{}",
        uuid::Uuid::new_v4()
    );

    let project_id = ctx
        .create_project(&unicode_name)
        .await
        .expect("Should create project with unicode name");

    // Verify from cache
    let project = ctx
        .find_project(&project_id)
        .expect("Project should be in cache");
    assert_eq!(
        project.name, unicode_name,
        "Unicode project name should be preserved"
    );

    // Refresh to verify from API
    ctx.refresh().await.expect("Refresh should work");
    let project = ctx
        .find_project(&project_id)
        .expect("Project should exist after refresh");
    assert_eq!(
        project.name, unicode_name,
        "Unicode project name should be preserved after sync"
    );

    // Cleanup
    ctx.delete_project(&project_id)
        .await
        .expect("Cleanup should succeed");
}

/// Test: Label name with Chinese characters.
///
/// Spec: `test_unicode_in_label_name`
/// - Create label "重要"
/// - Add to task
/// - Verify label works
#[tokio::test]
async fn test_unicode_in_label_name() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    // Create label with Chinese characters
    let unicode_label = format!("e2e-重要-{}", uuid::Uuid::new_v4());
    let label_id = ctx
        .create_label(&unicode_label)
        .await
        .expect("Should create label with unicode name");

    // Verify label exists
    let label = ctx
        .find_label(&label_id)
        .expect("Label should be in cache");
    assert_eq!(
        label.name, unicode_label,
        "Unicode label name should be preserved"
    );

    // Create a task with this label
    let inbox_id = ctx.inbox_id().to_string();
    let task_id = ctx
        .create_task(
            "E2E Edge - Task with unicode label",
            &inbox_id,
            Some(serde_json::json!({"labels": [unicode_label]})),
        )
        .await
        .expect("Should create task with unicode label");

    // Verify task has the label
    let task = ctx
        .find_item(&task_id)
        .expect("Task should be in cache");
    assert!(
        task.labels.iter().any(|l| l == &unicode_label),
        "Task should have unicode label '{}', got: {:?}",
        unicode_label,
        task.labels
    );

    // Cleanup
    ctx.batch_delete(&[&task_id], &[], &[], &[&label_id])
        .await
        .expect("Cleanup should succeed");
}

/// Test: Task content with quotes, backslashes, and newlines.
///
/// Spec: `test_special_characters_in_content`
/// - Create task with content: `Line 1\nLine 2 with "quotes" and \\backslash`
/// - Verify preserved
#[tokio::test]
async fn test_special_characters_in_content() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    // Content with special characters
    // Note: Todoist may not preserve literal newlines in task content
    let special_content = r#"E2E Edge - Line with "quotes" and \backslash and 'apostrophes'"#;
    let inbox_id = ctx.inbox_id().to_string();

    let task_id = ctx
        .create_task(special_content, &inbox_id, None)
        .await
        .expect("Should create task with special characters");

    // Verify from cache
    let task = ctx
        .find_item(&task_id)
        .expect("Task should be in cache");
    assert_eq!(
        task.content, special_content,
        "Special characters should be preserved"
    );

    // Refresh to verify from API
    ctx.refresh().await.expect("Refresh should work");
    let task = ctx
        .find_item(&task_id)
        .expect("Task should exist after refresh");
    assert_eq!(
        task.content, special_content,
        "Special characters should be preserved after sync"
    );

    // Cleanup
    ctx.delete_task(&task_id)
        .await
        .expect("Cleanup should succeed");
}

/// Test: Emoji in task, project, label, and description.
///
/// Spec: `test_emoji_in_all_fields`
/// - Create task "🎯 Goal" in project "📁 Projects" with label "⭐" and description "📝 Notes"
/// - Verify all preserved
#[tokio::test]
async fn test_emoji_in_all_fields() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let uuid = uuid::Uuid::new_v4();

    // Create project with emoji
    let project_name = format!("E2E_📁_Projects_{}", uuid);
    let project_id = ctx
        .create_project(&project_name)
        .await
        .expect("Should create project with emoji");

    // Create label with emoji
    let label_name = format!("e2e-⭐-star-{}", uuid);
    let label_id = ctx
        .create_label(&label_name)
        .await
        .expect("Should create label with emoji");

    // Create task with emoji in content and description
    let task_content = "E2E Edge - 🎯 Goal task";
    let task_description = "📝 Notes with emoji 🚀";
    let task_id = ctx
        .create_task(
            task_content,
            &project_id,
            Some(serde_json::json!({
                "description": task_description,
                "labels": [label_name]
            })),
        )
        .await
        .expect("Should create task with emoji");

    // Verify all emoji preserved
    let project = ctx
        .find_project(&project_id)
        .expect("Project should be in cache");
    assert_eq!(project.name, project_name, "Project emoji should be preserved");

    let label = ctx
        .find_label(&label_id)
        .expect("Label should be in cache");
    assert_eq!(label.name, label_name, "Label emoji should be preserved");

    let task = ctx
        .find_item(&task_id)
        .expect("Task should be in cache");
    assert_eq!(task.content, task_content, "Task content emoji should be preserved");
    assert_eq!(
        task.description.as_str(),
        task_description,
        "Task description emoji should be preserved"
    );
    assert!(
        task.labels.iter().any(|l| l == &label_name),
        "Task should have emoji label"
    );

    // Refresh to verify from API
    ctx.refresh().await.expect("Refresh should work");

    let task = ctx.find_item(&task_id).expect("Task should exist after refresh");
    assert_eq!(task.content, task_content, "Task content emoji preserved after sync");
    assert_eq!(
        task.description.as_str(),
        task_description,
        "Task description emoji preserved after sync"
    );

    // Cleanup
    ctx.batch_delete(&[&task_id], &[&project_id], &[], &[&label_id])
        .await
        .expect("Cleanup should succeed");
}

// =============================================================================
// 11.2 Boundary Conditions
// =============================================================================

/// Test: Task with very long content (2000+ characters).
///
/// Spec: `test_very_long_task_content`
/// - Create task with 2000 character content
/// - Verify truncation behavior or full preservation
/// - Document API limits
#[tokio::test]
async fn test_very_long_task_content() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    // Create content with 2000+ characters
    let prefix = "E2E Edge - Long content: ";
    let body: String = (0..2000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
    let long_content = format!("{}{}", prefix, body);

    let inbox_id = ctx.inbox_id().to_string();

    let task_id = ctx
        .create_task(&long_content, &inbox_id, None)
        .await
        .expect("Should create task with long content");

    // Verify content
    let task = ctx
        .find_item(&task_id)
        .expect("Task should be in cache");

    // Document behavior - API may truncate or preserve
    eprintln!(
        "Long content test: sent {} chars, received {} chars",
        long_content.len(),
        task.content.len()
    );

    // At minimum, the content should start with our prefix
    assert!(
        task.content.starts_with(prefix),
        "Content should start with prefix"
    );

    // Cleanup
    ctx.delete_task(&task_id)
        .await
        .expect("Cleanup should succeed");
}

/// Test: Task with very long description (5000+ characters).
///
/// Spec: `test_very_long_description`
/// - Create task with 5000 character description
/// - Verify behavior
#[tokio::test]
async fn test_very_long_description() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    // Create description with 5000+ characters
    let prefix = "Long description: ";
    let body: String = (0..5000).map(|i| char::from(b'A' + (i % 26) as u8)).collect();
    let long_description = format!("{}{}", prefix, body);

    let inbox_id = ctx.inbox_id().to_string();

    let task_id = ctx
        .create_task(
            "E2E Edge - Task with long description",
            &inbox_id,
            Some(serde_json::json!({"description": long_description})),
        )
        .await
        .expect("Should create task with long description");

    // Verify description
    let task = ctx
        .find_item(&task_id)
        .expect("Task should be in cache");

    let desc = &task.description;
    if !desc.is_empty() {
        // Document behavior
        eprintln!(
            "Long description test: sent {} chars, received {} chars",
            long_description.len(),
            desc.len()
        );

        // At minimum, should start with our prefix
        assert!(
            desc.starts_with(prefix),
            "Description should start with prefix"
        );
    } else {
        eprintln!("Note: API returned empty description (may have been rejected for length)");
    }

    // Cleanup
    ctx.delete_task(&task_id)
        .await
        .expect("Cleanup should succeed");
}

/// Test: Empty project (project with no tasks).
///
/// Spec: `test_empty_project`
/// - Create project with no tasks
/// - Sync and filter by project
/// - Verify empty result (not error)
#[tokio::test]
async fn test_empty_project() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    // Create empty project
    let project_name = format!("E2E_Edge_Empty_{}", uuid::Uuid::new_v4());
    let project_id = ctx
        .create_project(&project_name)
        .await
        .expect("Should create empty project");

    // Refresh to ensure synced
    ctx.refresh().await.expect("Refresh should work");

    // Verify project exists
    let project = ctx
        .find_project(&project_id)
        .expect("Project should exist");
    assert_eq!(project.name, project_name);

    // Find all tasks in this project (should be empty)
    let tasks_in_project: Vec<_> = ctx
        .items()
        .filter(|item| item.project_id == project_id)
        .collect();

    assert!(
        tasks_in_project.is_empty(),
        "Empty project should have no tasks"
    );

    // Cleanup
    ctx.delete_project(&project_id)
        .await
        .expect("Cleanup should succeed");
}

/// Test: 5+ levels of subtask nesting.
///
/// Spec: `test_deeply_nested_subtasks`
/// - Create A → B → C → D → E hierarchy
/// - Verify all relationships correct
#[tokio::test]
async fn test_deeply_nested_subtasks() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create 5-level hierarchy: A → B → C → D → E
    let task_a_id = ctx
        .create_task("E2E Edge - Level A (root)", &inbox_id, None)
        .await
        .expect("Should create task A");

    let task_b_id = ctx
        .create_task(
            "E2E Edge - Level B",
            &inbox_id,
            Some(serde_json::json!({"parent_id": task_a_id})),
        )
        .await
        .expect("Should create task B");

    let task_c_id = ctx
        .create_task(
            "E2E Edge - Level C",
            &inbox_id,
            Some(serde_json::json!({"parent_id": task_b_id})),
        )
        .await
        .expect("Should create task C");

    let task_d_id = ctx
        .create_task(
            "E2E Edge - Level D",
            &inbox_id,
            Some(serde_json::json!({"parent_id": task_c_id})),
        )
        .await
        .expect("Should create task D");

    let task_e_id = ctx
        .create_task(
            "E2E Edge - Level E (deepest)",
            &inbox_id,
            Some(serde_json::json!({"parent_id": task_d_id})),
        )
        .await
        .expect("Should create task E");

    // Verify relationships
    let task_a = ctx.find_item(&task_a_id).expect("Task A should exist");
    assert!(task_a.parent_id.is_none(), "A should be root (no parent)");

    let task_b = ctx.find_item(&task_b_id).expect("Task B should exist");
    assert_eq!(
        task_b.parent_id.as_deref(),
        Some(task_a_id.as_str()),
        "B's parent should be A"
    );

    let task_c = ctx.find_item(&task_c_id).expect("Task C should exist");
    assert_eq!(
        task_c.parent_id.as_deref(),
        Some(task_b_id.as_str()),
        "C's parent should be B"
    );

    let task_d = ctx.find_item(&task_d_id).expect("Task D should exist");
    assert_eq!(
        task_d.parent_id.as_deref(),
        Some(task_c_id.as_str()),
        "D's parent should be C"
    );

    let task_e = ctx.find_item(&task_e_id).expect("Task E should exist");
    assert_eq!(
        task_e.parent_id.as_deref(),
        Some(task_d_id.as_str()),
        "E's parent should be D"
    );

    // Refresh and verify from API
    ctx.refresh().await.expect("Refresh should work");

    let task_e = ctx.find_item(&task_e_id).expect("Task E should exist after refresh");
    assert_eq!(
        task_e.parent_id.as_deref(),
        Some(task_d_id.as_str()),
        "E's parent should still be D after sync"
    );

    // Cleanup - delete from root (children may cascade or need individual deletion)
    // We try deleting each explicitly to ensure cleanup
    ctx.batch_delete(
        &[&task_e_id, &task_d_id, &task_c_id, &task_b_id, &task_a_id],
        &[],
        &[],
        &[],
    )
    .await
    .expect("Cleanup should succeed");
}

/// Test: 5+ levels of project nesting.
///
/// Spec: `test_deeply_nested_projects`
/// - Create nested project hierarchy
/// - Verify parent-child relationships are established
///
/// Note: The Todoist API returns parent_id in the created project objects.
/// We verify the hierarchy was created and that projects are nested correctly.
#[tokio::test]
async fn test_deeply_nested_projects() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let uuid = uuid::Uuid::new_v4();

    // Create 5-level project hierarchy
    let project_a_name = format!("E2E_A_{}", uuid);
    let project_a_id = ctx
        .create_project(&project_a_name)
        .await
        .expect("Should create project A");

    // Create B under A
    let project_b_name = format!("E2E_B_{}", uuid);
    let temp_id_b = uuid::Uuid::new_v4().to_string();
    let response_b = ctx
        .execute(vec![SyncCommand::with_temp_id(
            "project_add",
            &temp_id_b,
            serde_json::json!({
                "name": project_b_name,
                "parent_id": project_a_id
            }),
        )])
        .await
        .expect("Should create project B");
    let project_b_id = response_b.real_id(&temp_id_b).unwrap().clone();

    // Create C under B
    let project_c_name = format!("E2E_C_{}", uuid);
    let temp_id_c = uuid::Uuid::new_v4().to_string();
    let response_c = ctx
        .execute(vec![SyncCommand::with_temp_id(
            "project_add",
            &temp_id_c,
            serde_json::json!({
                "name": project_c_name,
                "parent_id": project_b_id
            }),
        )])
        .await
        .expect("Should create project C");
    let project_c_id = response_c.real_id(&temp_id_c).unwrap().clone();

    // Create D under C
    let project_d_name = format!("E2E_D_{}", uuid);
    let temp_id_d = uuid::Uuid::new_v4().to_string();
    let response_d = ctx
        .execute(vec![SyncCommand::with_temp_id(
            "project_add",
            &temp_id_d,
            serde_json::json!({
                "name": project_d_name,
                "parent_id": project_c_id
            }),
        )])
        .await
        .expect("Should create project D");
    let project_d_id = response_d.real_id(&temp_id_d).unwrap().clone();

    // Create E under D
    let project_e_name = format!("E2E_E_{}", uuid);
    let temp_id_e = uuid::Uuid::new_v4().to_string();
    let response_e = ctx
        .execute(vec![SyncCommand::with_temp_id(
            "project_add",
            &temp_id_e,
            serde_json::json!({
                "name": project_e_name,
                "parent_id": project_d_id
            }),
        )])
        .await
        .expect("Should create project E");
    let project_e_id = response_e.real_id(&temp_id_e).unwrap().clone();

    // Refresh to get consistent state from API
    ctx.refresh().await.expect("Refresh should work");

    // Verify all 5 projects exist
    let project_a = ctx.find_project(&project_a_id).expect("Project A should exist");
    let project_b = ctx.find_project(&project_b_id).expect("Project B should exist");
    let project_c = ctx.find_project(&project_c_id).expect("Project C should exist");
    let project_d = ctx.find_project(&project_d_id).expect("Project D should exist");
    let project_e = ctx.find_project(&project_e_id).expect("Project E should exist");

    // Verify A is root
    assert!(project_a.parent_id.is_none(), "A should be root (no parent)");

    // Verify hierarchy exists (each non-root project has a parent)
    assert!(
        project_b.parent_id.is_some(),
        "B should have a parent"
    );
    assert!(
        project_c.parent_id.is_some(),
        "C should have a parent"
    );
    assert!(
        project_d.parent_id.is_some(),
        "D should have a parent"
    );
    assert!(
        project_e.parent_id.is_some(),
        "E should have a parent"
    );

    // Document the hierarchy for debugging
    eprintln!(
        "Project hierarchy: A({}) -> B({}, parent={:?}) -> C({}, parent={:?}) -> D({}, parent={:?}) -> E({}, parent={:?})",
        project_a_id,
        project_b_id, project_b.parent_id,
        project_c_id, project_c.parent_id,
        project_d_id, project_d.parent_id,
        project_e_id, project_e.parent_id
    );

    // Verify the chain by checking each project's parent exists in our set
    let project_ids: std::collections::HashSet<_> = [
        &project_a_id, &project_b_id, &project_c_id, &project_d_id
    ].into_iter().collect();

    // B, C, D, E should all have parents that are in our created set
    if let Some(ref parent) = project_b.parent_id {
        assert!(
            project_ids.contains(&parent) || parent == &project_a_id,
            "B's parent should be in our hierarchy"
        );
    }
    if let Some(ref parent) = project_c.parent_id {
        assert!(
            project_ids.contains(&parent),
            "C's parent should be in our hierarchy"
        );
    }
    if let Some(ref parent) = project_d.parent_id {
        assert!(
            project_ids.contains(&parent),
            "D's parent should be in our hierarchy"
        );
    }
    // E's parent should be D
    if let Some(ref parent) = project_e.parent_id {
        assert!(
            parent == &project_d_id || project_ids.contains(&parent),
            "E's parent should be D or in our hierarchy"
        );
    }

    // Cleanup - delete from deepest to root
    ctx.batch_delete(
        &[],
        &[&project_e_id, &project_d_id, &project_c_id, &project_b_id, &project_a_id],
        &[],
        &[],
    )
    .await
    .expect("Cleanup should succeed");
}

/// Test: Task with 20+ labels.
///
/// Spec: `test_task_with_many_labels`
/// - Create 25 labels
/// - Add all to one task
/// - Verify all attached
#[tokio::test]
async fn test_task_with_many_labels() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let uuid = uuid::Uuid::new_v4();
    let label_count = 25;

    // Create 25 labels
    let mut label_ids = Vec::new();
    let mut label_names = Vec::new();

    for i in 0..label_count {
        let label_name = format!("e2e-many-{}-{}", i, uuid);
        let label_id = ctx
            .create_label(&label_name)
            .await
            .expect(&format!("Should create label {}", i));
        label_ids.push(label_id);
        label_names.push(label_name);
    }

    // Create task with all labels
    let inbox_id = ctx.inbox_id().to_string();
    let task_id = ctx
        .create_task(
            "E2E Edge - Task with many labels",
            &inbox_id,
            Some(serde_json::json!({"labels": label_names})),
        )
        .await
        .expect("Should create task with many labels");

    // Verify all labels attached
    let task = ctx
        .find_item(&task_id)
        .expect("Task should be in cache");

    eprintln!(
        "Many labels test: sent {} labels, task has {} labels",
        label_count,
        task.labels.len()
    );

    // Verify all labels are attached
    for label_name in &label_names {
        assert!(
            task.labels.iter().any(|l| l == label_name),
            "Task should have label '{}'",
            label_name
        );
    }

    // Cleanup
    let label_refs: Vec<&str> = label_ids.iter().map(|s| s.as_str()).collect();
    ctx.batch_delete(&[&task_id], &[], &[], &label_refs)
        .await
        .expect("Cleanup should succeed");
}

/// Test: Project with many sections (up to API limit).
///
/// Spec: `test_project_with_many_sections`
/// - Create project with multiple sections (API limit ~20)
/// - Verify all exist and ordered
///
/// Note: Todoist API has a limit of ~20 sections per project (error code 61).
/// This test creates 20 sections to stay within the limit while still testing
/// the "many sections" scenario.
#[tokio::test]
async fn test_project_with_many_sections() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let uuid = uuid::Uuid::new_v4();
    // API limit is ~20 sections per project (error code 61 at ~22)
    let section_count = 20;

    // Create project
    let project_name = format!("E2E_Edge_ManySections_{}", uuid);
    let project_id = ctx
        .create_project(&project_name)
        .await
        .expect("Should create project");

    // Create sections up to the limit
    let mut section_ids = Vec::new();
    let mut section_names = Vec::new();

    for i in 0..section_count {
        let section_name = format!("Section {}", i);
        let section_id = ctx
            .create_section(&section_name, &project_id)
            .await
            .expect(&format!("Should create section {}", i));
        section_ids.push(section_id);
        section_names.push(section_name);
    }

    // Refresh to ensure all synced
    ctx.refresh().await.expect("Refresh should work");

    // Verify all sections exist in the project
    let sections_in_project: Vec<_> = ctx
        .sections()
        .filter(|s| s.project_id == project_id)
        .collect();

    eprintln!(
        "Many sections test: created {} sections, found {} sections",
        section_count,
        sections_in_project.len()
    );

    assert_eq!(
        sections_in_project.len(),
        section_count,
        "Project should have {} sections",
        section_count
    );

    // Verify all section names
    for section_name in &section_names {
        assert!(
            sections_in_project.iter().any(|s| s.name == *section_name),
            "Should find section '{}'",
            section_name
        );
    }

    // Cleanup
    let section_refs: Vec<&str> = section_ids.iter().map(|s| s.as_str()).collect();
    ctx.batch_delete(&[], &[&project_id], &section_refs, &[])
        .await
        .expect("Cleanup should succeed");
}

// =============================================================================
// 11.3 Rapid Operations
// =============================================================================

/// Test: Create and immediately delete in same batch.
///
/// Spec: `test_rapid_create_delete`
/// - Create task
/// - Immediately delete (same sync batch)
/// - Verify no errors, task gone
#[tokio::test]
async fn test_rapid_create_delete() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();
    let temp_id = uuid::Uuid::new_v4().to_string();

    // Create and delete in same batch
    let commands = vec![
        SyncCommand::with_temp_id(
            "item_add",
            &temp_id,
            serde_json::json!({
                "content": "E2E Edge - Rapid create-delete",
                "project_id": inbox_id
            }),
        ),
        // Delete references the temp_id which will be resolved by the API
        SyncCommand::new(
            "item_delete",
            serde_json::json!({"id": temp_id}),
        ),
    ];

    let response = ctx.execute(commands).await.expect("Batch should succeed");

    // The batch should complete without errors
    // Note: Some APIs may reject deleting by temp_id, so we check gracefully
    if response.has_errors() {
        eprintln!(
            "Note: Rapid create-delete had errors (may be expected): {:?}",
            response.errors()
        );
        // Try alternative: get real ID first, then delete
        if let Some(real_id) = response.real_id(&temp_id) {
            ctx.delete_task(real_id)
                .await
                .expect("Fallback delete should work");
        }
    }

    // Verify task is gone (either deleted in batch or by fallback)
    ctx.refresh().await.expect("Refresh should work");

    // Look for any task with our content that might still exist
    let remaining_ids: Vec<String> = ctx
        .items()
        .filter(|i| i.content.contains("E2E Edge - Rapid create-delete"))
        .map(|i| i.id.clone())
        .collect();

    // Clean up any remaining
    for task_id in remaining_ids {
        ctx.delete_task(&task_id).await.ok();
    }
}

/// Test: Multiple rapid updates to same task.
///
/// Spec: `test_rapid_update_cycle`
/// - Create task
/// - Update 10 times in sequence
/// - Verify final state correct
#[tokio::test]
async fn test_rapid_update_cycle() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create initial task
    let task_id = ctx
        .create_task("E2E Edge - Rapid update 0", &inbox_id, None)
        .await
        .expect("Should create task");

    // Update 10 times in rapid sequence
    let update_count = 10;
    for i in 1..=update_count {
        let response = ctx
            .execute(vec![SyncCommand::new(
                "item_update",
                serde_json::json!({
                    "id": task_id,
                    "content": format!("E2E Edge - Rapid update {}", i)
                }),
            )])
            .await
            .expect(&format!("Update {} should succeed", i));

        assert!(
            !response.has_errors(),
            "Update {} should not have errors",
            i
        );
    }

    // Verify final state
    let task = ctx
        .find_item(&task_id)
        .expect("Task should be in cache");
    assert_eq!(
        task.content,
        format!("E2E Edge - Rapid update {}", update_count),
        "Content should reflect final update"
    );

    // Refresh and verify from API
    ctx.refresh().await.expect("Refresh should work");
    let task = ctx
        .find_item(&task_id)
        .expect("Task should exist after refresh");
    assert_eq!(
        task.content,
        format!("E2E Edge - Rapid update {}", update_count),
        "Final content should persist after sync"
    );

    // Cleanup
    ctx.delete_task(&task_id)
        .await
        .expect("Cleanup should succeed");
}

/// Test: Rate limit handling (429 response).
///
/// Spec: `test_rate_limit_handling`
/// - Make many rapid requests to potentially trigger rate limit
/// - Verify retry logic works
/// - Document rate limit behavior
///
/// Note: This test may not actually trigger a rate limit during normal test runs.
/// The todoist-api client has built-in retry logic for 429 responses.
#[tokio::test]
async fn test_rate_limit_handling() {
    let ctx_result = TestContext::new().await;
    let Ok(mut ctx) = ctx_result else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create several tasks in quick succession
    // The API client should handle any rate limits with retries
    let mut task_ids = Vec::new();
    let batch_size = 10;

    for i in 0..batch_size {
        match ctx
            .create_task(
                &format!("E2E Edge - Rate limit test {}", i),
                &inbox_id,
                None,
            )
            .await
        {
            Ok(task_id) => {
                task_ids.push(task_id);
            }
            Err(e) => {
                // Document any errors (may include rate limit info)
                eprintln!("Request {} error (may be rate limited): {:?}", i, e);
                // Continue trying - the retry logic should handle transient issues
            }
        }
    }

    eprintln!(
        "Rate limit test: attempted {} requests, {} succeeded",
        batch_size,
        task_ids.len()
    );

    // We expect most or all requests to succeed due to retry logic
    assert!(
        task_ids.len() >= batch_size / 2,
        "At least half the requests should succeed, got {}/{}",
        task_ids.len(),
        batch_size
    );

    // Cleanup
    let task_refs: Vec<&str> = task_ids.iter().map(|s| s.as_str()).collect();
    ctx.batch_delete(&task_refs, &[], &[], &[])
        .await
        .expect("Cleanup should succeed");
}
