//! Integration tests for SyncManager.
//!
//! These tests use wiremock to mock the Todoist API and verify that SyncManager
//! correctly orchestrates sync operations between the API and cache.

use tempfile::tempdir;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use todoist_api::client::TodoistClient;
use todoist_cache::{Cache, CacheStore, SyncManager};

/// Creates a mock full sync response JSON.
fn mock_full_sync_response() -> serde_json::Value {
    serde_json::json!({
        "sync_token": "new_sync_token_abc123",
        "full_sync": true,
        "full_sync_date_utc": "2025-01-26T10:00:00Z",
        "items": [
            {
                "id": "item-1",
                "project_id": "proj-1",
                "content": "Buy groceries",
                "description": "",
                "priority": 1,
                "child_order": 0,
                "day_order": 0,
                "is_collapsed": false,
                "labels": [],
                "checked": false,
                "is_deleted": false
            },
            {
                "id": "item-2",
                "project_id": "proj-1",
                "content": "Call dentist",
                "description": "",
                "priority": 2,
                "child_order": 1,
                "day_order": 0,
                "is_collapsed": false,
                "labels": ["health"],
                "checked": false,
                "is_deleted": false
            }
        ],
        "projects": [
            {
                "id": "proj-1",
                "name": "Inbox",
                "color": "grey",
                "child_order": 0,
                "is_collapsed": false,
                "shared": false,
                "can_assign_tasks": false,
                "is_deleted": false,
                "is_archived": false,
                "is_favorite": false,
                "inbox_project": true
            }
        ],
        "labels": [],
        "sections": [],
        "notes": [],
        "project_notes": [],
        "reminders": [],
        "filters": [],
        "collaborators": [],
        "collaborator_states": [],
        "live_notifications": [],
        "sync_status": {},
        "temp_id_mapping": {},
        "completed_info": [],
        "locations": []
    })
}

/// Creates a mock incremental sync response JSON.
fn mock_incremental_sync_response() -> serde_json::Value {
    serde_json::json!({
        "sync_token": "incremental_token_xyz789",
        "full_sync": false,
        "items": [
            {
                "id": "item-3",
                "project_id": "proj-1",
                "content": "New task from sync",
                "description": "",
                "priority": 1,
                "child_order": 2,
                "day_order": 0,
                "is_collapsed": false,
                "labels": [],
                "checked": false,
                "is_deleted": false
            }
        ],
        "projects": [],
        "labels": [],
        "sections": [],
        "notes": [],
        "project_notes": [],
        "reminders": [],
        "filters": [],
        "collaborators": [],
        "collaborator_states": [],
        "live_notifications": [],
        "sync_status": {},
        "temp_id_mapping": {},
        "completed_info": [],
        "locations": []
    })
}

#[tokio::test]
async fn test_sync_performs_full_sync_when_no_cache() {
    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Set up mock to expect full sync request (sync_token=*)
    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(body_string_contains("sync_token=%2A")) // URL-encoded "*"
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_full_sync_response()))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Cache should need full sync initially
    assert!(manager.cache().needs_full_sync());

    // Perform sync
    let cache = manager.sync().await.expect("sync failed");

    // Verify cache was updated
    assert!(!cache.needs_full_sync());
    assert_eq!(cache.sync_token, "new_sync_token_abc123");
    assert_eq!(cache.items.len(), 2);
    assert_eq!(cache.projects.len(), 1);

    // Verify cache was persisted
    let store2 = CacheStore::with_path(cache_path);
    let loaded = store2.load().expect("failed to load cache");
    assert_eq!(loaded.sync_token, "new_sync_token_abc123");
    assert_eq!(loaded.items.len(), 2);
}

