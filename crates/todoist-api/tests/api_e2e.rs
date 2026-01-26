//! End-to-end tests for the Todoist API client.
//!
//! These tests require a valid Todoist API token set in .env.local as:
//! TODOIST_TEST_API_TOKEN=<token>

use std::fs;
use todoist_api::client::TodoistClient;
use todoist_api::sync::{SyncCommand, SyncRequest};

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

/// Response wrapper for paginated list endpoints
#[derive(serde::Deserialize)]
struct ListResponse {
    results: Vec<serde_json::Value>,
}

#[tokio::test]
async fn test_get_projects() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // GET /projects should return a paginated response with results array
    let result: Result<ListResponse, _> = client.get("/projects").await;

    match result {
        Ok(response) => {
            println!("Got {} projects", response.results.len());
        }
        Err(e) => {
            panic!("Failed to get projects: {}", e);
        }
    }
}

#[tokio::test]
async fn test_get_tasks() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // GET /tasks should return a paginated response with results array
    let result: Result<ListResponse, _> = client.get("/tasks").await;

    match result {
        Ok(response) => {
            println!("Got {} tasks", response.results.len());
        }
        Err(e) => {
            panic!("Failed to get tasks: {}", e);
        }
    }
}

#[tokio::test]
async fn test_auth_failure() {
    let client = TodoistClient::new("invalid-token");

    let result: Result<Vec<serde_json::Value>, _> = client.get("/projects").await;

    assert!(result.is_err(), "Should fail with invalid token");

    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 2, "Auth failure should return exit code 2");
}

#[tokio::test]
async fn test_create_and_delete_task() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // Create a test task
    let body = serde_json::json!({
        "content": "Test task from e2e test",
        "description": "This task was created by an automated test"
    });

    let result: Result<serde_json::Value, _> = client.post("/tasks", &body).await;

    match result {
        Ok(task) => {
            let task_id = task["id"].as_str().expect("Task should have an id");
            println!("Created task with id: {}", task_id);

            // Delete the task we just created
            let delete_result = client.delete(&format!("/tasks/{}", task_id)).await;
            assert!(delete_result.is_ok(), "Should be able to delete the task");
            println!("Deleted task: {}", task_id);
        }
        Err(e) => {
            panic!("Failed to create task: {}", e);
        }
    }
}

// ============================================================================
// Sync API E2E Tests
// ============================================================================

#[tokio::test]
async fn test_sync_full_sync() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);
    let request = SyncRequest::full_sync();
    let response = client.sync(request).await;

    match response {
        Ok(sync_response) => {
            assert!(sync_response.full_sync, "Should be a full sync");
            assert!(!sync_response.sync_token.is_empty(), "Should have a sync token");
            println!(
                "Full sync: {} projects, {} items, {} labels",
                sync_response.projects.len(),
                sync_response.items.len(),
                sync_response.labels.len()
            );
            // Verify we have at least an inbox project
            assert!(
                sync_response.projects.iter().any(|p| p.inbox_project),
                "Should have an inbox project"
            );
        }
        Err(e) => {
            panic!("Failed to sync: {}", e);
        }
    }
}

#[tokio::test]
async fn test_sync_incremental() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // First, do a full sync to get a sync token
    let full_sync = client.sync(SyncRequest::full_sync()).await.unwrap();
    let sync_token = full_sync.sync_token.clone();
    println!("Got sync token: {}", sync_token);

    // Now do an incremental sync with that token
    let incremental = client
        .sync(SyncRequest::incremental(&sync_token))
        .await
        .unwrap();

    assert!(
        !incremental.full_sync,
        "Should be an incremental sync, not a full sync"
    );
    println!(
        "Incremental sync: {} items changed",
        incremental.items.len()
    );
}

#[tokio::test]
async fn test_sync_create_and_complete_item() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // First sync to get the inbox project ID
    let full_sync = client.sync(SyncRequest::full_sync()).await.unwrap();
    let inbox = full_sync
        .projects
        .iter()
        .find(|p| p.inbox_project)
        .expect("Should have inbox project");
    let inbox_id = inbox.id.clone();

    // Create an item via sync command
    let temp_id = uuid::Uuid::new_v4().to_string();
    let add_command = SyncCommand::with_temp_id(
        "item_add",
        &temp_id,
        serde_json::json!({
            "content": "E2E test item via sync",
            "project_id": inbox_id
        }),
    );

    let add_response = client
        .sync(SyncRequest::with_commands(vec![add_command]))
        .await
        .unwrap();

    assert!(!add_response.has_errors(), "item_add should succeed");
    let real_id = add_response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone();
    println!("Created item with temp_id {} -> real_id {}", temp_id, real_id);

    // Complete the item
    let close_command = SyncCommand::new("item_close", serde_json::json!({"id": real_id}));
    let close_response = client
        .sync(SyncRequest::with_commands(vec![close_command.clone()]))
        .await
        .unwrap();
    assert!(
        !close_response.has_errors(),
        "item_close should succeed: {:?}",
        close_response.errors()
    );
    println!("Completed item {}", real_id);

    // Delete the item to clean up
    let delete_command = SyncCommand::new("item_delete", serde_json::json!({"id": real_id}));
    let delete_response = client
        .sync(SyncRequest::with_commands(vec![delete_command]))
        .await
        .unwrap();
    assert!(
        !delete_response.has_errors(),
        "item_delete should succeed: {:?}",
        delete_response.errors()
    );
    println!("Deleted item for cleanup");
}

#[tokio::test]
async fn test_sync_specific_resource_types() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token);

    // Request only projects
    let request =
        SyncRequest::full_sync().with_resource_types(vec!["projects".to_string()]);
    let response = client.sync(request).await.unwrap();

    // Should have projects
    assert!(!response.projects.is_empty(), "Should have projects");
    println!("Got {} projects", response.projects.len());
}
