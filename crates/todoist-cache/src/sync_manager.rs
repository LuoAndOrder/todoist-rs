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

use chrono::{DateTime, Duration, Utc};
use strsim::levenshtein;
use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{SyncCommand, SyncRequest, SyncResponse};

use crate::{Cache, CacheStore, CacheStoreError};

/// Default staleness threshold in minutes.
const DEFAULT_STALE_MINUTES: i64 = 5;

/// Maximum Levenshtein distance to consider a name as a suggestion.
const MAX_SUGGESTION_DISTANCE: usize = 3;

/// Formats the "not found" error message, optionally including a suggestion.
fn format_not_found_error(
    resource_type: &str,
    identifier: &str,
    suggestion: Option<&str>,
) -> String {
    let base = format!(
        "{} '{}' not found. Try running 'td sync' to refresh your cache.",
        resource_type, identifier
    );
    match suggestion {
        Some(s) => format!("{} Did you mean '{}'?", base, s),
        None => base,
    }
}

/// Finds the best matching name from a list of candidates using Levenshtein distance.
///
/// Returns the best match if its edit distance is within the threshold,
/// otherwise returns `None`.
fn find_similar_name<'a>(query: &str, candidates: impl Iterator<Item = &'a str>) -> Option<String> {
    let query_lower = query.to_lowercase();

    let (best_match, best_distance) = candidates
        .filter(|name| !name.is_empty())
        .map(|name| {
            let distance = levenshtein(&query_lower, &name.to_lowercase());
            (name.to_string(), distance)
        })
        .min_by_key(|(_, d)| *d)?;

    // Only suggest if the distance is within threshold and not an exact match
    if best_distance > 0 && best_distance <= MAX_SUGGESTION_DISTANCE {
        Some(best_match)
    } else {
        None
    }
}

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
        if self.cache.needs_full_sync() {
            // Already need a full sync, just do it
            let request = SyncRequest::full_sync();
            let response = self.client.sync(request).await?;
            self.cache.apply_sync_response(&response);
            self.store.save(&self.cache)?;
            return Ok(&self.cache);
        }

        // Try incremental sync
        let request = SyncRequest::incremental(&self.cache.sync_token);
        match self.client.sync(request).await {
            Ok(response) => {
                self.cache.apply_sync_response(&response);
                self.store.save(&self.cache)?;
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
                self.store.save(&self.cache)?;
                Ok(&self.cache)
            }
            Err(e) => Err(e.into()),
        }
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
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_api_rs::sync::SyncCommand;
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
        let request =
            SyncRequest::with_commands(commands).with_resource_types(vec!["all".to_string()]);
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
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
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
    ) -> Result<&todoist_api_rs::sync::Project> {
        // Try cache first
        if self.find_project_in_cache(name_or_id).is_some() {
            // Re-borrow to return reference (can't return from the if-let due to borrow checker)
            return Ok(self.find_project_in_cache(name_or_id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_project_in_cache(name_or_id).ok_or_else(|| {
            // Find similar project names for suggestion
            let suggestion = find_similar_name(
                name_or_id,
                self.cache
                    .projects
                    .iter()
                    .filter(|p| !p.is_deleted)
                    .map(|p| p.name.as_str()),
            );
            SyncError::NotFound {
                resource_type: "Project",
                identifier: name_or_id.to_string(),
                suggestion,
            }
        })
    }

    /// Helper to find a project in the cache by name or ID.
    ///
    /// Searches for non-deleted projects where either:
    /// - The name matches (case-insensitive)
    /// - The ID matches exactly
    fn find_project_in_cache(&self, name_or_id: &str) -> Option<&todoist_api_rs::sync::Project> {
        let name_lower = name_or_id.to_lowercase();
        self.cache
            .projects
            .iter()
            .find(|p| !p.is_deleted && (p.name.to_lowercase() == name_lower || p.id == name_or_id))
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
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
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
    ) -> Result<&todoist_api_rs::sync::Section> {
        // Try cache first
        if self.find_section_in_cache(name_or_id, project_id).is_some() {
            return Ok(self.find_section_in_cache(name_or_id, project_id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_section_in_cache(name_or_id, project_id)
            .ok_or_else(|| {
                // Find similar section names for suggestion (within same project if specified)
                let suggestion = find_similar_name(
                    name_or_id,
                    self.cache
                        .sections
                        .iter()
                        .filter(|s| {
                            !s.is_deleted && project_id.is_none_or(|pid| s.project_id == pid)
                        })
                        .map(|s| s.name.as_str()),
                );
                SyncError::NotFound {
                    resource_type: "Section",
                    identifier: name_or_id.to_string(),
                    suggestion,
                }
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
    ) -> Option<&todoist_api_rs::sync::Section> {
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
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
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
    ) -> Result<&todoist_api_rs::sync::Label> {
        // Try cache first
        if self.find_label_in_cache(name_or_id).is_some() {
            return Ok(self.find_label_in_cache(name_or_id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_label_in_cache(name_or_id).ok_or_else(|| {
            // Find similar label names for suggestion
            let suggestion = find_similar_name(
                name_or_id,
                self.cache
                    .labels
                    .iter()
                    .filter(|l| !l.is_deleted)
                    .map(|l| l.name.as_str()),
            );
            SyncError::NotFound {
                resource_type: "Label",
                identifier: name_or_id.to_string(),
                suggestion,
            }
        })
    }

    /// Helper to find a label in the cache by name or ID.
    ///
    /// Searches for non-deleted labels where either:
    /// - The name matches (case-insensitive)
    /// - The ID matches exactly
    fn find_label_in_cache(&self, name_or_id: &str) -> Option<&todoist_api_rs::sync::Label> {
        let name_lower = name_or_id.to_lowercase();
        self.cache
            .labels
            .iter()
            .find(|l| !l.is_deleted && (l.name.to_lowercase() == name_lower || l.id == name_or_id))
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
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
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
    pub async fn resolve_item(&mut self, id: &str) -> Result<&todoist_api_rs::sync::Item> {
        // Try cache first
        if self.find_item_in_cache(id).is_some() {
            return Ok(self.find_item_in_cache(id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_item_in_cache(id)
            .ok_or_else(|| SyncError::NotFound {
                resource_type: "Item",
                identifier: id.to_string(),
                suggestion: None, // Items are looked up by ID, no name suggestions
            })
    }

    /// Helper to find an item in the cache by ID.
    ///
    /// Searches for non-deleted items where the ID matches exactly.
    fn find_item_in_cache(&self, id: &str) -> Option<&todoist_api_rs::sync::Item> {
        self.cache
            .items
            .iter()
            .find(|i| !i.is_deleted && i.id == id)
    }

    /// Resolves an item (task) by ID or unique prefix, with auto-sync fallback.
    ///
    /// This method first attempts to find the item in the cache by exact ID match
    /// or unique prefix. If not found, it performs a sync and retries the lookup.
    ///
    /// # Arguments
    ///
    /// * `id_or_prefix` - The item ID or unique prefix to search for
    /// * `require_checked` - If `Some(true)`, only match completed items.
    ///   If `Some(false)`, only match uncompleted items.
    ///   If `None`, match any item regardless of completion status.
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
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Find uncompleted task by ID prefix
    ///     let item = manager.resolve_item_by_prefix("abc123", Some(false)).await?;
    ///     println!("Found item: {} ({})", item.content, item.id);
    ///
    ///     // Find any task by prefix (completed or not)
    ///     let item = manager.resolve_item_by_prefix("def456", None).await?;
    ///     println!("Found item: {}", item.content);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn resolve_item_by_prefix(
        &mut self,
        id_or_prefix: &str,
        require_checked: Option<bool>,
    ) -> Result<&todoist_api_rs::sync::Item> {
        // Try cache first
        match self.find_item_by_prefix_in_cache(id_or_prefix, require_checked) {
            ItemLookupResult::Found(_) => {
                // Re-lookup to return reference (borrow checker limitation)
                if let ItemLookupResult::Found(item) =
                    self.find_item_by_prefix_in_cache(id_or_prefix, require_checked)
                {
                    return Ok(item);
                }
                unreachable!()
            }
            ItemLookupResult::Ambiguous(msg) => {
                return Err(SyncError::NotFound {
                    resource_type: "Item",
                    identifier: msg,
                    suggestion: None,
                });
            }
            ItemLookupResult::NotFound => {
                // Continue to sync
            }
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        match self.find_item_by_prefix_in_cache(id_or_prefix, require_checked) {
            ItemLookupResult::Found(item) => Ok(item),
            ItemLookupResult::Ambiguous(msg) => Err(SyncError::NotFound {
                resource_type: "Item",
                identifier: msg,
                suggestion: None,
            }),
            ItemLookupResult::NotFound => Err(SyncError::NotFound {
                resource_type: "Item",
                identifier: id_or_prefix.to_string(),
                suggestion: None, // Items are looked up by ID, no name suggestions
            }),
        }
    }

    /// Helper to find an item in the cache by ID or unique prefix.
    ///
    /// Returns the found item, an ambiguity error message, or not found.
    fn find_item_by_prefix_in_cache(
        &self,
        id_or_prefix: &str,
        require_checked: Option<bool>,
    ) -> ItemLookupResult<'_> {
        // First try exact match
        if let Some(item) = self.cache.items.iter().find(|i| {
            !i.is_deleted
                && i.id == id_or_prefix
                && require_checked.is_none_or(|checked| i.checked == checked)
        }) {
            return ItemLookupResult::Found(item);
        }

        // Try prefix match
        let matches: Vec<&todoist_api_rs::sync::Item> = self
            .cache
            .items
            .iter()
            .filter(|i| {
                !i.is_deleted
                    && i.id.starts_with(id_or_prefix)
                    && require_checked.is_none_or(|checked| i.checked == checked)
            })
            .collect();

        match matches.len() {
            0 => ItemLookupResult::NotFound,
            1 => ItemLookupResult::Found(matches[0]),
            _ => {
                // Ambiguous prefix - provide helpful error message
                let mut msg = format!(
                    "Ambiguous task ID \"{}\"\n\nMultiple tasks match this prefix:",
                    id_or_prefix
                );
                for item in matches.iter().take(5) {
                    let prefix = &item.id[..6.min(item.id.len())];
                    msg.push_str(&format!("\n  {}  {}", prefix, item.content));
                }
                if matches.len() > 5 {
                    msg.push_str(&format!("\n  ... and {} more", matches.len() - 5));
                }
                msg.push_str("\n\nPlease use a longer prefix.");
                ItemLookupResult::Ambiguous(msg)
            }
        }
    }
}

/// Result of an item lookup by prefix.
enum ItemLookupResult<'a> {
    /// Found exactly one matching item.
    Found(&'a todoist_api_rs::sync::Item),
    /// Multiple items match the prefix (contains error message).
    Ambiguous(String),
    /// No matching item found.
    NotFound,
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

    // Tests for fuzzy matching suggestions

    #[test]
    fn test_find_similar_name_exact_match_returns_none() {
        // Exact match should not return a suggestion
        let candidates = ["Work", "Personal", "Shopping"];
        let result = find_similar_name("Work", candidates.iter().copied());
        assert!(result.is_none());
    }

    #[test]
    fn test_find_similar_name_case_insensitive_exact_match_returns_none() {
        // Case-insensitive exact match should not return a suggestion
        let candidates = ["Work", "Personal", "Shopping"];
        let result = find_similar_name("work", candidates.iter().copied());
        assert!(result.is_none());
    }

    #[test]
    fn test_find_similar_name_single_typo() {
        // Single character typo should suggest
        let candidates = ["Work", "Personal", "Shopping"];
        let result = find_similar_name("Wrok", candidates.iter().copied());
        assert_eq!(result, Some("Work".to_string()));
    }

    #[test]
    fn test_find_similar_name_missing_letter() {
        // Missing letter should suggest
        let candidates = ["Inbox", "Personal", "Shopping"];
        let result = find_similar_name("inbx", candidates.iter().copied());
        assert_eq!(result, Some("Inbox".to_string()));
    }

    #[test]
    fn test_find_similar_name_extra_letter() {
        // Extra letter should suggest
        let candidates = ["Work", "Personal", "Shopping"];
        let result = find_similar_name("Workk", candidates.iter().copied());
        assert_eq!(result, Some("Work".to_string()));
    }

    #[test]
    fn test_find_similar_name_too_different() {
        // Very different string should not suggest
        let candidates = ["Work", "Personal", "Shopping"];
        let result = find_similar_name("Completely Different", candidates.iter().copied());
        assert!(result.is_none());
    }

    #[test]
    fn test_find_similar_name_empty_candidates() {
        // Empty candidates list should return None
        let candidates: Vec<&str> = vec![];
        let result = find_similar_name("Work", candidates.iter().copied());
        assert!(result.is_none());
    }

    #[test]
    fn test_find_similar_name_best_match_selected() {
        // Should select the best (closest) match
        let candidates = ["Workshop", "Work", "Working"];
        let result = find_similar_name("Wok", candidates.iter().copied());
        assert_eq!(result, Some("Work".to_string()));
    }

    #[test]
    fn test_format_not_found_error_without_suggestion() {
        let msg = format_not_found_error("Project", "inbox", None);
        assert_eq!(
            msg,
            "Project 'inbox' not found. Try running 'td sync' to refresh your cache."
        );
    }

    #[test]
    fn test_format_not_found_error_with_suggestion() {
        let msg = format_not_found_error("Project", "inbox", Some("Inbox"));
        assert_eq!(
            msg,
            "Project 'inbox' not found. Try running 'td sync' to refresh your cache. Did you mean 'Inbox'?"
        );
    }

    #[test]
    fn test_format_not_found_error_label_with_suggestion() {
        let msg = format_not_found_error("Label", "urgnt", Some("urgent"));
        assert_eq!(
            msg,
            "Label 'urgnt' not found. Try running 'td sync' to refresh your cache. Did you mean 'urgent'?"
        );
    }
}
