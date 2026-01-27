//! End-to-end tests for the todoist-cache crate.
//!
//! These tests validate cache functionality with the real Todoist API.
//! They require a valid Todoist API token set in .env.local as:
//! TODOIST_TEST_API_TOKEN=<token>
//!
//! Run with: cargo test --package todoist-cache --features e2e --test cache_e2e

#![cfg(feature = "e2e")]

use std::fs;
use tempfile::tempdir;

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{SyncCommand, SyncRequest};
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

// ============================================================================
// Full Sync E2E Tests
// ============================================================================

#[tokio::test]
async fn test_e2e_full_sync_populates_cache() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token);
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Cache should need full sync initially
    assert!(manager.cache().needs_full_sync(), "Fresh cache should need full sync");
    assert!(manager.cache().last_sync.is_none(), "Fresh cache should have no last_sync");

    // Perform full sync
    let cache = manager.sync().await.expect("sync failed");

    // Verify cache was populated
    assert!(!cache.needs_full_sync(), "Cache should no longer need full sync");
    assert!(!cache.sync_token.is_empty(), "Should have sync token");
    assert_ne!(cache.sync_token, "*", "Sync token should not be '*'");
    assert!(cache.last_sync.is_some(), "Should have last_sync timestamp");
    assert!(cache.full_sync_date_utc.is_some(), "Should have full_sync_date_utc");

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
    assert_eq!(loaded.sync_token, cache.sync_token, "Persisted cache should have same sync_token");
    assert_eq!(
        loaded.projects.len(),
        cache.projects.len(),
        "Persisted cache should have same projects"
    );
}

// ============================================================================
// Incremental Sync E2E Tests
// ============================================================================

#[tokio::test]
async fn test_e2e_incremental_sync_updates_cache() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token);
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client.clone(), store).expect("failed to create manager");

    // Perform initial full sync
    let cache = manager.sync().await.expect("initial sync failed");
    let initial_sync_token = cache.sync_token.clone();
    let initial_item_count = cache.items.len();
    println!(
        "Initial sync: {} items, token: {}",
        initial_item_count, initial_sync_token
    );

    // Get the inbox project ID
    let inbox = cache
        .projects
        .iter()
        .find(|p| p.inbox_project)
        .expect("Should have inbox project");
    let inbox_id = inbox.id.clone();

    // Create a new item via API
    let temp_id = uuid::Uuid::new_v4().to_string();
    let add_command = SyncCommand::with_temp_id(
        "item_add",
        &temp_id,
        serde_json::json!({
            "content": "E2E cache test item",
            "project_id": inbox_id
        }),
    );

    let add_response = client
        .sync(SyncRequest::with_commands(vec![add_command]))
        .await
        .expect("item_add failed");

    assert!(!add_response.has_errors(), "item_add should succeed");
    let real_id = add_response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone();
    println!("Created item with id: {}", real_id);

    // Now perform incremental sync - should pick up the new item
    let cache = manager.sync().await.expect("incremental sync failed");

    assert_ne!(
        cache.sync_token, initial_sync_token,
        "Sync token should have changed"
    );
    assert!(
        cache.items.iter().any(|i| i.id == real_id),
        "Cache should contain new item after incremental sync"
    );
    println!(
        "After incremental sync: {} items (was {})",
        cache.items.len(),
        initial_item_count
    );

    // Clean up: delete the item
    let delete_command = SyncCommand::new("item_delete", serde_json::json!({"id": real_id}));
    let delete_response = client
        .sync(SyncRequest::with_commands(vec![delete_command]))
        .await
        .expect("item_delete failed");
    assert!(
        !delete_response.has_errors(),
        "item_delete should succeed: {:?}",
        delete_response.errors()
    );
    println!("Cleaned up test item");
}

// ============================================================================
// Cache Persistence E2E Tests
// ============================================================================

