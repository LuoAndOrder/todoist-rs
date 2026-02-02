//! Cache file storage with XDG path support.
//!
//! This module provides persistent storage for the Todoist cache using XDG-compliant
//! paths. The cache is stored as JSON at `~/.cache/td/cache.json`.
//!
//! Both synchronous and asynchronous I/O methods are provided:
//! - `save()`, `load()` - Synchronous methods using `std::fs`
//! - `save_async()`, `load_async()` - Asynchronous methods using `tokio::fs`
//!
//! The async methods are recommended for use in async contexts (like `SyncManager::sync()`)
//! to avoid blocking the tokio runtime.

use std::fs;
use std::io;
use std::path::PathBuf;

use directories::ProjectDirs;
use thiserror::Error;

use crate::Cache;

/// Default cache filename.
const CACHE_FILENAME: &str = "cache.json";

/// Application qualifier (for XDG paths).
const QUALIFIER: &str = "";

/// Application organization (for XDG paths).
const ORGANIZATION: &str = "";

/// Application name (for XDG paths).
const APPLICATION: &str = "td";

/// Errors that can occur during cache storage operations.
#[derive(Debug, Error)]
pub enum CacheStoreError {
    /// Failed to determine XDG cache directory.
    #[error("failed to determine cache directory: no valid home directory found")]
    NoCacheDir,