#[tokio::test]
async fn test_sync_performs_incremental_sync_with_existing_cache() {
    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create an existing cache with a sync token
    let store = CacheStore::with_path(cache_path.clone());
    let mut existing_cache = Cache::new();
    existing_cache.sync_token = "existing_token_123".to_string();
    existing_cache.items = vec![todoist_api::sync::Item {
        id: "item-1".to_string(),
        user_id: None,
        project_id: "proj-1".to_string(),
        content: "Existing task".to_string(),
        description: String::new(),
        priority: 1,
        due: None,
        deadline: None,
        parent_id: None,
        child_order: 0,
        section_id: None,
        day_order: 0,
        is_collapsed: false,
        labels: vec![],
        added_by_uid: None,
        assigned_by_uid: None,
        responsible_uid: None,
        checked: false,
        is_deleted: false,
        added_at: None,
        updated_at: None,
        completed_at: None,
        duration: None,
    }];
    store.save(&existing_cache).expect("failed to save cache");

    // Set up mock to expect incremental sync request
    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(body_string_contains("sync_token=existing_token_123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_incremental_sync_response()))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path);
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Cache should not need full sync
    assert!(!manager.cache().needs_full_sync());

    // Perform sync
    let cache = manager.sync().await.expect("sync failed");

    // Verify incremental update was merged
    assert_eq!(cache.sync_token, "incremental_token_xyz789");
    assert_eq!(cache.items.len(), 2); // 1 existing + 1 new
    assert!(cache.items.iter().any(|i| i.id == "item-1"));
    assert!(cache.items.iter().any(|i| i.id == "item-3"));
}

#[tokio::test]
async fn test_full_sync_forces_full_sync_even_with_existing_token() {
    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create an existing cache with a sync token
    let store = CacheStore::with_path(cache_path.clone());
    let mut existing_cache = Cache::new();
    existing_cache.sync_token = "existing_token_123".to_string();
    store.save(&existing_cache).expect("failed to save cache");

    // Set up mock to expect FULL sync request (sync_token=*), not incremental
    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(body_string_contains("sync_token=%2A")) // URL-encoded "*"
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_full_sync_response()))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path);
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Perform FULL sync
    let cache = manager.full_sync().await.expect("full_sync failed");

    // Verify full sync was performed
    assert_eq!(cache.sync_token, "new_sync_token_abc123");
    assert_eq!(cache.items.len(), 2);
}

#[tokio::test]
async fn test_sync_persists_cache_to_disk() {
    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_full_sync_response()))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    manager.sync().await.expect("sync failed");

    // Verify cache file exists and contains correct data
    assert!(cache_path.exists());

    // Load with a new store instance
    let store2 = CacheStore::with_path(cache_path);
    let loaded = store2.load().expect("failed to load cache");

    assert_eq!(loaded.sync_token, "new_sync_token_abc123");
    assert!(loaded.last_sync.is_some());
    assert!(loaded.full_sync_date_utc.is_some());
}

#[tokio::test]
async fn test_sync_handles_api_error() {
    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path);
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    let result = manager.sync().await;
    assert!(result.is_err());

    // Cache should not have been saved
    assert!(!manager.store().exists());
}

#[tokio::test]
async fn test_reload_refreshes_cache_from_disk() {
    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_full_sync_response()))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Perform initial sync
    manager.sync().await.expect("sync failed");
    assert_eq!(manager.cache().items.len(), 2);

    // Externally modify the cache file
    let store2 = CacheStore::with_path(cache_path.clone());
    let mut modified_cache = store2.load().expect("failed to load");
    modified_cache.items.clear();
    store2.save(&modified_cache).expect("failed to save modified cache");

    // Manager's in-memory cache should still show 2 items
    assert_eq!(manager.cache().items.len(), 2);

    // Reload from disk
    manager.reload().expect("reload failed");

    // Now should reflect the modified file (0 items)
    assert_eq!(manager.cache().items.len(), 0);
}

#[tokio::test]
async fn test_is_stale_with_sync_manager() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create a cache that was synced 10 minutes ago
    let store = CacheStore::with_path(cache_path.clone());
    let mut old_cache = Cache::new();
    old_cache.sync_token = "old_token".to_string();
    old_cache.last_sync = Some(chrono::Utc::now() - chrono::Duration::minutes(10));
    store.save(&old_cache).expect("failed to save cache");

    let client = TodoistClient::new("test-token"); // Won't actually be used
    let store = CacheStore::with_path(cache_path);
    let manager = SyncManager::new(client, store).expect("failed to create manager");

    // Cache should be stale (>5 minutes old by default)
    assert!(manager.is_stale(chrono::Utc::now()));
    assert!(manager.needs_sync(chrono::Utc::now()));
}

#[tokio::test]
async fn test_is_not_stale_when_recently_synced() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create a cache that was synced just now
    let store = CacheStore::with_path(cache_path.clone());
    let mut fresh_cache = Cache::new();
    fresh_cache.sync_token = "fresh_token".to_string();
    fresh_cache.last_sync = Some(chrono::Utc::now());
    store.save(&fresh_cache).expect("failed to save cache");

    let client = TodoistClient::new("test-token");
    let store = CacheStore::with_path(cache_path);
    let manager = SyncManager::new(client, store).expect("failed to create manager");

    // Cache should not be stale
    assert!(!manager.is_stale(chrono::Utc::now()));
    assert!(!manager.needs_sync(chrono::Utc::now()));
}