#[tokio::test]
async fn test_e2e_cache_survives_restart() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create first manager and sync
    {
        let client = TodoistClient::new(&token);
        let store = CacheStore::with_path(cache_path.clone());
        let mut manager = SyncManager::new(client, store).expect("failed to create manager");

        let cache = manager.sync().await.expect("sync failed");
        println!(
            "First session: synced {} projects, {} items",
            cache.projects.len(),
            cache.items.len()
        );
    }

    // Verify cache file exists
    assert!(cache_path.exists(), "Cache file should exist after sync");

    // Create new manager (simulating restart) - should load existing cache
    {
        let client = TodoistClient::new(&token);
        let store = CacheStore::with_path(cache_path.clone());
        let manager = SyncManager::new(client, store).expect("failed to create manager after restart");

        // Should NOT need full sync - existing cache should be loaded
        assert!(
            !manager.cache().needs_full_sync(),
            "Restored cache should not need full sync"
        );
        assert!(
            manager.cache().last_sync.is_some(),
            "Restored cache should have last_sync"
        );
        assert!(
            !manager.cache().projects.is_empty(),
            "Restored cache should have projects"
        );

        println!(
            "Second session: loaded {} projects, {} items from disk",
            manager.cache().projects.len(),
            manager.cache().items.len()
        );

        // Should have the inbox project
        assert!(
            manager.cache().projects.iter().any(|p| p.inbox_project),
            "Restored cache should have inbox project"
        );
    }
}

#[tokio::test]
async fn test_e2e_cache_persistence_across_syncs() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token);
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client.clone(), store).expect("failed to create manager");

    // Perform initial sync
    manager.sync().await.expect("initial sync failed");
    let sync_token_after_first = manager.cache().sync_token.clone();

    // Get inbox ID for creating a test item
    let inbox = manager
        .cache()
        .projects
        .iter()
        .find(|p| p.inbox_project)
        .expect("Should have inbox project");
    let inbox_id = inbox.id.clone();

    // Create an item directly via API
    let temp_id = uuid::Uuid::new_v4().to_string();
    let add_command = SyncCommand::with_temp_id(
        "item_add",
        &temp_id,
        serde_json::json!({
            "content": "Persistence test item",
            "project_id": inbox_id
        }),
    );
    let add_response = client
        .sync(SyncRequest::with_commands(vec![add_command]))
        .await
        .expect("item_add failed");
    let real_id = add_response.real_id(&temp_id).expect("Should have mapping").clone();

    // Perform incremental sync
    manager.sync().await.expect("incremental sync failed");

    // Verify the item is in the cache
    assert!(
        manager.cache().items.iter().any(|i| i.id == real_id),
        "Item should be in cache after sync"
    );

    // Drop manager, create new one from same cache file
    drop(manager);

    let store2 = CacheStore::with_path(cache_path);
    let manager2 = SyncManager::new(client.clone(), store2).expect("failed to create manager");

    // The restored cache should still have the item
    assert!(
        manager2.cache().items.iter().any(|i| i.id == real_id),
        "Item should persist in restored cache"
    );

    // Sync token should have changed from first sync
    assert_ne!(
        manager2.cache().sync_token, sync_token_after_first,
        "Sync token should have changed"
    );

    // Clean up
    let delete_command = SyncCommand::new("item_delete", serde_json::json!({"id": real_id}));
    client
        .sync(SyncRequest::with_commands(vec![delete_command]))
        .await
        .expect("cleanup failed");
}

// ============================================================================
// Staleness E2E Tests
// ============================================================================