    /// I/O error during file read.
    #[error("failed to read cache file '{path}': {source}")]
    ReadError {
        /// The path that failed to read.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// I/O error during file write.
    #[error("failed to write cache file '{path}': {source}")]
    WriteError {
        /// The path that failed to write.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// I/O error during directory creation.
    #[error("failed to create cache directory '{path}': {source}")]
    CreateDirError {
        /// The directory path that failed to create.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// I/O error during file delete.
    #[error("failed to delete cache file '{path}': {source}")]
    DeleteError {
        /// The path that failed to delete.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result type for cache store operations.
pub type Result<T> = std::result::Result<T, CacheStoreError>;

/// Persistent storage for the Todoist cache.
///
/// `CacheStore` handles reading and writing the cache to disk using XDG-compliant
/// paths. On Unix systems, the cache is stored at `~/.cache/td/cache.json`.
///
/// # Thread Safety
///
/// `CacheStore` is [`Send`] and [`Sync`], but file operations are not atomic.
/// Concurrent calls to `save()` from multiple threads could result in corrupted
/// data on disk. For concurrent access, use external synchronization:
///
/// ```no_run
/// use std::sync::{Arc, Mutex};
/// use todoist_cache_rs::CacheStore;
///
/// let store = Arc::new(Mutex::new(CacheStore::new()?));
/// # Ok::<(), todoist_cache_rs::CacheStoreError>(())
/// ```
///
/// In typical CLI usage, the store is owned by a single-threaded runtime
/// and external synchronization is not needed.
///
/// # Example
///
/// ```no_run
/// use todoist_cache_rs::{Cache, CacheStore};
///
/// let store = CacheStore::new()?;
///
/// // Load existing cache or create new one
/// let cache = store.load().unwrap_or_default();
///
/// // Save cache to disk
/// store.save(&cache)?;
/// # Ok::<(), todoist_cache_rs::CacheStoreError>(())
/// ```
#[derive(Debug, Clone)]
pub struct CacheStore {
    /// Path to the cache file.
    path: PathBuf,
}

impl CacheStore {
    /// Creates a new `CacheStore` with the default XDG cache path.
    ///
    /// The cache file will be located at `~/.cache/td/cache.json` on Unix systems.
    ///
    /// # Errors
    ///
    /// Returns `CacheStoreError::NoCacheDir` if the home directory cannot be determined.
    pub fn new() -> Result<Self> {
        let path = Self::default_path()?;
        Ok(Self { path })
    }

    /// Creates a new `CacheStore` with a custom path.
    ///
    /// This is primarily useful for testing.
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Returns the default XDG cache path for the cache file.
    ///
    /// On Unix: `~/.cache/td/cache.json`
    /// On macOS: `~/Library/Caches/td/cache.json`
    /// On Windows: `C:\Users\<User>\AppData\Local\td\cache\cache.json`
    ///
    /// # Errors
    ///
    /// Returns `CacheStoreError::NoCacheDir` if the home directory cannot be determined.
    pub fn default_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .ok_or(CacheStoreError::NoCacheDir)?;

        let cache_dir = project_dirs.cache_dir();
        Ok(cache_dir.join(CACHE_FILENAME))
    }

    /// Returns the path to the cache file.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Loads the cache from disk.
    ///
    /// # Errors
    ///
    /// - Returns `CacheStoreError::ReadError` if the file cannot be read.
    /// - Returns `CacheStoreError::Json` if the file contains invalid JSON.
    ///
    /// # Note
    ///
    /// If the cache file does not exist, this returns an I/O error with
    /// `ErrorKind::NotFound`. Use `load_or_default()` to get a default cache
    /// when the file is missing.
    pub fn load(&self) -> Result<Cache> {
        let contents = fs::read_to_string(&self.path).map_err(|e| CacheStoreError::ReadError {
            path: self.path.clone(),
            source: e,
        })?;
        let mut cache: Cache = serde_json::from_str(&contents)?;
        // Rebuild indexes since they are not serialized
        cache.rebuild_indexes();
        Ok(cache)
    }

    /// Loads the cache from disk, returning a default cache if the file doesn't exist.
    ///
    /// # Errors
    ///
    /// - Returns `CacheStoreError::ReadError` for I/O errors other than "file not found".
    /// - Returns `CacheStoreError::Json` if the file contains invalid JSON.
    pub fn load_or_default(&self) -> Result<Cache> {
        match self.load() {
            Ok(cache) => Ok(cache),
            Err(CacheStoreError::ReadError { ref source, .. })
                if source.kind() == io::ErrorKind::NotFound =>
            {
                Ok(Cache::default())
            }
            Err(e) => Err(e),
        }
    }

    /// Saves the cache to disk atomically.
    ///
    /// Creates the parent directory if it doesn't exist. The cache is written
    /// as pretty-printed JSON for easier debugging.
    ///
    /// Uses atomic write (tempfile + rename) to prevent corruption if the process
    /// crashes mid-write.
    ///
    /// # Errors
    ///
    /// - Returns `CacheStoreError::CreateDirError` if the directory cannot be created.
    /// - Returns `CacheStoreError::WriteError` if the file cannot be written.
    /// - Returns `CacheStoreError::Json` if serialization fails.
    pub fn save(&self, cache: &Cache) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| CacheStoreError::CreateDirError {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let json = serde_json::to_string_pretty(cache)?;

        // Atomic write: write to temp file, then rename
        // This prevents corruption if the process crashes mid-write
        let temp_path = self.path.with_extension("tmp");
        fs::write(&temp_path, &json).map_err(|e| CacheStoreError::WriteError {
            path: temp_path.clone(),
            source: e,
        })?;
        fs::rename(&temp_path, &self.path).map_err(|e| CacheStoreError::WriteError {
            path: self.path.clone(),
            source: e,
        })?;

        Ok(())
    }

    /// Returns true if the cache file exists on disk.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Deletes the cache file from disk.
    ///
    /// # Errors
    ///
    /// Returns `CacheStoreError::DeleteError` if the file cannot be deleted.
    /// Does not return an error if the file doesn't exist.
    pub fn delete(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CacheStoreError::DeleteError {
                path: self.path.clone(),
                source: e,
            }),
        }
    }

    // =========================================================================
    // Async I/O Methods
    // =========================================================================

    /// Loads the cache from disk asynchronously.
    ///
    /// This is the async equivalent of [`load()`](Self::load). Use this method
    /// in async contexts to avoid blocking the tokio runtime.
    ///
    /// # Errors
    ///
    /// - Returns `CacheStoreError::ReadError` if the file cannot be read.
    /// - Returns `CacheStoreError::Json` if the file contains invalid JSON.
    ///
    /// # Note
    ///
    /// If the cache file does not exist, this returns an I/O error with
    /// `ErrorKind::NotFound`. Use [`load_or_default_async()`](Self::load_or_default_async)
    /// to get a default cache when the file is missing.
    pub async fn load_async(&self) -> Result<Cache> {
        let contents =
            tokio::fs::read_to_string(&self.path)
                .await
                .map_err(|e| CacheStoreError::ReadError {
                    path: self.path.clone(),
                    source: e,
                })?;
        let mut cache: Cache = serde_json::from_str(&contents)?;
        // Rebuild indexes since they are not serialized
        cache.rebuild_indexes();
        Ok(cache)
    }

    /// Loads the cache from disk asynchronously, returning a default cache if the file doesn't exist.
    ///
    /// This is the async equivalent of [`load_or_default()`](Self::load_or_default).
    ///
    /// # Errors
    ///
    /// - Returns `CacheStoreError::ReadError` for I/O errors other than "file not found".
    /// - Returns `CacheStoreError::Json` if the file contains invalid JSON.
    pub async fn load_or_default_async(&self) -> Result<Cache> {
        match self.load_async().await {
            Ok(cache) => Ok(cache),
            Err(CacheStoreError::ReadError { ref source, .. })
                if source.kind() == io::ErrorKind::NotFound =>
            {
                Ok(Cache::default())
            }
            Err(e) => Err(e),
        }
    }

    /// Saves the cache to disk asynchronously using atomic write.
    ///
    /// This is the async equivalent of [`save()`](Self::save). Use this method
    /// in async contexts to avoid blocking the tokio runtime.
    ///
    /// Creates the parent directory if it doesn't exist. The cache is written
    /// as pretty-printed JSON for easier debugging.
    ///
    /// Uses atomic write (tempfile + rename) to prevent corruption if the process
    /// crashes mid-write.
    ///
    /// # Errors
    ///
    /// - Returns `CacheStoreError::CreateDirError` if the directory cannot be created.
    /// - Returns `CacheStoreError::WriteError` if the file cannot be written.
    /// - Returns `CacheStoreError::Json` if serialization fails.
    pub async fn save_async(&self, cache: &Cache) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| CacheStoreError::CreateDirError {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
        }

        let json = serde_json::to_string_pretty(cache)?;

        // Atomic write: write to temp file, then rename
        // This prevents corruption if the process crashes mid-write
        let temp_path = self.path.with_extension("tmp");
        tokio::fs::write(&temp_path, &json)
            .await
            .map_err(|e| CacheStoreError::WriteError {
                path: temp_path.clone(),
                source: e,
            })?;
        tokio::fs::rename(&temp_path, &self.path)
            .await
            .map_err(|e| CacheStoreError::WriteError {
                path: self.path.clone(),
                source: e,
            })?;

        Ok(())
    }

    /// Deletes the cache file from disk asynchronously.
    ///
    /// This is the async equivalent of [`delete()`](Self::delete).
    ///
    /// # Errors
    ///
    /// Returns `CacheStoreError::DeleteError` if the file cannot be deleted.
    /// Does not return an error if the file doesn't exist.
    pub async fn delete_async(&self) -> Result<()> {
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CacheStoreError::DeleteError {
                path: self.path.clone(),
                source: e,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // Synchronous I/O Tests
    // ==========================================================================

    #[test]
    fn test_default_path_returns_xdg_path() {
        let path = CacheStore::default_path().expect("should get default path");

        // Path should end with td/cache.json (or td\cache.json on Windows)
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with("td/cache.json")
                || path_str.ends_with("td\\cache.json")
                || path_str.ends_with("td/cache/cache.json")
                || path_str.ends_with("td\\cache\\cache.json"),
            "path should contain td and cache.json: {}",
            path_str
        );

        // Path should be absolute
        assert!(path.is_absolute(), "path should be absolute: {:?}", path);
    }

    #[test]
    fn test_cache_store_new_uses_default_path() {
        let store = CacheStore::new().expect("should create store");
        let default_path = CacheStore::default_path().expect("should get default path");

        assert_eq!(store.path(), &default_path);
    }

    #[test]
    fn test_cache_store_with_custom_path() {
        let custom_path = PathBuf::from("/tmp/test/cache.json");
        let store = CacheStore::with_path(custom_path.clone());

        assert_eq!(store.path(), &custom_path);
    }

    #[test]
    fn test_cache_store_path_contains_application_name() {
        let path = CacheStore::default_path().expect("should get default path");
        let path_str = path.to_string_lossy();

        assert!(
            path_str.contains("td"),
            "path should contain 'td': {}",
            path_str
        );
    }

    #[test]
    fn test_read_error_includes_file_path() {
        let path = PathBuf::from("/nonexistent/path/to/cache.json");
        let store = CacheStore::with_path(path.clone());

        let result = store.load();
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_msg = error.to_string();

        // Error message should include the file path
        assert!(
            error_msg.contains("/nonexistent/path/to/cache.json"),
            "error should include file path: {}",
            error_msg
        );
        assert!(
            error_msg.contains("failed to read cache file"),
            "error should describe the operation: {}",
            error_msg
        );
    }

    #[test]
    fn test_read_error_has_source() {
        use std::error::Error;

        let path = PathBuf::from("/nonexistent/path/to/cache.json");
        let store = CacheStore::with_path(path);

        let result = store.load();
        let error = result.unwrap_err();

        // Should have an underlying source error
        assert!(
            error.source().is_some(),
            "error should have a source io::Error"
        );
    }

    #[test]
    fn test_load_or_default_still_works_for_not_found() {
        let path = PathBuf::from("/nonexistent/path/to/cache.json");
        let store = CacheStore::with_path(path);

        // load_or_default should return a default cache for missing files
        let result = store.load_or_default();
        assert!(result.is_ok());

        let cache = result.unwrap();
        assert_eq!(cache.sync_token, "*");
    }

    #[test]
    fn test_write_error_includes_file_path() {
        use tempfile::tempdir;

        // Create a file where we need a directory - this will cause mkdir to fail
        let temp_dir = tempdir().expect("failed to create temp dir");
        let blocker_file = temp_dir.path().join("blocker");
        fs::write(&blocker_file, "blocking").expect("failed to create blocker file");

        // Try to create a cache file inside the blocker file (which is not a directory)
        let path = blocker_file.join("subdir").join("cache.json");
        let store = CacheStore::with_path(path);

        let cache = crate::Cache::new();
        let result = store.save(&cache);
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_msg = error.to_string();

        // Error message should describe the operation and include a path
        assert!(
            error_msg.contains("failed to create cache directory")
                || error_msg.contains("failed to write cache file"),
            "error should describe the operation: {}",
            error_msg
        );
        assert!(
            error_msg.contains("blocker"),
            "error should include path component: {}",
            error_msg
        );
    }

    #[test]
    fn test_delete_error_includes_file_path() {
        // Create a directory where a file is expected - delete will fail
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("failed to create temp dir");
        let path = temp_dir.path().join("cache.json");

        // Create a directory at the cache path (can't delete a directory with remove_file)
        fs::create_dir(&path).expect("failed to create directory");

        let store = CacheStore::with_path(path.clone());
        let result = store.delete();

        // On some systems this may succeed or fail depending on behavior
        // If it fails, the error should include the path
        if let Err(error) = result {
            let error_msg = error.to_string();
            assert!(
                error_msg.contains("cache.json"),
                "error should include file path: {}",
                error_msg
            );
            assert!(
                error_msg.contains("failed to delete cache file"),
                "error should describe the operation: {}",
                error_msg
            );
        }
    }

    #[test]
    fn test_error_message_format_read() {
        let error = CacheStoreError::ReadError {
            path: PathBuf::from("/home/user/.cache/td/cache.json"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
        };

        let msg = error.to_string();
        assert_eq!(
            msg,
            "failed to read cache file '/home/user/.cache/td/cache.json': permission denied"
        );
    }

    #[test]
    fn test_error_message_format_write() {
        let error = CacheStoreError::WriteError {
            path: PathBuf::from("/home/user/.cache/td/cache.json"),
            source: io::Error::other("disk full"),
        };

        let msg = error.to_string();
        assert_eq!(
            msg,
            "failed to write cache file '/home/user/.cache/td/cache.json': disk full"
        );
    }

    #[test]
    fn test_error_message_format_create_dir() {
        let error = CacheStoreError::CreateDirError {
            path: PathBuf::from("/home/user/.cache/td"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
        };

        let msg = error.to_string();
        assert_eq!(
            msg,
            "failed to create cache directory '/home/user/.cache/td': permission denied"
        );
    }

    #[test]
    fn test_error_message_format_delete() {
        let error = CacheStoreError::DeleteError {
            path: PathBuf::from("/home/user/.cache/td/cache.json"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
        };

        let msg = error.to_string();
        assert_eq!(
            msg,
            "failed to delete cache file '/home/user/.cache/td/cache.json': permission denied"
        );
    }

    // ==========================================================================
    // Async I/O Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_save_and_load_async() {
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("failed to create temp dir");
        let path = temp_dir.path().join("cache.json");
        let store = CacheStore::with_path(path);

        // Create a cache with some data
        let mut cache = crate::Cache::new();
        cache.sync_token = "test-token".to_string();

        // Save asynchronously
        store.save_async(&cache).await.expect("save_async failed");

        // Load asynchronously
        let loaded = store.load_async().await.expect("load_async failed");
        assert_eq!(loaded.sync_token, "test-token");
    }

    #[tokio::test]
    async fn test_atomic_write_async() {
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("failed to create temp dir");
        let path = temp_dir.path().join("cache.json");
        let store = CacheStore::with_path(path.clone());

        let cache = crate::Cache::new();
        store.save_async(&cache).await.expect("save_async failed");

        // Verify no temp file left behind
        let temp_path = path.with_extension("tmp");
        assert!(!temp_path.exists(), "temp file should be cleaned up");
        assert!(path.exists(), "cache file should exist");
    }

    #[tokio::test]
    async fn test_load_async_missing_file() {
        let path = PathBuf::from("/nonexistent/path/to/cache.json");
        let store = CacheStore::with_path(path);

        let result = store.load_async().await;
        assert!(result.is_err());

        // Should be a ReadError with NotFound
        match result.unwrap_err() {
            CacheStoreError::ReadError { source, .. } => {
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
            }
            other => panic!("expected ReadError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_load_or_default_async_missing_file() {
        let path = PathBuf::from("/nonexistent/path/to/cache.json");
        let store = CacheStore::with_path(path);

        // Should return default cache for missing files
        let result = store.load_or_default_async().await;
        assert!(result.is_ok());

        let cache = result.unwrap();
        assert_eq!(cache.sync_token, "*");
    }

    #[tokio::test]
    async fn test_delete_async() {
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("failed to create temp dir");
        let path = temp_dir.path().join("cache.json");
        let store = CacheStore::with_path(path.clone());

        // Create the file
        let cache = crate::Cache::new();
        store.save_async(&cache).await.expect("save_async failed");
        assert!(path.exists());

        // Delete asynchronously
        store.delete_async().await.expect("delete_async failed");
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_delete_async_nonexistent() {
        let path = PathBuf::from("/nonexistent/path/to/cache.json");
        let store = CacheStore::with_path(path);

        // Should not error for missing files
        let result = store.delete_async().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_save_async_creates_directory() {
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("failed to create temp dir");
        let path = temp_dir.path().join("subdir").join("nested").join("cache.json");
        let store = CacheStore::with_path(path.clone());

        // Parent directory doesn't exist
        assert!(!path.parent().unwrap().exists());

        // Save should create it
        let cache = crate::Cache::new();
        store.save_async(&cache).await.expect("save_async failed");

        assert!(path.exists());
    }
}
