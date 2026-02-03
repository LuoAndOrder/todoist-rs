//! End-to-end tests for the filter parser and evaluator.
//!
//! These tests validate filter parsing and evaluation against real Todoist data.
//! They require a valid Todoist API token set in .env.local as:
//! TODOIST_TEST_API_TOKEN=<token>
//!
//! Run with: cargo test --package todoist-cache --features e2e --test filter_e2e

#![cfg(feature = "e2e")]

use std::fs;

use chrono::Local;
use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{SyncCommand, SyncCommandType, SyncRequest};
use todoist_cache_rs::filter::{FilterContext, FilterEvaluator, FilterParser};
use todoist_cache_rs::{CacheStore, SyncManager};

fn get_test_token() -> Option<String> {
    // Try to read from .env.local at workspace root
    for path in &[".env.local", "../../.env.local", "../../../.env.local"] {
        if let Ok(contents) = fs::read_to_string(path) {
            for line in contents.lines() {
                // Support both formats for backwards compatibility
                if let Some(token) = line
                    .strip_prefix("TODOIST_TEST_API_TOKEN=")
                    .or_else(|| line.strip_prefix("todoist_test_api_key="))
                {
                    return Some(token.trim().to_string());
                }
            }
        }
    }

    // Fall back to environment variable
    std::env::var("TODOIST_TEST_API_TOKEN")
        .or_else(|_| std::env::var("TODOIST_TEST_API_KEY"))
        .ok()
}

fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

// ============================================================================
// Filter "today" E2E Tests
// ============================================================================

#[tokio::test]
async fn test_e2e_filter_today_returns_correct_items() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token).unwrap();
    let store = CacheStore::with_path(cache_path);
    let mut manager = SyncManager::new(client.clone(), store).expect("failed to create manager");

    // Perform initial sync to get inbox project
    let cache = manager.sync().await.expect("initial sync failed");
    let inbox = cache
        .projects
        .iter()
        .find(|p| p.inbox_project)
        .expect("Should have inbox project");
    let inbox_id = inbox.id.clone();

    // Create a task due today
    let today = today_str();
    let temp_id_today = uuid::Uuid::new_v4().to_string();
    let add_today = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id_today,
        serde_json::json!({
            "content": "E2E filter test - due today",
            "project_id": inbox_id,
            "due": { "date": today }
        }),
    );

    // Create a task with no due date
    let temp_id_nodate = uuid::Uuid::new_v4().to_string();
    let add_nodate = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id_nodate,
        serde_json::json!({
            "content": "E2E filter test - no due date",
            "project_id": inbox_id
        }),
    );

    let add_response = client
        .sync(SyncRequest::with_commands(vec![add_today, add_nodate]))
        .await
        .expect("item_add failed");

    assert!(!add_response.has_errors(), "item_add should succeed");
    let today_item_id = add_response
        .real_id(&temp_id_today)
        .expect("Should have temp_id mapping for today item")
        .clone();
    let nodate_item_id = add_response
        .real_id(&temp_id_nodate)
        .expect("Should have temp_id mapping for nodate item")
        .clone();
    println!(
        "Created items: today={}, nodate={}",
        today_item_id, nodate_item_id
    );

    // Sync to get the new items in cache
    let cache = manager.sync().await.expect("sync failed");

    // Parse and evaluate "today" filter
    let filter = FilterParser::parse("today").expect("Failed to parse 'today' filter");
    let context = FilterContext::new(&cache.projects, &cache.sections, &cache.labels);
    let evaluator = FilterEvaluator::new(&filter, &context);

    let matching_items = evaluator.filter_items(&cache.items);

    // The "today" task should match
    let today_matches = matching_items.iter().any(|i| i.id == today_item_id);
    assert!(
        today_matches,
        "Filter 'today' should match item due today (id={})",
        today_item_id
    );

    // The "no due date" task should NOT match
    let nodate_matches = matching_items.iter().any(|i| i.id == nodate_item_id);
    assert!(
        !nodate_matches,
        "Filter 'today' should NOT match item with no due date (id={})",
        nodate_item_id
    );

    println!(
        "E2E filter 'today': {} items matched, expected item due today included: {}",
        matching_items.len(),
        today_matches
    );

    // Clean up: delete the test items
    let delete_commands = vec![
        SyncCommand::new(
            SyncCommandType::ItemDelete,
            serde_json::json!({"id": today_item_id}),
        ),
        SyncCommand::new(
            SyncCommandType::ItemDelete,
            serde_json::json!({"id": nodate_item_id}),
        ),
    ];
    let delete_response = client
        .sync(SyncRequest::with_commands(delete_commands))
        .await
        .expect("item_delete failed");
    assert!(
        !delete_response.has_errors(),
        "item_delete should succeed: {:?}",
        delete_response.errors()
    );
    println!("Cleaned up test items");
}

