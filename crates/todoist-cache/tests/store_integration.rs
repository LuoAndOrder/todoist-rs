//! Integration tests for CacheStore.
//!
//! These tests verify that the cache store correctly reads and writes to disk.

use std::fs;

use tempfile::tempdir;
use todoist_cache::{Cache, CacheStore};

#[test]
fn test_save_and_load_roundtrip() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let store = CacheStore::with_path(cache_path.clone());

    // Create a cache with some data
    let mut cache = Cache::new();
    cache.sync_token = "test_token_12345".to_string();

    // Save to disk
    store.save(&cache).expect("failed to save cache");

    // Verify file exists
    assert!(cache_path.exists(), "cache file should exist after save");

    // Load from disk
    let loaded = store.load().expect("failed to load cache");

    // Verify data matches
    assert_eq!(loaded.sync_token, cache.sync_token);
    assert_eq!(loaded.items, cache.items);
    assert_eq!(loaded.projects, cache.projects);
}

#[test]
fn test_load_missing_file_returns_error() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("nonexistent.json");

    let store = CacheStore::with_path(cache_path);

    let result = store.load();
    assert!(result.is_err(), "load should fail for missing file");

    // Verify it's an I/O error
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("I/O error"),
        "error should be I/O related: {}",
        err
    );
}

#[test]
fn test_load_or_default_missing_file() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("nonexistent.json");

    let store = CacheStore::with_path(cache_path);

    let cache = store
        .load_or_default()
        .expect("load_or_default should succeed for missing file");

    // Should return a default cache
    assert_eq!(cache.sync_token, "*");
    assert!(cache.items.is_empty());
    assert!(cache.needs_full_sync());
}

#[test]
fn test_load_or_default_with_existing_file() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let store = CacheStore::with_path(cache_path);

    // Save a cache first
    let mut original = Cache::new();
    original.sync_token = "existing_token".to_string();
    store.save(&original).expect("failed to save cache");

    // Load with default fallback
    let loaded = store
        .load_or_default()
        .expect("load_or_default should succeed");

    assert_eq!(loaded.sync_token, "existing_token");
}

#[test]
fn test_save_creates_parent_directories() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("nested").join("deep").join("cache.json");

    let store = CacheStore::with_path(cache_path.clone());

    let cache = Cache::new();
    store.save(&cache).expect("save should create directories");

    assert!(cache_path.exists(), "cache file should exist");
    assert!(cache_path.parent().unwrap().exists(), "parent dir should exist");
}

#[test]
fn test_exists_returns_false_for_missing_file() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("nonexistent.json");

    let store = CacheStore::with_path(cache_path);

    assert!(!store.exists(), "exists should return false for missing file");
}

#[test]
fn test_exists_returns_true_after_save() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let store = CacheStore::with_path(cache_path);

    assert!(!store.exists(), "should not exist initially");

    store.save(&Cache::new()).expect("failed to save");

    assert!(store.exists(), "should exist after save");
}

#[test]
fn test_delete_removes_file() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let store = CacheStore::with_path(cache_path.clone());

    // Create the file
    store.save(&Cache::new()).expect("failed to save");
    assert!(cache_path.exists(), "file should exist after save");

    // Delete it
    store.delete().expect("failed to delete");
    assert!(!cache_path.exists(), "file should not exist after delete");
}

#[test]
fn test_delete_nonexistent_file_succeeds() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("nonexistent.json");

    let store = CacheStore::with_path(cache_path);

    // Should not error when deleting a file that doesn't exist
    store.delete().expect("delete should succeed for missing file");
}

#[test]
fn test_load_invalid_json_returns_error() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    // Write invalid JSON
    fs::write(&cache_path, "{ not valid json").expect("failed to write file");

    let store = CacheStore::with_path(cache_path);

    let result = store.load();
    assert!(result.is_err(), "load should fail for invalid JSON");

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("JSON error"),
        "error should be JSON related: {}",
        err
    );
}

#[test]
fn test_cache_file_is_pretty_printed() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let store = CacheStore::with_path(cache_path.clone());

    let cache = Cache::new();
    store.save(&cache).expect("failed to save");

    let contents = fs::read_to_string(&cache_path).expect("failed to read file");

    // Pretty-printed JSON should have newlines
    assert!(
        contents.contains('\n'),
        "JSON should be pretty-printed with newlines"
    );
}

#[test]
fn test_save_overwrites_existing_file() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let cache_path = temp_dir.path().join("cache.json");

    let store = CacheStore::with_path(cache_path);

    // Save first version
    let mut cache1 = Cache::new();
    cache1.sync_token = "token_v1".to_string();
    store.save(&cache1).expect("failed to save v1");

    // Save second version
    let mut cache2 = Cache::new();
    cache2.sync_token = "token_v2".to_string();
    store.save(&cache2).expect("failed to save v2");

    // Load should return v2
    let loaded = store.load().expect("failed to load");
    assert_eq!(loaded.sync_token, "token_v2");
}
