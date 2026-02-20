//! End-to-end tests for the Todoist API client.
//!
//! These tests require a valid Todoist API token set in .env.local as:
//! TODOIST_TEST_API_TOKEN=<token>
//!
//! Run with: cargo test -p todoist-api-rs --features e2e --test api_e2e

#![cfg(feature = "e2e")]

use std::fs;
use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{SyncCommand, SyncCommandType, SyncRequest};

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

#[tokio::test]
async fn test_sync_create_and_complete_item() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let client = TodoistClient::new(token).unwrap();

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
        SyncCommandType::ItemAdd,
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
    println!(
        "Created item with temp_id {} -> real_id {}",
        temp_id, real_id
    );

    // Complete the item
    let close_command = SyncCommand::new(
        SyncCommandType::ItemClose,
        serde_json::json!({"id": real_id}),
    );
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
    let delete_command = SyncCommand::new(
        SyncCommandType::ItemDelete,
        serde_json::json!({"id": real_id}),
    );
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
