//! End-to-end tests for the todoist-cache crate.
//!
//! These tests validate cache functionality with the real Todoist API.
//! They require a valid Todoist API token set in .env.local as:
//! TODOIST_TEST_API_TOKEN=<token>
//!
//! Run with: cargo test -p todoist-cache-rs --features e2e --test cache_e2e

#![cfg(feature = "e2e")]

use std::fs;
use tempfile::tempdir;

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{SyncCommand, SyncCommandType, SyncRequest};
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

/// Validates that a full sync populates the cache with real API data.
///
/// This test ensures that the API response deserialization works correctly
/// with real Todoist data structures.
#[tokio::test]
async fn test_e2e_full_sync_populates_cache() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token).unwrap();
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Cache should need full sync initially
    assert!(
        manager.cache().needs_full_sync(),
        "Fresh cache should need full sync"
    );
    assert!(
        manager.cache().last_sync.is_none(),
        "Fresh cache should have no last_sync"
    );

    // Perform full sync
    let cache = manager.sync().await.expect("sync failed");

    // Verify cache was populated
    assert!(
        !cache.needs_full_sync(),
        "Cache should no longer need full sync"
    );
    assert!(!cache.sync_token.is_empty(), "Should have sync token");
    assert_ne!(cache.sync_token, "*", "Sync token should not be '*'");
    assert!(cache.last_sync.is_some(), "Should have last_sync timestamp");
    assert!(
        cache.full_sync_date_utc.is_some(),
        "Should have full_sync_date_utc"
    );

    // Should have at least the inbox project
    assert!(
        cache.projects.iter().any(|p| p.inbox_project),
        "Should have inbox project"
    );

    println!(
        "E2E Full sync: {} projects, {} items, {} labels, {} sections",
        cache.projects.len(),
        cache.items.len(),
        cache.labels.len(),
        cache.sections.len()
    );

    // Verify cache was persisted to disk
    assert!(cache_path.exists(), "Cache file should exist");

    let store2 = CacheStore::with_path(cache_path);
    let loaded = store2.load().expect("failed to load persisted cache");
    assert_eq!(
        loaded.sync_token, cache.sync_token,
        "Persisted cache should have same sync_token"
    );
    assert_eq!(
        loaded.projects.len(),
        cache.projects.len(),
        "Persisted cache should have same projects"
    );
}

