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

use todoist_api::client::TodoistClient;
use todoist_api::sync::{SyncCommand, SyncRequest};
use todoist_cache::{CacheStore, SyncManager};

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
