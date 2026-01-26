//! Cache file storage with XDG path support.
//!
//! This module provides persistent storage for the Todoist cache using XDG-compliant
//! paths. The cache is stored as JSON at `~/.cache/td/cache.json`.

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

    /// I/O error during file operations.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

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
/// # Example
///
/// ```no_run
/// use todoist_cache::{Cache, CacheStore};
///
/// let store = CacheStore::new()?;
///
/// // Load existing cache or create new one
/// let cache = store.load().unwrap_or_default();
///
/// // Save cache to disk
/// store.save(&cache)?;
/// # Ok::<(), todoist_cache::CacheStoreError>(())
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
    /// - Returns `CacheStoreError::Io` if the file cannot be read.
    /// - Returns `CacheStoreError::Json` if the file contains invalid JSON.
    ///
    /// # Note
    ///
    /// If the cache file does not exist, this returns an I/O error with
    /// `ErrorKind::NotFound`. Use `load_or_default()` to get a default cache
    /// when the file is missing.
    pub fn load(&self) -> Result<Cache> {
        let contents = fs::read_to_string(&self.path)?;
        let cache = serde_json::from_str(&contents)?;
        Ok(cache)
    }

    /// Loads the cache from disk, returning a default cache if the file doesn't exist.
    ///
    /// # Errors
    ///
    /// - Returns `CacheStoreError::Io` for I/O errors other than "file not found".
    /// - Returns `CacheStoreError::Json` if the file contains invalid JSON.
    pub fn load_or_default(&self) -> Result<Cache> {
        match self.load() {
            Ok(cache) => Ok(cache),
            Err(CacheStoreError::Io(ref e)) if e.kind() == io::ErrorKind::NotFound => {
                Ok(Cache::default())
            }
            Err(e) => Err(e),
        }
    }

    /// Saves the cache to disk.
    ///
    /// Creates the parent directory if it doesn't exist. The cache is written
    /// as pretty-printed JSON for easier debugging.
    ///
    /// # Errors
    ///
    /// - Returns `CacheStoreError::Io` if the file or directory cannot be created/written.
    /// - Returns `CacheStoreError::Json` if serialization fails.
    pub fn save(&self, cache: &Cache) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(cache)?;
        fs::write(&self.path, json)?;
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
    /// Returns `CacheStoreError::Io` if the file cannot be deleted.
    /// Does not return an error if the file doesn't exist.
    pub fn delete(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CacheStoreError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