#[tokio::test]
async fn test_custom_stale_threshold() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create a cache that was synced 3 minutes ago
    let store = CacheStore::with_path(cache_path.clone());
    let mut cache = Cache::new();
    cache.sync_token = "token".to_string();
    cache.last_sync = Some(chrono::Utc::now() - chrono::Duration::minutes(3));
    store.save(&cache).expect("failed to save cache");

    let client = TodoistClient::new("test-token");
    let store = CacheStore::with_path(cache_path.clone());

    // With default 5-minute threshold, should NOT be stale
    let manager5 = SyncManager::new(client.clone(), store).expect("failed to create manager");
    assert!(!manager5.is_stale(chrono::Utc::now()));

    // With 2-minute threshold, SHOULD be stale
    let store2 = CacheStore::with_path(cache_path);
    let manager2 =
        SyncManager::with_stale_threshold(client, store2, 2).expect("failed to create manager");
    assert!(manager2.is_stale(chrono::Utc::now()));
}

#[tokio::test]
async fn test_sync_updates_last_sync_timestamp() {
    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_full_sync_response()))
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path);
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Initially no last_sync
    assert!(manager.cache().last_sync.is_none());

    let before_sync = chrono::Utc::now();
    manager.sync().await.expect("sync failed");
    let after_sync = chrono::Utc::now();

    // last_sync should be set and within the sync window
    let last_sync = manager.cache().last_sync.expect("last_sync should be set");
    assert!(last_sync >= before_sync);
    assert!(last_sync <= after_sync);
}

/// Creates a mock response for a command execution (item_add).
fn mock_command_response() -> serde_json::Value {
    serde_json::json!({
        "sync_token": "post_command_token_456",
        "full_sync": false,
        "items": [
            {
                "id": "real-item-id-789",
                "project_id": "proj-1",
                "content": "New task from command",
                "description": "",
                "priority": 1,
                "child_order": 0,
                "day_order": 0,
                "is_collapsed": false,
                "labels": [],
                "checked": false,
                "is_deleted": false
            }
        ],
        "projects": [],
        "labels": [],
        "sections": [],
        "notes": [],
        "project_notes": [],
        "reminders": [],
        "filters": [],
        "collaborators": [],
        "collaborator_states": [],
        "live_notifications": [],
        "sync_status": {
            "test-cmd-uuid": "ok"
        },
        "temp_id_mapping": {
            "temp-item-123": "real-item-id-789"
        },
        "completed_info": [],
        "locations": []
    })
}

#[tokio::test]
async fn test_execute_commands_adds_item_to_cache() {
    use todoist_api::sync::SyncCommand;

    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create an existing cache with sync token
    let store = CacheStore::with_path(cache_path.clone());
    let mut existing_cache = Cache::new();
    existing_cache.sync_token = "existing_token".to_string();
    store.save(&existing_cache).expect("failed to save cache");

    // Set up mock to respond to command
    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(body_string_contains("commands="))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_command_response()))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Initially no items in cache
    assert_eq!(manager.cache().items.len(), 0);

    // Execute item_add command
    let cmd = SyncCommand::with_temp_id(
        "item_add",
        "temp-item-123",
        serde_json::json!({"content": "New task from command", "project_id": "proj-1"}),
    );
    let response = manager
        .execute_commands(vec![cmd])
        .await
        .expect("execute_commands failed");

    // Verify response contains temp_id_mapping
    assert_eq!(
        response.temp_id_mapping.get("temp-item-123"),
        Some(&"real-item-id-789".to_string())
    );

    // Verify cache was updated with the new item
    assert_eq!(manager.cache().items.len(), 1);
    assert_eq!(manager.cache().items[0].id, "real-item-id-789");
    assert_eq!(manager.cache().items[0].content, "New task from command");

    // Verify sync_token was updated
    assert_eq!(manager.cache().sync_token, "post_command_token_456");

    // Verify cache was persisted to disk
    let store2 = CacheStore::with_path(cache_path);
    let loaded = store2.load().expect("failed to load cache");
    assert_eq!(loaded.items.len(), 1);
    assert_eq!(loaded.sync_token, "post_command_token_456");
}

