//! End-to-end tests for the Todoist API client.
//!
//! These tests require a valid Todoist API token set in .env.local as:
//! todoist_test_api_key=<token>

use std::fs;
use todoist_api::client::TodoistClient;

fn get_test_token() -> Option<String> {
    // Try to read from .env.local at workspace root
    for path in &[".env.local", "../../.env.local", "../../../.env.local"] {
        if let Ok(contents) = fs::read_to_string(path) {
            for line in contents.lines() {
                if let Some(token) = line.strip_prefix("todoist_test_api_key=") {
                    return Some(token.trim().to_string());
                }
            }
        }
    }

    // Fall back to environment variable
    std::env::var("TODOIST_TEST_API_KEY").ok()
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