/// Consolidated test for external change detection scenarios.
///
/// This test validates that the SyncManager correctly detects changes made
/// outside of it (via direct API calls). It tests:
/// 1. Task created externally is picked up by sync
/// 2. Task updated externally is picked up by sync
/// 3. Task deleted externally is picked up by sync
/// 4. Bulk operations are picked up by sync
///
/// By consolidating these tests, we reduce from 4 full syncs to 1.
#[tokio::test]
async fn test_sync_detects_external_changes() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create the SyncManager and perform initial sync (ONE full sync for all scenarios)
    let client = TodoistClient::new(&token).unwrap();
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client.clone(), store).expect("failed to create manager");

    let cache = manager.sync().await.expect("initial sync failed");
    let initial_item_count = cache.items.len();

    // Get inbox ID
    let inbox = cache
        .projects
        .iter()
        .find(|p| p.inbox_project && !p.is_deleted)
        .expect("Should have inbox project");
    let inbox_id = inbox.id.clone();

    // Track all task IDs for cleanup
    let mut all_task_ids: Vec<String> = Vec::new();

    // =========================================================================
    // Scenario 1: External Creation
    // =========================================================================
    println!("\n=== Scenario 1: External Creation ===");

    let temp_id = uuid::Uuid::new_v4().to_string();
    let add_command = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id,
        serde_json::json!({
            "content": "E2E external creation test",
            "project_id": inbox_id
        }),
    );
    let add_response = client
        .sync(SyncRequest::with_commands(vec![add_command]))
        .await
        .expect("item_add failed");
    assert!(!add_response.has_errors(), "item_add should succeed");
    let created_task_id = add_response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone();
    all_task_ids.push(created_task_id.clone());
    println!("Created external task with id: {}", created_task_id);

    // The manager's cache should NOT contain the task yet
    assert!(
        manager
            .cache()
            .items
            .iter()
            .all(|i| i.id != created_task_id),
        "Task should NOT be in manager's cache before sync"
    );

    // Sync - should pick up the external creation
    let cache = manager.sync().await.expect("sync failed");
    assert!(
        cache
            .items
            .iter()
            .any(|i| i.id == created_task_id && !i.is_deleted),
        "Created task should be in cache after sync"
    );
    println!(
        "After sync: {} items (was {})",
        cache.items.len(),
        initial_item_count
    );

    // =========================================================================
    // Scenario 2: External Update
    // =========================================================================
    println!("\n=== Scenario 2: External Update ===");

    // Create a task through the manager
    let temp_id = uuid::Uuid::new_v4().to_string();
    let add_command = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id,
        serde_json::json!({
            "content": "Original content",
            "project_id": inbox_id
        }),
    );
    let response = manager
        .execute_commands(vec![add_command])
        .await
        .expect("item_add failed");
    let update_task_id = response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone();
    all_task_ids.push(update_task_id.clone());
    println!("Created task for update test with id: {}", update_task_id);

    // Verify original content
    let task = manager
        .cache()
        .items
        .iter()
        .find(|i| i.id == update_task_id)
        .expect("Task should be in cache");
    assert_eq!(task.content, "Original content");

    // Update the task via direct API call (external to manager)
    let update_command = SyncCommand::new(
        SyncCommandType::ItemUpdate,
        serde_json::json!({
            "id": update_task_id,
            "content": "Modified content"
        }),
    );
    let update_response = client
        .sync(SyncRequest::with_commands(vec![update_command]))
        .await
        .expect("external update failed");
    assert!(!update_response.has_errors(), "item_update should succeed");
    println!("Updated task externally");

    // The manager's cache should still have the old content
    let task_before_sync = manager
        .cache()
        .items
        .iter()
        .find(|i| i.id == update_task_id)
        .expect("Task should be in cache");
    assert_eq!(
        task_before_sync.content, "Original content",
        "Task should still have original content before sync"
    );

    // Sync - should pick up the external update
    let cache = manager.sync().await.expect("sync failed");
    let task_after_sync = cache
        .items
        .iter()
        .find(|i| i.id == update_task_id)
        .expect("Task should be in cache after sync");
    assert_eq!(
        task_after_sync.content, "Modified content",
        "Task should have modified content after sync"
    );
    println!("Task content updated to: {}", task_after_sync.content);

    // =========================================================================
    // Scenario 3: External Deletion
    // =========================================================================
    println!("\n=== Scenario 3: External Deletion ===");

    // Create a task through the manager
    let temp_id = uuid::Uuid::new_v4().to_string();
    let add_command = SyncCommand::with_temp_id(
        SyncCommandType::ItemAdd,
        &temp_id,
        serde_json::json!({
            "content": "E2E external deletion test",
            "project_id": inbox_id
        }),
    );
    let response = manager
        .execute_commands(vec![add_command])
        .await
        .expect("item_add failed");
    let delete_task_id = response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone();
    // Note: Don't add to all_task_ids since we're deleting it
    println!("Created task for deletion test with id: {}", delete_task_id);

    // Verify task is in cache
    assert!(
        manager
            .cache()
            .items
            .iter()
            .any(|i| i.id == delete_task_id && !i.is_deleted),
        "Task should be in cache after creation"
    );

    // Delete the task via direct API call (external to manager)
    let delete_command = SyncCommand::new(
        SyncCommandType::ItemDelete,
        serde_json::json!({"id": delete_task_id}),
    );
    let delete_response = client
        .sync(SyncRequest::with_commands(vec![delete_command]))
        .await
        .expect("external delete failed");
    assert!(!delete_response.has_errors(), "item_delete should succeed");
    println!("Deleted task externally");

    // The manager's cache should still have the task
    let item_in_cache = manager
        .cache()
        .items
        .iter()
        .find(|i| i.id == delete_task_id);
    assert!(
        item_in_cache.is_some(),
        "Task should still be in cache (may be marked deleted or not)"
    );

    // Sync - should pick up the external deletion
    let cache = manager.sync().await.expect("sync failed");
    let task_after_sync = cache.items.iter().find(|i| i.id == delete_task_id);
    match task_after_sync {
        Some(item) => {
            assert!(
                item.is_deleted,
                "Task should be marked as deleted after sync"
            );
            println!("Task marked as deleted in cache");
        }
        None => {
            println!("Task removed from cache entirely");
        }
    }

    // =========================================================================
    // Scenario 4: Bulk Operations
    // =========================================================================
    println!("\n=== Scenario 4: Bulk Operations ===");

    let current_item_count = manager
        .cache()
        .items
        .iter()
        .filter(|i| !i.is_deleted)
        .count();

    // Create 20 tasks in one batch via direct API call
    let batch_size = 20;
    let temp_ids: Vec<String> = (0..batch_size)
        .map(|_| uuid::Uuid::new_v4().to_string())
        .collect();

    let commands: Vec<SyncCommand> = temp_ids
        .iter()
        .enumerate()
        .map(|(i, temp_id)| {
            SyncCommand::with_temp_id(
                SyncCommandType::ItemAdd,
                temp_id,
                serde_json::json!({
                    "content": format!("E2E bulk task {}", i),
                    "project_id": inbox_id
                }),
            )
        })
        .collect();

    let add_response = client
        .sync(SyncRequest::with_commands(commands))
        .await
        .expect("bulk item_add failed");
    assert!(!add_response.has_errors(), "Bulk item_add should succeed");

    // Get real IDs
    let bulk_task_ids: Vec<String> = temp_ids
        .iter()
        .map(|tid| {
            add_response
                .real_id(tid)
                .expect("Should have mapping")
                .clone()
        })
        .collect();
    all_task_ids.extend(bulk_task_ids.clone());
    println!("Created {} tasks via bulk operation", bulk_task_ids.len());

    // Sync to pick up the bulk changes
    let cache = manager.sync().await.expect("sync failed");

    // Verify all bulk tasks are in cache
    let mut found_count = 0;
    for task_id in &bulk_task_ids {
        if cache
            .items
            .iter()
            .any(|i| i.id == *task_id && !i.is_deleted)
        {
            found_count += 1;
        }
    }
    assert_eq!(
        found_count, batch_size,
        "All {} bulk tasks should be in cache, found {}",
        batch_size, found_count
    );

    let new_item_count = cache.items.iter().filter(|i| !i.is_deleted).count();
    println!(
        "After bulk sync: {} items (was {}), {} new tasks found",
        new_item_count, current_item_count, found_count
    );

    // =========================================================================
    // Cleanup
    // =========================================================================
    println!("\n=== Cleanup ===");

    let delete_commands: Vec<SyncCommand> = all_task_ids
        .iter()
        .map(|id| SyncCommand::new(SyncCommandType::ItemDelete, serde_json::json!({"id": id})))
        .collect();

    if !delete_commands.is_empty() {
        client
            .sync(SyncRequest::with_commands(delete_commands))
            .await
            .expect("cleanup failed");
        println!("Cleaned up {} test tasks", all_task_ids.len());
    }

    println!("\n=== All external change detection scenarios passed ===");
}