#[tokio::test]
async fn test_e2e_stale_cache_triggers_refresh() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create a pre-populated cache with an old last_sync timestamp
    let client = TodoistClient::new(&token);

    // First, do a real sync to get valid data
    {
        let store = CacheStore::with_path(cache_path.clone());
        let mut manager = SyncManager::new(client.clone(), store).expect("failed to create manager");
        manager.sync().await.expect("initial sync failed");
    }

    // Now modify the cache to have an old last_sync time
    let store = CacheStore::with_path(cache_path.clone());
    let mut cache = store.load().expect("failed to load cache");
    let original_sync_token = cache.sync_token.clone();

    // Set last_sync to 10 minutes ago (beyond default 5-minute threshold)
    cache.last_sync = Some(chrono::Utc::now() - chrono::Duration::minutes(10));
    store.save(&cache).expect("failed to save modified cache");

    // Create new manager with 1-second stale threshold for fast testing
    let store2 = CacheStore::with_path(cache_path);
    let mut manager = SyncManager::with_stale_threshold(client.clone(), store2, 1)
        .expect("failed to create manager");

    // Cache should be stale
    let now = chrono::Utc::now();
    assert!(
        manager.is_stale(now),
        "Cache should be stale (last_sync was 10 minutes ago, threshold is 1 minute)"
    );
    assert!(
        manager.needs_sync(now),
        "Manager should indicate sync is needed"
    );

    // Sync should fetch updates
    manager.sync().await.expect("sync failed");

    // After sync, should no longer be stale
    assert!(
        !manager.is_stale(chrono::Utc::now()),
        "Cache should no longer be stale after sync"
    );

    // Verify last_sync was updated
    assert!(
        manager.cache().last_sync.is_some(),
        "last_sync should be set"
    );

    // The sync may or may not get new data depending on server state,
    // but the token might be the same or different. What matters is
    // the cache is no longer stale.
    println!(
        "After stale refresh: sync_token={} (was {})",
        manager.cache().sync_token, original_sync_token
    );
}

#[tokio::test]
async fn test_e2e_fresh_cache_does_not_trigger_full_sync() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token);
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Perform initial full sync
    let cache = manager.sync().await.expect("initial sync failed");
    let sync_token_after_full = cache.sync_token.clone();
    println!("After full sync: token = {}", sync_token_after_full);

    // Cache should be fresh
    let now = chrono::Utc::now();
    assert!(
        !manager.is_stale(now),
        "Cache should be fresh immediately after sync"
    );
    assert!(
        !manager.needs_sync(now),
        "Manager should not indicate sync is needed for fresh cache"
    );

    // Perform another sync - should be incremental, not full
    let cache = manager.sync().await.expect("second sync failed");

    // The sync token may or may not change depending on whether there were changes
    // on the server, but the key point is we didn't need to do a full sync
    assert!(
        !cache.needs_full_sync(),
        "Should still not need full sync"
    );

    println!(
        "After second sync: token = {} (was {})",
        cache.sync_token, sync_token_after_full
    );
}

// ============================================================================
// Full Sync Force E2E Tests
// ============================================================================

#[tokio::test]
async fn test_e2e_force_full_sync() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token);
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Perform initial sync
    let cache = manager.sync().await.expect("initial sync failed");
    let initial_token = cache.sync_token.clone();
    assert!(!cache.needs_full_sync(), "Should have valid sync token");
    println!("Initial sync complete: token = {}", initial_token);

    // Force a full sync
    let cache = manager.full_sync().await.expect("full sync failed");

    // Should have a new sync token and full_sync_date_utc should be updated
    assert!(!cache.needs_full_sync(), "Should still have valid sync token");
    assert!(cache.full_sync_date_utc.is_some(), "Should have full_sync_date_utc");

    // Token may or may not be different, but full_sync_date_utc should be recent
    let full_sync_date = cache.full_sync_date_utc.unwrap();
    let now = chrono::Utc::now();
    let age = now.signed_duration_since(full_sync_date);
    assert!(
        age.num_seconds() < 60,
        "full_sync_date_utc should be recent (within 60 seconds), was {} seconds ago",
        age.num_seconds()
    );

    println!(
        "After forced full sync: token = {}, full_sync_date = {}",
        cache.sync_token, full_sync_date
    );
}

// ============================================================================
// Sync Behavior E2E Tests (Spec Section 10)
// ============================================================================

