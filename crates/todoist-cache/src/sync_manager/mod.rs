//! Sync manager for orchestrating synchronization between the Todoist API and local cache.
//!
//! The `SyncManager` handles:
//! - Full sync when no cache exists or when explicitly requested
//! - Incremental sync using stored sync tokens
//! - Cache staleness detection (>5 minutes by default)
//!
//! # Example
//!
//! ```no_run
//! use todoist_api_rs::client::TodoistClient;
//! use todoist_cache_rs::{CacheStore, SyncManager};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = TodoistClient::new("your-api-token")?;
//!     let store = CacheStore::new()?;
//!     let mut manager = SyncManager::new(client, store)?;
//!
//!     // Sync (full if no cache, incremental otherwise)
//!     let cache = manager.sync().await?;
//!     println!("Synced {} items", cache.items.len());
//!
//!     Ok(())
//! }
//! ```

mod lookups;

use chrono::{DateTime, Duration, Utc};
use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{SyncCommand, SyncRequest, SyncResponse};

use crate::{Cache, CacheStore, CacheStoreError};

// Re-export lookup utilities for error formatting and tests
#[cfg(test)]
pub(crate) use lookups::find_similar_name;
pub(crate) use lookups::format_not_found_error;

/// Default staleness threshold in minutes.
const DEFAULT_STALE_MINUTES: i64 = 5;