// ============================================================================
// Filter "p1 & @urgent" E2E Tests
// ============================================================================

#[tokio::test]
async fn test_e2e_filter_priority_and_label_intersection() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token).unwrap();
    let store = CacheStore::with_path(cache_path);
    let mut manager = SyncManager::new(client.clone(), store).expect("failed to create manager");

    // Perform initial sync to get inbox project
    let cache = manager.sync().await.expect("initial sync failed");
    let inbox = cache
        .projects
        .iter()
        .find(|p| p.inbox_project)
        .expect("Should have inbox project");
    let inbox_id = inbox.id.clone();

    // Create a label "urgent" if it doesn't exist
    let urgent_label_exists = cache
        .labels
        .iter()
        .any(|l| l.name.to_lowercase() == "urgent");
    let label_temp_id = uuid::Uuid::new_v4().to_string();
    if !urgent_label_exists {
        let add_label = SyncCommand::with_temp_id(
            SyncCommandType::LabelAdd,
            &label_temp_id,
            serde_json::json!({
                "name": "urgent"
            }),
        );
        let label_response = client
            .sync(SyncRequest::with_commands(vec![add_label]))
            .await
            .expect("label_add failed");
        assert!(
            !label_response.has_errors(),
            "label_add should succeed: {:?}",
            label_response.errors()
        );
        println!("Created 'urgent' label");
    }

    // Create test items:
    // 1. p1 + @urgent (should match)
    // 2. p1 only (should NOT match)
    // 3. @urgent only (should NOT match)
    // 4. p2 + @urgent (should NOT match)

    let temp_id_p1_urgent = uuid::Uuid::new_v4().to_string();
    let add_p1_urgent = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id_p1_urgent,
        serde_json::json!({
            "content": "E2E filter test - p1 with urgent label",
            "project_id": inbox_id,
            "priority": 4,  // p1 in API (inverted)
            "labels": ["urgent"]
        }),
    );

    let temp_id_p1_only = uuid::Uuid::new_v4().to_string();
    let add_p1_only = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id_p1_only,
        serde_json::json!({
            "content": "E2E filter test - p1 only",
            "project_id": inbox_id,
            "priority": 4  // p1 in API
        }),
    );

    let temp_id_urgent_only = uuid::Uuid::new_v4().to_string();
    let add_urgent_only = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id_urgent_only,
        serde_json::json!({
            "content": "E2E filter test - urgent only",
            "project_id": inbox_id,
            "labels": ["urgent"]
        }),
    );

    let temp_id_p2_urgent = uuid::Uuid::new_v4().to_string();
    let add_p2_urgent = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id_p2_urgent,
        serde_json::json!({
            "content": "E2E filter test - p2 with urgent",
            "project_id": inbox_id,
            "priority": 3,  // p2 in API
            "labels": ["urgent"]
        }),
    );

    let add_response = client
        .sync(SyncRequest::with_commands(vec![
            add_p1_urgent,
            add_p1_only,
            add_urgent_only,
            add_p2_urgent,
        ]))
        .await
        .expect("item_add failed");

    assert!(!add_response.has_errors(), "item_add should succeed");

    let p1_urgent_id = add_response
        .real_id(&temp_id_p1_urgent)
        .expect("temp_id mapping")
        .clone();
    let p1_only_id = add_response
        .real_id(&temp_id_p1_only)
        .expect("temp_id mapping")
        .clone();
    let urgent_only_id = add_response
        .real_id(&temp_id_urgent_only)
        .expect("temp_id mapping")
        .clone();
    let p2_urgent_id = add_response
        .real_id(&temp_id_p2_urgent)
        .expect("temp_id mapping")
        .clone();

    println!(
        "Created items: p1_urgent={}, p1_only={}, urgent_only={}, p2_urgent={}",
        p1_urgent_id, p1_only_id, urgent_only_id, p2_urgent_id
    );

    // Sync to get the new items in cache
    let cache = manager.sync().await.expect("sync failed");

    // Parse and evaluate "p1 & @urgent" filter
    let filter =
        FilterParser::parse("p1 & @urgent").expect("Failed to parse 'p1 & @urgent' filter");
    let context = FilterContext::new(&cache.projects, &cache.sections, &cache.labels);
    let evaluator = FilterEvaluator::new(&filter, &context);

    let matching_items = evaluator.filter_items(&cache.items);

    // Only p1 + urgent should match
    assert!(
        matching_items.iter().any(|i| i.id == p1_urgent_id),
        "Filter 'p1 & @urgent' should match p1 item with urgent label"
    );
    assert!(
        !matching_items.iter().any(|i| i.id == p1_only_id),
        "Filter 'p1 & @urgent' should NOT match p1-only item"
    );
    assert!(
        !matching_items.iter().any(|i| i.id == urgent_only_id),
        "Filter 'p1 & @urgent' should NOT match urgent-only item"
    );
    assert!(
        !matching_items.iter().any(|i| i.id == p2_urgent_id),
        "Filter 'p1 & @urgent' should NOT match p2 item with urgent label"
    );

    println!(
        "E2E filter 'p1 & @urgent': {} items matched",
        matching_items.len()
    );

    // Clean up: delete the test items
    let mut delete_commands = vec![
        SyncCommand::new(
            SyncCommandType::ItemDelete,
            serde_json::json!({"id": p1_urgent_id}),
        ),
        SyncCommand::new(
            SyncCommandType::ItemDelete,
            serde_json::json!({"id": p1_only_id}),
        ),
        SyncCommand::new(
            SyncCommandType::ItemDelete,
            serde_json::json!({"id": urgent_only_id}),
        ),
        SyncCommand::new(
            SyncCommandType::ItemDelete,
            serde_json::json!({"id": p2_urgent_id}),
        ),
    ];

    // If we created the label, delete it too
    if !urgent_label_exists {
        if let Some(real_label_id) = add_response.real_id(&label_temp_id) {
            delete_commands.push(SyncCommand::new(
                SyncCommandType::LabelDelete,
                serde_json::json!({"id": real_label_id}),
            ));
        }
    }

    let delete_response = client
        .sync(SyncRequest::with_commands(delete_commands))
        .await
        .expect("delete failed");
    assert!(
        !delete_response.has_errors(),
        "delete should succeed: {:?}",
        delete_response.errors()
    );
    println!("Cleaned up test items and label");
}