/// Test that sync picks up tasks created externally (outside of the cache manager).
///
/// This test validates that when a task is created via direct API call (not through
/// the SyncManager), a subsequent sync will detect and include the new task.
#[tokio::test]
async fn test_sync_picks_up_task_created_externally() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create the SyncManager and perform initial sync
    let client = TodoistClient::new(&token);
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

    // Create a task via direct API call (external to manager)
    let temp_id = uuid::Uuid::new_v4().to_string();
    let add_command = SyncCommand::with_temp_id(
        "item_add",
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
    let task_id = add_response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone();
    println!("Created external task with id: {}", task_id);

    // The manager's cache should NOT contain the task yet (it was created externally)
    assert!(
        manager.cache().items.iter().all(|i| i.id != task_id),
        "Task should NOT be in manager's cache before sync"
    );

    // Now sync - should pick up the external change
    let cache = manager.sync().await.expect("sync failed");

    // Task should now be in cache
    assert!(
        cache.items.iter().any(|i| i.id == task_id && !i.is_deleted),
        "Task should be in cache after sync"
    );
    println!(
        "After sync: {} items (was {})",
        cache.items.len(),
        initial_item_count
    );

    // Clean up
    let delete_command = SyncCommand::new("item_delete", serde_json::json!({"id": task_id}));
    client
        .sync(SyncRequest::with_commands(vec![delete_command]))
        .await
        .expect("cleanup failed");
    println!("Cleaned up test task");
}

/// Test that sync picks up tasks deleted externally (outside of the cache manager).
///
/// This test validates that when a task is deleted via direct API call (not through
/// the SyncManager), a subsequent sync will detect and remove the task from cache.
#[tokio::test]
async fn test_sync_picks_up_task_deleted_externally() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token);
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client.clone(), store).expect("failed to create manager");

    // Initial sync
    let cache = manager.sync().await.expect("initial sync failed");

    // Get inbox ID
    let inbox = cache
        .projects
        .iter()
        .find(|p| p.inbox_project && !p.is_deleted)
        .expect("Should have inbox project");
    let inbox_id = inbox.id.clone();

    // Create a task through the manager so it's in our cache
    let temp_id = uuid::Uuid::new_v4().to_string();
    let add_command = SyncCommand::with_temp_id(
        "item_add",
        &temp_id,
        serde_json::json!({
            "content": "E2E external deletion test",
            "project_id": inbox_id
        }),
    );
    let commands = vec![add_command];
    let response = manager
        .execute_commands(commands)
        .await
        .expect("item_add failed");
    let task_id = response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone();
    println!("Created task with id: {}", task_id);

    // Verify task is in cache
    assert!(
        manager
            .cache()
            .items
            .iter()
            .any(|i| i.id == task_id && !i.is_deleted),
        "Task should be in cache after creation"
    );

    // Delete the task via direct API call (external to manager)
    let delete_command = SyncCommand::new("item_delete", serde_json::json!({"id": task_id}));
    let delete_response = client
        .sync(SyncRequest::with_commands(vec![delete_command]))
        .await
        .expect("external delete failed");
    assert!(!delete_response.has_errors(), "item_delete should succeed");
    println!("Deleted task externally");

    // The manager's cache should still have the task (marked as not deleted)
    // because we haven't synced yet
    let item_in_cache = manager.cache().items.iter().find(|i| i.id == task_id);
    assert!(
        item_in_cache.is_some(),
        "Task should still be in cache (may be marked deleted or not)"
    );

    // Now sync - should pick up the external deletion
    let cache = manager.sync().await.expect("sync failed");

    // Task should be marked as deleted or removed from cache
    let task_after_sync = cache.items.iter().find(|i| i.id == task_id);
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
}