#[tokio::test]
async fn test_execute_commands_handles_api_error() {
    use todoist_api::sync::SyncCommand;

    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create an existing cache
    let store = CacheStore::with_path(cache_path.clone());
    let mut existing_cache = Cache::new();
    existing_cache.sync_token = "original_token".to_string();
    store.save(&existing_cache).expect("failed to save cache");

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    let cmd = SyncCommand::new("item_add", serde_json::json!({"content": "Test"}));
    let result = manager.execute_commands(vec![cmd]).await;

    // Should return error
    assert!(result.is_err());

    // Cache should remain unchanged (original token preserved)
    assert_eq!(manager.cache().sync_token, "original_token");

    // Disk cache should also remain unchanged
    let store2 = CacheStore::with_path(cache_path);
    let loaded = store2.load().expect("failed to load cache");
    assert_eq!(loaded.sync_token, "original_token");
}

#[tokio::test]
async fn test_execute_commands_updates_last_sync() {
    use todoist_api::sync::SyncCommand;

    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let store = CacheStore::with_path(cache_path.clone());
    let mut existing_cache = Cache::new();
    existing_cache.sync_token = "token".to_string();
    existing_cache.last_sync = None; // No previous sync
    store.save(&existing_cache).expect("failed to save cache");

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_command_response()))
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path);
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Initially no last_sync
    assert!(manager.cache().last_sync.is_none());

    let before = chrono::Utc::now();
    let cmd = SyncCommand::new("item_add", serde_json::json!({"content": "Test"}));
    manager
        .execute_commands(vec![cmd])
        .await
        .expect("execute_commands failed");
    let after = chrono::Utc::now();

    // last_sync should be updated
    let last_sync = manager.cache().last_sync.expect("last_sync should be set");
    assert!(last_sync >= before);
    assert!(last_sync <= after);
}

/// Creates a mock response for item_delete command.
/// The item is returned with is_deleted: true, which triggers removal from cache.
fn mock_delete_command_response() -> serde_json::Value {
    serde_json::json!({
        "sync_token": "post_delete_token_789",
        "full_sync": false,
        "items": [
            {
                "id": "item-to-delete",
                "project_id": "proj-1",
                "content": "Task to delete",
                "description": "",
                "priority": 1,
                "child_order": 0,
                "day_order": 0,
                "is_collapsed": false,
                "labels": [],
                "checked": false,
                "is_deleted": true
            }
        ],
        "projects": [],
        "labels": [],
        "sections": [],
        "notes": [],
        "project_notes": [],
        "reminders": [],
        "filters": [],
        "collaborators": [],
        "collaborator_states": [],
        "live_notifications": [],
        "sync_status": {
            "delete-cmd-uuid": "ok"
        },
        "temp_id_mapping": {},
        "completed_info": [],
        "locations": []
    })
}

#[tokio::test]
async fn test_execute_commands_removes_item_on_delete() {
    use todoist_api::sync::SyncCommand;

    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create a cache with an existing item that we'll delete
    let store = CacheStore::with_path(cache_path.clone());
    let mut existing_cache = Cache::new();
    existing_cache.sync_token = "pre_delete_token".to_string();
    existing_cache.items = vec![
        todoist_api::sync::Item {
            id: "item-to-delete".to_string(),
            user_id: None,
            project_id: "proj-1".to_string(),
            content: "Task to delete".to_string(),
            description: String::new(),
            priority: 1,
            due: None,
            deadline: None,
            parent_id: None,
            child_order: 0,
            section_id: None,
            day_order: 0,
            is_collapsed: false,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            updated_at: None,
            completed_at: None,
            duration: None,
        },
        todoist_api::sync::Item {
            id: "item-to-keep".to_string(),
            user_id: None,
            project_id: "proj-1".to_string(),
            content: "Task to keep".to_string(),
            description: String::new(),
            priority: 1,
            due: None,
            deadline: None,
            parent_id: None,
            child_order: 1,
            section_id: None,
            day_order: 0,
            is_collapsed: false,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            updated_at: None,
            completed_at: None,
            duration: None,
        },
    ];
    store.save(&existing_cache).expect("failed to save cache");

    // Set up mock to respond to delete command
    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(body_string_contains("commands="))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_delete_command_response()))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Initially 2 items in cache
    assert_eq!(manager.cache().items.len(), 2);
    assert!(manager.cache().items.iter().any(|i| i.id == "item-to-delete"));

    // Execute item_delete command
    let cmd = SyncCommand::new("item_delete", serde_json::json!({"id": "item-to-delete"}));
    manager
        .execute_commands(vec![cmd])
        .await
        .expect("execute_commands failed");

    // Verify deleted item was removed from cache
    assert_eq!(manager.cache().items.len(), 1);
    assert!(!manager.cache().items.iter().any(|i| i.id == "item-to-delete"));
    assert!(manager.cache().items.iter().any(|i| i.id == "item-to-keep"));

    // Verify sync_token was updated
    assert_eq!(manager.cache().sync_token, "post_delete_token_789");

    // Verify cache was persisted to disk without the deleted item
    let store2 = CacheStore::with_path(cache_path);
    let loaded = store2.load().expect("failed to load cache");
    assert_eq!(loaded.items.len(), 1);
    assert!(!loaded.items.iter().any(|i| i.id == "item-to-delete"));
}

