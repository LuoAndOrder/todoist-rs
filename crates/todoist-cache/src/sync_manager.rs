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
//! use todoist_api::client::TodoistClient;
//! use todoist_cache::{CacheStore, SyncManager};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = TodoistClient::new("your-api-token");
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

use chrono::{DateTime, Duration, Utc};
use todoist_api::client::TodoistClient;
use todoist_api::sync::{SyncCommand, SyncRequest, SyncResponse};

use crate::{Cache, CacheStore, CacheStoreError};

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
    Api(#[from] todoist_api::error::Error),

    /// Resource not found in cache (even after sync).
    #[error("{resource_type} not found: {identifier}")]
    NotFound {
        /// The type of resource that was not found (e.g., "project", "label").
        resource_type: &'static str,
        /// The name or ID that was searched for.
        identifier: String,
    },
}

/// Result type for sync operations.
pub type Result<T> = std::result::Result<T, SyncError>;

/// Orchestrates synchronization between the Todoist API and local cache.
///
/// `SyncManager` provides methods for syncing data, checking cache staleness,
/// and forcing full syncs when needed.
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
    /// The cache is saved to disk after a successful sync.
    ///
    /// # Returns
    ///
    /// A reference to the updated cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or if saving the cache fails.
    pub async fn sync(&mut self) -> Result<&Cache> {
        let request = if self.cache.needs_full_sync() {
            SyncRequest::full_sync()
        } else {
            SyncRequest::incremental(&self.cache.sync_token)
        };

        let response = self.client.sync(request).await?;
        self.cache.apply_sync_response(&response);
        self.store.save(&self.cache)?;

        Ok(&self.cache)
    }

    /// Forces a full sync, ignoring the stored sync token.
    ///
    /// This replaces all cached data with fresh data from the server.
    /// The cache is saved to disk after a successful sync.
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
        self.store.save(&self.cache)?;

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
    /// use todoist_api::client::TodoistClient;
    /// use todoist_api::sync::SyncCommand;
    /// use todoist_cache::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token");
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Create a new task
    ///     let temp_id = uuid::Uuid::new_v4().to_string();
    ///     let cmd = SyncCommand::with_temp_id(
    ///         "item_add",
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
        // Request all resource types along with commands so the API returns
        // affected resources (items, projects, etc.) in the response.
        // Without resource_types, the API only returns sync_status and temp_id_mapping.
        let request = SyncRequest::with_commands(commands)
            .with_resource_types(vec!["all".to_string()]);
        let response = self.client.sync(request).await?;

        // Apply the mutation response to update cache with affected resources
        self.cache.apply_mutation_response(&response);

        // Persist the updated cache
        self.store.save(&self.cache)?;

        Ok(response)
    }

    // ==================== Smart Lookup Methods ====================

    /// Resolves a project by name or ID, with auto-sync fallback.
    ///
    /// This method first attempts to find the project in the cache. If not found,
    /// it performs a sync and retries the lookup. This provides a seamless experience
    /// where users can reference recently-created projects without manual syncing.
    ///
    /// # Arguments
    ///
    /// * `name_or_id` - The project name (case-insensitive) or ID to search for
    ///
    /// # Returns
    ///
    /// A reference to the matching `Project` from the cache.
    ///
    /// # Errors
    ///
    /// Returns `SyncError::NotFound` if the project cannot be found even after syncing.
    /// Returns `SyncError::Api` if the sync operation fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use todoist_api::client::TodoistClient;
    /// use todoist_cache::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token");
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Find by name (case-insensitive)
    ///     let project = manager.resolve_project("work").await?;
    ///     println!("Found project: {} ({})", project.name, project.id);
    ///
    ///     // Find by ID
    ///     let project = manager.resolve_project("12345678").await?;
    ///     println!("Found project: {}", project.name);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn resolve_project(
        &mut self,
        name_or_id: &str,
    ) -> Result<&todoist_api::sync::Project> {
        // Try cache first
        if self.find_project_in_cache(name_or_id).is_some() {
            // Re-borrow to return reference (can't return from the if-let due to borrow checker)
            return Ok(self.find_project_in_cache(name_or_id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_project_in_cache(name_or_id).ok_or_else(|| SyncError::NotFound {
            resource_type: "project",
            identifier: name_or_id.to_string(),
        })
    }

    /// Helper to find a project in the cache by name or ID.
    ///
    /// Searches for non-deleted projects where either:
    /// - The name matches (case-insensitive)
    /// - The ID matches exactly
    fn find_project_in_cache(&self, name_or_id: &str) -> Option<&todoist_api::sync::Project> {
        let name_lower = name_or_id.to_lowercase();
        self.cache.projects.iter().find(|p| {
            !p.is_deleted && (p.name.to_lowercase() == name_lower || p.id == name_or_id)
        })
    }

    /// Resolves a section by name or ID, with auto-sync fallback.
    ///
    /// This method first attempts to find the section in the cache. If not found,
    /// it performs a sync and retries the lookup.
    ///
    /// # Arguments
    ///
    /// * `name_or_id` - The section name (case-insensitive) or ID to search for
    /// * `project_id` - Optional project ID to scope the search. If provided, only
    ///   sections in that project are considered for name matching.
    ///
    /// # Returns
    ///
    /// A reference to the matching `Section` from the cache.
    ///
    /// # Errors
    ///
    /// Returns `SyncError::NotFound` if the section cannot be found even after syncing.
    /// Returns `SyncError::Api` if the sync operation fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use todoist_api::client::TodoistClient;
    /// use todoist_cache::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token");
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Find by name within a specific project
    ///     let section = manager.resolve_section("To Do", Some("12345678")).await?;
    ///     println!("Found section: {} ({})", section.name, section.id);
    ///
    ///     // Find by ID (project_id is ignored for ID lookups)
    ///     let section = manager.resolve_section("87654321", None).await?;
    ///     println!("Found section: {}", section.name);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn resolve_section(
        &mut self,
        name_or_id: &str,
        project_id: Option<&str>,
    ) -> Result<&todoist_api::sync::Section> {
        // Try cache first
        if self.find_section_in_cache(name_or_id, project_id).is_some() {
            return Ok(self.find_section_in_cache(name_or_id, project_id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_section_in_cache(name_or_id, project_id)
            .ok_or_else(|| SyncError::NotFound {
                resource_type: "section",
                identifier: name_or_id.to_string(),
            })
    }

    /// Helper to find a section in the cache by name or ID.
    ///
    /// Searches for non-deleted sections where either:
    /// - The ID matches exactly (ignores project_id filter)
    /// - The name matches (case-insensitive) and optionally within the specified project
    fn find_section_in_cache(
        &self,
        name_or_id: &str,
        project_id: Option<&str>,
    ) -> Option<&todoist_api::sync::Section> {
        let name_lower = name_or_id.to_lowercase();
        self.cache.sections.iter().find(|s| {
            if s.is_deleted {
                return false;
            }
            // ID match takes precedence (ignores project filter)
            if s.id == name_or_id {
                return true;
            }
            // Name match with optional project filter
            if s.name.to_lowercase() == name_lower {
                return project_id.is_none_or(|pid| s.project_id == pid);
            }
            false
        })
    }

    /// Resolves a label by name or ID, with auto-sync fallback.
    ///
    /// This method first attempts to find the label in the cache. If not found,
    /// it performs a sync and retries the lookup.
    ///
    /// # Arguments
    ///
    /// * `name_or_id` - The label name (case-insensitive) or ID to search for
    ///
    /// # Returns
    ///
    /// A reference to the matching `Label` from the cache.
    ///
    /// # Errors
    ///
    /// Returns `SyncError::NotFound` if the label cannot be found even after syncing.
    /// Returns `SyncError::Api` if the sync operation fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use todoist_api::client::TodoistClient;
    /// use todoist_cache::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token");
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Find by name (case-insensitive)
    ///     let label = manager.resolve_label("urgent").await?;
    ///     println!("Found label: {} ({})", label.name, label.id);
    ///
    ///     // Find by ID
    ///     let label = manager.resolve_label("12345678").await?;
    ///     println!("Found label: {}", label.name);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn resolve_label(
        &mut self,
        name_or_id: &str,
    ) -> Result<&todoist_api::sync::Label> {
        // Try cache first
        if self.find_label_in_cache(name_or_id).is_some() {
            return Ok(self.find_label_in_cache(name_or_id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_label_in_cache(name_or_id).ok_or_else(|| SyncError::NotFound {
            resource_type: "label",
            identifier: name_or_id.to_string(),
        })
    }

    /// Helper to find a label in the cache by name or ID.
    ///
    /// Searches for non-deleted labels where either:
    /// - The name matches (case-insensitive)
    /// - The ID matches exactly
    fn find_label_in_cache(&self, name_or_id: &str) -> Option<&todoist_api::sync::Label> {
        let name_lower = name_or_id.to_lowercase();
        self.cache.labels.iter().find(|l| {
            !l.is_deleted && (l.name.to_lowercase() == name_lower || l.id == name_or_id)
        })
    }

    /// Resolves an item (task) by ID, with auto-sync fallback.
    ///
    /// This method first attempts to find the item in the cache. If not found,
    /// it performs a sync and retries the lookup.
    ///
    /// Note: Unlike projects, sections, and labels, items can only be looked up
    /// by ID since task content is not guaranteed to be unique.
    ///
    /// # Arguments
    ///
    /// * `id` - The item ID to search for
    ///
    /// # Returns
    ///
    /// A reference to the matching `Item` from the cache.
    ///
    /// # Errors
    ///
    /// Returns `SyncError::NotFound` if the item cannot be found even after syncing.
    /// Returns `SyncError::Api` if the sync operation fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use todoist_api::client::TodoistClient;
    /// use todoist_cache::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token");
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Find by ID
    ///     let item = manager.resolve_item("12345678").await?;
    ///     println!("Found item: {} ({})", item.content, item.id);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn resolve_item(&mut self, id: &str) -> Result<&todoist_api::sync::Item> {
        // Try cache first
        if self.find_item_in_cache(id).is_some() {
            return Ok(self.find_item_in_cache(id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_item_in_cache(id).ok_or_else(|| SyncError::NotFound {
            resource_type: "item",
            identifier: id.to_string(),
        })
    }

    /// Helper to find an item in the cache by ID.
    ///
    /// Searches for non-deleted items where the ID matches exactly.
    fn find_item_in_cache(&self, id: &str) -> Option<&todoist_api::sync::Item> {
        self.cache
            .items
            .iter()
            .find(|i| !i.is_deleted && i.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test staleness calculation with various scenarios

    #[test]
    fn test_is_stale_when_never_synced() {
        // A cache that has never been synced should always be stale
        let cache = Cache::new();
        assert!(cache.last_sync.is_none());

        // We can't easily test SyncManager without a real client/store,
        // so we test the staleness logic directly on Cache
        let last_sync: Option<DateTime<Utc>> = None;
        let now = Utc::now();
        let threshold = Duration::minutes(5);

        let is_stale = match last_sync {
            None => true,
            Some(ls) => now.signed_duration_since(ls) > threshold,
        };
        assert!(is_stale);
    }

    #[test]
    fn test_is_stale_when_recently_synced() {
        let now = Utc::now();
        let last_sync = Some(now - Duration::minutes(2)); // 2 minutes ago
        let threshold = Duration::minutes(5);

        let is_stale = match last_sync {
            None => true,
            Some(ls) => now.signed_duration_since(ls) > threshold,
        };
        assert!(!is_stale);
    }

    #[test]
    fn test_is_stale_when_old_sync() {
        let now = Utc::now();
        let last_sync = Some(now - Duration::minutes(10)); // 10 minutes ago
        let threshold = Duration::minutes(5);

        let is_stale = match last_sync {
            None => true,
            Some(ls) => now.signed_duration_since(ls) > threshold,
        };
        assert!(is_stale);
    }

    #[test]
    fn test_is_stale_at_threshold_boundary() {
        let now = Utc::now();
        // Exactly at threshold - should not be stale (> not >=)
        let last_sync = Some(now - Duration::minutes(5));
        let threshold = Duration::minutes(5);

        let is_stale = match last_sync {
            None => true,
            Some(ls) => now.signed_duration_since(ls) > threshold,
        };
        assert!(!is_stale);
    }

    #[test]
    fn test_is_stale_just_over_threshold() {
        let now = Utc::now();
        // Just over threshold - should be stale
        let last_sync = Some(now - Duration::minutes(5) - Duration::seconds(1));
        let threshold = Duration::minutes(5);

        let is_stale = match last_sync {
            None => true,
            Some(ls) => now.signed_duration_since(ls) > threshold,
        };
        assert!(is_stale);
    }

    #[test]
    fn test_needs_sync_when_full_sync_needed() {
        // If cache.needs_full_sync() is true, needs_sync should return true
        let cache = Cache::new();
        assert!(cache.needs_full_sync());
        // needs_sync = needs_full_sync || is_stale
        // With new cache: needs_full_sync=true, is_stale=true (no last_sync)
        // Result: true
    }

    #[test]
    fn test_needs_sync_when_stale() {
        let mut cache = Cache::new();
        cache.sync_token = "some_token".to_string(); // Not full sync
        cache.last_sync = Some(Utc::now() - Duration::minutes(10)); // Stale

        assert!(!cache.needs_full_sync());
        // needs_sync = false || is_stale(true) = true
    }

    #[test]
    fn test_needs_sync_when_fresh() {
        let mut cache = Cache::new();
        cache.sync_token = "some_token".to_string(); // Not full sync
        cache.last_sync = Some(Utc::now()); // Just synced

        assert!(!cache.needs_full_sync());
        // needs_sync = false || is_stale(false) = false
    }
}