/// Test that sync picks up tasks updated externally (outside of the cache manager).
///
/// This test validates that when a task is modified via direct API call (not through
/// the SyncManager), a subsequent sync will detect and update the task in cache.
#[tokio::test]
async fn test_sync_picks_up_task_updated_externally() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token);
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client.clone(), store).expect("failed to create manager");

    // Initial sync
    let cache = manager.sync().await.expect("initial sync failed");

    // Get inbox ID
    let inbox = cache
        .projects
        .iter()
        .find(|p| p.inbox_project && !p.is_deleted)
        .expect("Should have inbox project");
    let inbox_id = inbox.id.clone();

    // Create a task through the manager
    let temp_id = uuid::Uuid::new_v4().to_string();
    let add_command = SyncCommand::with_temp_id(
        "item_add",
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
    let task_id = response
        .real_id(&temp_id)
        .expect("Should have temp_id mapping")
        .clone();
    println!("Created task with id: {}", task_id);

    // Verify original content
    let task = manager
        .cache()
        .items
        .iter()
        .find(|i| i.id == task_id)
        .expect("Task should be in cache");
    assert_eq!(task.content, "Original content");

    // Update the task via direct API call (external to manager)
    let update_command = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": task_id,
            "content": "Modified content"
        }),
    );
    let update_response = client
        .sync(SyncRequest::with_commands(vec![update_command]))
        .await
        .expect("external update failed");
    assert!(
        !update_response.has_errors(),
        "item_update should succeed"
    );
    println!("Updated task externally");

    // The manager's cache should still have the old content
    let task_before_sync = manager
        .cache()
        .items
        .iter()
        .find(|i| i.id == task_id)
        .expect("Task should be in cache");
    assert_eq!(
        task_before_sync.content, "Original content",
        "Task should still have original content before sync"
    );

    // Now sync - should pick up the external update
    let cache = manager.sync().await.expect("sync failed");

    // Task should have the updated content
    let task_after_sync = cache
        .items
        .iter()
        .find(|i| i.id == task_id)
        .expect("Task should be in cache after sync");
    assert_eq!(
        task_after_sync.content, "Modified content",
        "Task should have modified content after sync"
    );
    println!("Task content updated to: {}", task_after_sync.content);

    // Clean up
    let delete_command = SyncCommand::new("item_delete", serde_json::json!({"id": task_id}));
    client
        .sync(SyncRequest::with_commands(vec![delete_command]))
        .await
        .expect("cleanup failed");
    println!("Cleaned up test task");
}

/// Test that bulk operations sync correctly.
///
/// This test creates 20 tasks in one sync command batch and verifies
/// all tasks appear in the cache after syncing.
#[tokio::test]
async fn test_sync_after_bulk_operations() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let client = TodoistClient::new(&token);
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client.clone(), store).expect("failed to create manager");

    // Initial sync
    let cache = manager.sync().await.expect("initial sync failed");
    let initial_item_count = cache.items.iter().filter(|i| !i.is_deleted).count();

    // Get inbox ID
    let inbox = cache
        .projects
        .iter()
        .find(|p| p.inbox_project && !p.is_deleted)
        .expect("Should have inbox project");
    let inbox_id = inbox.id.clone();

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
                "item_add",
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
    let task_ids: Vec<String> = temp_ids
        .iter()
        .map(|tid| add_response.real_id(tid).expect("Should have mapping").clone())
        .collect();
    println!("Created {} tasks via bulk operation", task_ids.len());

    // Sync to pick up the bulk changes
    let cache = manager.sync().await.expect("sync failed");

    // Verify all tasks are in cache
    let mut found_count = 0;
    for task_id in &task_ids {
        if cache.items.iter().any(|i| i.id == *task_id && !i.is_deleted) {
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
        new_item_count, initial_item_count, found_count
    );

    // Clean up - batch delete all created tasks
    let delete_commands: Vec<SyncCommand> = task_ids
        .iter()
        .map(|id| SyncCommand::new("item_delete", serde_json::json!({"id": id})))
        .collect();

    client
        .sync(SyncRequest::with_commands(delete_commands))
        .await
        .expect("cleanup failed");
    println!("Cleaned up {} test tasks", batch_size);
}