/// Errors that can occur during sync operations.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// Cache storage error.
    #[error("cache error: {0}")]
    Cache(#[from] CacheStoreError),

    /// API error.
    #[error("API error: {0}")]
    Api(#[from] todoist_api_rs::error::Error),

    /// Resource not found in cache (even after sync).
    #[error("{}", format_not_found_error(resource_type, identifier, suggestion.as_deref()))]
    NotFound {
        /// The type of resource that was not found (e.g., "project", "label").
        resource_type: &'static str,
        /// The name or ID that was searched for.
        identifier: String,
        /// Optional suggestion for similar resource names.
        suggestion: Option<String>,
    },

    /// Sync token was rejected by the API.
    ///
    /// This indicates the cached sync token is no longer valid and the client
    /// should perform a full sync to obtain a fresh token.
    #[error("sync token invalid or expired, full sync required")]
    SyncTokenInvalid,

    /// Validation or lookup error for user-provided input.
    #[error("{0}")]
    Validation(String),
}

/// Result type for sync operations.
pub type Result<T> = std::result::Result<T, SyncError>;

/// Orchestrates synchronization between the Todoist API and local cache.
///
/// `SyncManager` provides methods for syncing data, checking cache staleness,
/// and forcing full syncs when needed.
///
/// # Thread Safety
///
/// `SyncManager` is [`Send`] but **not** [`Sync`]. Most methods require `&mut self`
/// because they modify the internal cache and persist changes to disk.
///
/// For multi-threaded usage, wrap in `Arc<Mutex<SyncManager>>` or
/// `Arc<tokio::sync::Mutex<SyncManager>>`:
///
/// ```no_run
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
/// use todoist_api_rs::client::TodoistClient;
/// use todoist_cache_rs::{CacheStore, SyncManager};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = TodoistClient::new("token")?;
/// let store = CacheStore::new()?;
/// let manager = Arc::new(Mutex::new(SyncManager::new(client, store)?));
///
/// // Lock before calling mutable methods
/// let mut guard = manager.lock().await;
/// guard.sync().await?;
/// # Ok(())
/// # }
/// ```
///
/// In typical CLI usage, the manager is owned by a single async task and no
/// synchronization is needed.
pub struct SyncManager {
    /// The Todoist API client.
    client: TodoistClient,

    /// The cache storage.
    store: CacheStore,

    /// The current in-memory cache.
    cache: Cache,

    /// Staleness threshold in minutes.
    stale_minutes: i64,
}

impl SyncManager {
    /// Creates a new `SyncManager` with the given client and store.
    ///
    /// The cache is loaded from disk if it exists, otherwise a new empty cache is created.
    ///
    /// # Arguments
    ///
    /// * `client` - The Todoist API client
    /// * `store` - The cache store for persistence
    ///
    /// # Errors
    ///
    /// Returns an error if loading the cache from disk fails (excluding file not found).
    pub fn new(client: TodoistClient, store: CacheStore) -> Result<Self> {
        let cache = store.load_or_default()?;
        Ok(Self {
            client,
            store,
            cache,
            stale_minutes: DEFAULT_STALE_MINUTES,
        })
    }

    /// Creates a new `SyncManager` with a custom staleness threshold.
    ///
    /// # Arguments
    ///
    /// * `client` - The Todoist API client
    /// * `store` - The cache store for persistence
    /// * `stale_minutes` - Number of minutes after which the cache is considered stale
    pub fn with_stale_threshold(
        client: TodoistClient,
        store: CacheStore,
        stale_minutes: i64,
    ) -> Result<Self> {
        let cache = store.load_or_default()?;
        Ok(Self {
            client,
            store,
            cache,
            stale_minutes,
        })
    }

    /// Returns a reference to the current cache.
    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    /// Returns a reference to the cache store.
    pub fn store(&self) -> &CacheStore {
        &self.store
    }

    /// Returns a reference to the Todoist client.
    pub fn client(&self) -> &TodoistClient {
        &self.client
    }

    /// Returns true if the cache is stale (older than the configured threshold).
    ///
    /// A cache is considered stale if:
    /// - It has never been synced (`last_sync` is `None`)
    /// - It was last synced more than `stale_minutes` ago
    ///
    /// # Arguments
    ///
    /// * `now` - The current time to compare against
    pub fn is_stale(&self, now: DateTime<Utc>) -> bool {
        match self.cache.last_sync {
            None => true,
            Some(last_sync) => {
                let threshold = Duration::minutes(self.stale_minutes);
                now.signed_duration_since(last_sync) > threshold
            }
        }
    }

    /// Returns true if a sync is needed.
    ///
    /// A sync is needed if:
    /// - The cache requires a full sync (no sync token)
    /// - The cache is stale
    ///
    /// # Arguments
    ///
    /// * `now` - The current time to compare against
    pub fn needs_sync(&self, now: DateTime<Utc>) -> bool {
        self.cache.needs_full_sync() || self.is_stale(now)
    }

    /// Performs a sync operation.
    ///
    /// This method automatically determines whether to perform a full or incremental sync:
    /// - Full sync if the cache has never been synced (sync_token is "*")
    /// - Incremental sync otherwise
    ///
    /// If an incremental sync fails due to an invalid sync token, this method
    /// automatically falls back to a full sync with `sync_token='*'`.
    ///
    /// The cache is saved to disk asynchronously after a successful sync.
    ///
    /// # Returns
    ///
    /// A reference to the updated cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or if saving the cache fails.
    pub async fn sync(&mut self) -> Result<&Cache> {
        if self.cache.needs_full_sync() {
            // Already need a full sync, just do it
            let request = SyncRequest::full_sync();
            let response = self.client.sync(request).await?;
            self.cache.apply_sync_response(&response);
            self.store.save_async(&self.cache).await?;
            return Ok(&self.cache);
        }

        // Try incremental sync
        let request = SyncRequest::incremental(&self.cache.sync_token);
        match self.client.sync(request).await {
            Ok(response) => {
                self.cache.apply_sync_response(&response);
                self.store.save_async(&self.cache).await?;
                Ok(&self.cache)
            }
            Err(e) if e.is_invalid_sync_token() => {
                // Sync token rejected - fall back to full sync
                eprintln!("Warning: Sync token invalid, performing full sync to recover.");

                // Reset sync token to force full sync
                self.cache.sync_token = "*".to_string();

                // Perform full sync
                let request = SyncRequest::full_sync();
                let response = self.client.sync(request).await?;
                self.cache.apply_sync_response(&response);
                self.store.save_async(&self.cache).await?;
                Ok(&self.cache)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Forces a full sync, ignoring the stored sync token.
    ///
    /// This replaces all cached data with fresh data from the server.
    /// The cache is saved to disk asynchronously after a successful sync.
    ///
    /// # Returns
    ///
    /// A reference to the updated cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or if saving the cache fails.
    pub async fn full_sync(&mut self) -> Result<&Cache> {
        let request = SyncRequest::full_sync();
        let response = self.client.sync(request).await?;
        self.cache.apply_sync_response(&response);
        self.store.save_async(&self.cache).await?;

        Ok(&self.cache)
    }

    /// Reloads the cache from disk.
    ///
    /// This discards any in-memory changes and loads the cache from disk.
    /// Useful if the cache file was modified externally.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the cache from disk fails.
    pub fn reload(&mut self) -> Result<&Cache> {
        self.cache = self.store.load_or_default()?;
        Ok(&self.cache)
    }

    /// Executes one or more commands via the Sync API.
    ///
    /// This method sends the commands to the Todoist API, applies the response
    /// to the cache, and saves the cache to disk. It returns the full response
    /// so callers can access `temp_id_mapping` to resolve temporary IDs to
    /// real IDs, and `sync_status` to check per-command results.
    ///
    /// # Arguments
    ///
    /// * `commands` - A vector of `SyncCommand` objects to execute
    ///
    /// # Returns
    ///
    /// The `SyncResponse` from the API, containing:
    /// - `sync_status`: Success/failure for each command (keyed by command UUID)
    /// - `temp_id_mapping`: Maps temporary IDs to real IDs for created resources
    /// - Updated resources affected by the commands
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or if saving the cache fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_api_rs::sync::{SyncCommand, SyncCommandType};
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Create a new task
    ///     let temp_id = uuid::Uuid::new_v4().to_string();
    ///     let cmd = SyncCommand::with_temp_id(
    ///         SyncCommandType::ItemAdd,
    ///         &temp_id,
    ///         serde_json::json!({"content": "Buy milk", "project_id": "inbox"}),
    ///     );
    ///
    ///     let response = manager.execute_commands(vec![cmd]).await?;
    ///
    ///     // Get the real ID from temp_id_mapping
    ///     if let Some(real_id) = response.temp_id_mapping.get(&temp_id) {
    ///         println!("Created task with ID: {}", real_id);
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn execute_commands(&mut self, commands: Vec<SyncCommand>) -> Result<SyncResponse> {
        // Execute command batches against the current sync token so mutation
        // responses include incremental resource deltas (including delete tombstones).
        // Without resource_types, the API only returns sync_status and temp_id_mapping.
        let request = SyncRequest::incremental(self.cache.sync_token.clone())
            .with_resource_types(vec!["all".to_string()])
            .add_commands(commands);
        let response = self.client.sync(request).await?;

        // Apply the mutation response to update cache with affected resources
        self.cache.apply_mutation_response(&response);

        // Persist the updated cache asynchronously
        self.store.save_async(&self.cache).await?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests;