// ============================================================================
// Filter "#ProjectName" E2E Tests
// ============================================================================

#[tokio::test]
async fn test_e2e_filter_project_matches() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token).unwrap();
    let store = CacheStore::with_path(cache_path);
    let mut manager = SyncManager::new(client.clone(), store).expect("failed to create manager");

    // Perform initial sync
    let cache = manager.sync().await.expect("initial sync failed");
    let inbox = cache
        .projects
        .iter()
        .find(|p| p.inbox_project)
        .expect("Should have inbox project");
    let inbox_id = inbox.id.clone();

    // Create a test project
    let project_name = format!("E2E_FilterTest_{}", uuid::Uuid::new_v4());
    let project_temp_id = uuid::Uuid::new_v4().to_string();
    let add_project = SyncCommand::with_temp_id(
        SyncCommandType::ProjectAdd,
        &project_temp_id,
        serde_json::json!({
            "name": project_name
        }),
    );

    let project_response = client
        .sync(SyncRequest::with_commands(vec![add_project]))
        .await
        .expect("project_add failed");
    assert!(
        !project_response.has_errors(),
        "project_add should succeed: {:?}",
        project_response.errors()
    );
    let project_id = project_response
        .real_id(&project_temp_id)
        .expect("Should have project temp_id mapping")
        .clone();
    println!(
        "Created test project: name={}, id={}",
        project_name, project_id
    );

    // Create items:
    // 1. Item in the test project (should match)
    // 2. Item in inbox (should NOT match)

    let temp_id_in_project = uuid::Uuid::new_v4().to_string();
    let add_in_project = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id_in_project,
        serde_json::json!({
            "content": "E2E filter test - in test project",
            "project_id": project_id
        }),
    );

    let temp_id_in_inbox = uuid::Uuid::new_v4().to_string();
    let add_in_inbox = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id_in_inbox,
        serde_json::json!({
            "content": "E2E filter test - in inbox",
            "project_id": inbox_id
        }),
    );

    let add_response = client
        .sync(SyncRequest::with_commands(vec![
            add_in_project,
            add_in_inbox,
        ]))
        .await
        .expect("item_add failed");

    assert!(!add_response.has_errors(), "item_add should succeed");

    let in_project_id = add_response
        .real_id(&temp_id_in_project)
        .expect("temp_id mapping")
        .clone();
    let in_inbox_id = add_response
        .real_id(&temp_id_in_inbox)
        .expect("temp_id mapping")
        .clone();

    println!(
        "Created items: in_project={}, in_inbox={}",
        in_project_id, in_inbox_id
    );

    // Sync to get the new items and project in cache
    let cache = manager.sync().await.expect("sync failed");

    // Parse and evaluate "#ProjectName" filter
    let filter_query = format!("#{}", project_name);
    let filter = FilterParser::parse(&filter_query).expect("Failed to parse project filter");
    let context = FilterContext::new(&cache.projects, &cache.sections, &cache.labels);
    let evaluator = FilterEvaluator::new(&filter, &context);

    let matching_items = evaluator.filter_items(&cache.items);

    // Only the item in the test project should match
    assert!(
        matching_items.iter().any(|i| i.id == in_project_id),
        "Filter '{}' should match item in test project",
        filter_query
    );
    assert!(
        !matching_items.iter().any(|i| i.id == in_inbox_id),
        "Filter '{}' should NOT match item in inbox",
        filter_query
    );

    println!(
        "E2E filter '{}': {} items matched",
        filter_query,
        matching_items.len()
    );

    // Clean up: delete the test items and project
    let delete_commands = vec![
        SyncCommand::new(
            SyncCommandType::ItemDelete,
            serde_json::json!({"id": in_project_id}),
        ),
        SyncCommand::new(
            SyncCommandType::ItemDelete,
            serde_json::json!({"id": in_inbox_id}),
        ),
        SyncCommand::new(
            SyncCommandType::ProjectDelete,
            serde_json::json!({"id": project_id}),
        ),
    ];

    let delete_response = client
        .sync(SyncRequest::with_commands(delete_commands))
        .await
        .expect("delete failed");
    assert!(
        !delete_response.has_errors(),
        "delete should succeed: {:?}",
        delete_response.errors()
    );
    println!("Cleaned up test items and project");
}