/// Test that sync token survives restart (persistence).
///
/// This test verifies that after syncing, dropping the manager, and creating
/// a new one from the same cache file, the sync token is preserved and
/// incremental sync works correctly.
#[tokio::test]
async fn test_sync_token_survives_restart() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let sync_token_after_first_sync;
    let item_count_after_first_sync;

    // First session: sync and note the token
    {
        let client = TodoistClient::new(&token);
        let store = CacheStore::with_path(cache_path.clone());
        let mut manager = SyncManager::new(client, store).expect("failed to create manager");

        let cache = manager.sync().await.expect("sync failed");
        sync_token_after_first_sync = cache.sync_token.clone();
        item_count_after_first_sync = cache.items.len();

        println!(
            "First session: token = {}, {} items",
            &sync_token_after_first_sync[..20.min(sync_token_after_first_sync.len())],
            item_count_after_first_sync
        );
    }
    // Manager dropped here

    // Verify cache file exists
    assert!(cache_path.exists(), "Cache file should exist after first sync");

    // Second session: create new manager from same file
    {
        let client = TodoistClient::new(&token);
        let store = CacheStore::with_path(cache_path.clone());
        let manager = SyncManager::new(client, store).expect("failed to create manager after restart");

        // Verify token was preserved
        assert_eq!(
            manager.cache().sync_token, sync_token_after_first_sync,
            "Sync token should be preserved after restart"
        );
        assert!(!manager.cache().needs_full_sync(), "Should not need full sync");

        println!(
            "Second session: loaded token = {}, {} items",
            &manager.cache().sync_token[..20.min(manager.cache().sync_token.len())],
            manager.cache().items.len()
        );

        // Perform incremental sync - should work with preserved token
        let mut manager = manager;
        let cache = manager.sync().await.expect("incremental sync after restart failed");

        // The sync should succeed (no full sync needed)
        assert!(!cache.needs_full_sync(), "Should still not need full sync");
        println!(
            "Incremental sync after restart: {} items",
            cache.items.len()
        );
    }
}

/// Test that invalid sync token triggers full sync recovery.
///
/// This test manually corrupts the sync token in the cache file,
/// then creates a new manager and syncs. The sync should detect
/// the invalid token and fall back to a full sync.
#[tokio::test]
async fn test_full_sync_after_invalid_token() {
    let Some(token) = get_test_token() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // First, do a real sync to get valid data
    {
        let client = TodoistClient::new(&token);
        let store = CacheStore::with_path(cache_path.clone());
        let mut manager = SyncManager::new(client, store).expect("failed to create manager");
        manager.sync().await.expect("initial sync failed");
        println!("Initial sync complete");
    }

    // Corrupt the sync token in the cache file
    {
        let store = CacheStore::with_path(cache_path.clone());
        let mut cache = store.load().expect("failed to load cache");
        let original_token = cache.sync_token.clone();

        // Set an obviously invalid token
        cache.sync_token = "invalid_token_12345".to_string();
        store.save(&cache).expect("failed to save corrupted cache");

        println!(
            "Corrupted sync_token: {} -> {}",
            &original_token[..20.min(original_token.len())],
            cache.sync_token
        );
    }

    // Create new manager with the corrupted cache
    {
        let client = TodoistClient::new(&token);
        let store = CacheStore::with_path(cache_path);
        let mut manager = SyncManager::new(client, store).expect("failed to create manager");

        // Verify the corrupted token was loaded
        assert_eq!(
            manager.cache().sync_token, "invalid_token_12345",
            "Corrupted token should be loaded"
        );

        // Sync should recover via full sync
        // (The API will reject the invalid token, triggering fallback)
        let cache = manager.sync().await.expect("sync with invalid token should recover");

        // After recovery, should have a valid token again
        assert_ne!(
            cache.sync_token, "invalid_token_12345",
            "Sync token should be updated after recovery"
        );
        assert!(
            !cache.needs_full_sync(),
            "Should have valid sync state after recovery"
        );
        assert!(
            cache.projects.iter().any(|p| p.inbox_project),
            "Should have inbox project after recovery"
        );

        println!(
            "Recovery complete: new token = {}, {} projects, {} items",
            &cache.sync_token[..20.min(cache.sync_token.len())],
            cache.projects.len(),
            cache.items.len()
        );
    }
}