/// Creates a mock response for item_update command.
/// The item is returned with updated fields.
fn mock_update_command_response() -> serde_json::Value {
    serde_json::json!({
        "sync_token": "post_update_token_abc",
        "full_sync": false,
        "items": [
            {
                "id": "item-to-update",
                "project_id": "proj-1",
                "content": "Updated task content",
                "description": "New description",
                "priority": 4,
                "child_order": 0,
                "day_order": 0,
                "is_collapsed": false,
                "labels": ["work"],
                "checked": false,
                "is_deleted": false
            }
        ],
        "projects": [],
        "labels": [],
        "sections": [],
        "notes": [],
        "project_notes": [],
        "reminders": [],
        "filters": [],
        "collaborators": [],
        "collaborator_states": [],
        "live_notifications": [],
        "sync_status": {
            "update-cmd-uuid": "ok"
        },
        "temp_id_mapping": {},
        "completed_info": [],
        "locations": []
    })
}

#[tokio::test]
async fn test_execute_commands_updates_item_on_edit() {
    use todoist_api::sync::SyncCommand;

    let mock_server = MockServer::start().await;
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Create a cache with an existing item that we'll update
    let store = CacheStore::with_path(cache_path.clone());
    let mut existing_cache = Cache::new();
    existing_cache.sync_token = "pre_update_token".to_string();
    existing_cache.items = vec![todoist_api::sync::Item {
        id: "item-to-update".to_string(),
        user_id: None,
        project_id: "proj-1".to_string(),
        content: "Original task content".to_string(),
        description: String::new(),
        priority: 1,
        due: None,
        deadline: None,
        parent_id: None,
        child_order: 0,
        section_id: None,
        day_order: 0,
        is_collapsed: false,
        labels: vec![],
        added_by_uid: None,
        assigned_by_uid: None,
        responsible_uid: None,
        checked: false,
        is_deleted: false,
        added_at: None,
        updated_at: None,
        completed_at: None,
        duration: None,
    }];
    store.save(&existing_cache).expect("failed to save cache");

    // Set up mock to respond to update command
    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(body_string_contains("commands="))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_update_command_response()))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let store = CacheStore::with_path(cache_path.clone());
    let mut manager = SyncManager::new(client, store).expect("failed to create manager");

    // Verify original state
    assert_eq!(manager.cache().items.len(), 1);
    assert_eq!(manager.cache().items[0].content, "Original task content");
    assert_eq!(manager.cache().items[0].description, "");
    assert_eq!(manager.cache().items[0].priority, 1);
    assert!(manager.cache().items[0].labels.is_empty());

    // Execute item_update command
    let cmd = SyncCommand::new(
        "item_update",
        serde_json::json!({
            "id": "item-to-update",
            "content": "Updated task content",
            "description": "New description",
            "priority": 4,
            "labels": ["work"]
        }),
    );
    manager
        .execute_commands(vec![cmd])
        .await
        .expect("execute_commands failed");

    // Verify item was updated in cache
    assert_eq!(manager.cache().items.len(), 1);
    let updated_item = &manager.cache().items[0];
    assert_eq!(updated_item.id, "item-to-update");
    assert_eq!(updated_item.content, "Updated task content");
    assert_eq!(updated_item.description, "New description");
    assert_eq!(updated_item.priority, 4);
    assert_eq!(updated_item.labels, vec!["work"]);

    // Verify sync_token was updated
    assert_eq!(manager.cache().sync_token, "post_update_token_abc");

    // Verify cache was persisted to disk with updated item
    let store2 = CacheStore::with_path(cache_path);
    let loaded = store2.load().expect("failed to load cache");
    assert_eq!(loaded.items.len(), 1);
    assert_eq!(loaded.items[0].content, "Updated task content");
    assert_eq!(loaded.items[0].priority, 4);
}
