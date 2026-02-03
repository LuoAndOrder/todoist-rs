//! Local cache for Todoist data.
//!
//! This crate provides a local cache that mirrors the Sync API response structure,
//! enabling efficient incremental updates and offline read access.
//!
//! # Storage
//!
//! The cache is stored on disk using XDG-compliant paths via [`CacheStore`]:
//! - Unix: `~/.cache/td/cache.json`
//! - macOS: `~/Library/Caches/td/cache.json`
//! - Windows: `C:\Users\<User>\AppData\Local\td\cache\cache.json`
//!
//! # Example
//!
//! ```no_run
//! use todoist_cache_rs::{Cache, CacheStore};
//!
//! // Create a store with the default XDG path
//! let store = CacheStore::new()?;
//!
//! // Load existing cache or create a new one
//! let mut cache = store.load_or_default()?;
//!
//! // Modify the cache...
//! cache.sync_token = "new_token".to_string();
//!
//! // Save changes to disk
//! store.save(&cache)?;
//! # Ok::<(), todoist_cache_rs::CacheStoreError>(())
//! ```

pub mod filter;
mod merge;
mod store;
mod sync_manager;

pub use store::{CacheStore, CacheStoreError, Result as CacheStoreResult};
pub use sync_manager::{Result as SyncResult, SyncError, SyncManager};

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use todoist_api_rs::sync::{
    Filter, Item, Label, Note, Project, ProjectNote, Reminder, Section, User,
};

/// Indexes for O(1) cache lookups.
///
/// These indexes are rebuilt after every sync operation and when loading
/// the cache from disk. They map IDs and lowercase names to indices in
/// the corresponding vectors, enabling fast lookups without linear searches.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CacheIndexes {
    /// Project ID -> index in projects vec.
    pub projects_by_id: HashMap<String, usize>,
    /// Lowercase project name -> index in projects vec.
    pub projects_by_name: HashMap<String, usize>,
    /// Section ID -> index in sections vec.
    pub sections_by_id: HashMap<String, usize>,
    /// Lowercase section name -> list of (project_id, index in sections vec).
    /// Multiple sections can have the same name across different projects.
    pub sections_by_name: HashMap<String, Vec<(String, usize)>>,
    /// Label ID -> index in labels vec.
    pub labels_by_id: HashMap<String, usize>,
    /// Lowercase label name -> index in labels vec.
    pub labels_by_name: HashMap<String, usize>,
    /// Item ID -> index in items vec.
    pub items_by_id: HashMap<String, usize>,
}

/// Local cache for Todoist data.
///
/// The cache structure mirrors the Sync API response for easy updates from sync operations.
/// It stores all relevant resources and metadata about the last sync.
///
/// # Thread Safety
///
/// `Cache` is [`Send`] and [`Sync`], but it has no internal synchronization.
/// Concurrent reads are safe, but concurrent writes or read-modify-write
/// patterns require external synchronization.
///
/// For multi-threaded access, wrap in `Arc<RwLock<Cache>>`:
///
/// ```
/// use std::sync::{Arc, RwLock};
/// use todoist_cache_rs::Cache;
///
/// let cache = Arc::new(RwLock::new(Cache::new()));
///
/// // Read access
/// let items_count = cache.read().unwrap().items.len();
///
/// // Write access
/// cache.write().unwrap().sync_token = "new_token".to_string();
/// ```
///
/// In typical CLI usage, the cache is owned by a single-threaded runtime
/// and external synchronization is not needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cache {
    /// The sync token for incremental syncs.
    /// Use "*" for a full sync or the stored token for incremental updates.
    pub sync_token: String,

    /// UTC timestamp when the last full sync was performed.
    /// This is set when a full sync completes successfully.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_sync_date_utc: Option<DateTime<Utc>>,

    /// UTC timestamp of the last successful sync (full or incremental).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<DateTime<Utc>>,

    /// Cached tasks (called "items" in the Sync API).
    #[serde(default)]
    pub items: Vec<Item>,

    /// Cached projects.
    #[serde(default)]
    pub projects: Vec<Project>,

    /// Cached personal labels.
    #[serde(default)]
    pub labels: Vec<Label>,

    /// Cached sections.
    #[serde(default)]
    pub sections: Vec<Section>,

    /// Cached task comments (called "notes" in the Sync API).
    #[serde(default)]
    pub notes: Vec<Note>,

    /// Cached project comments.
    #[serde(default)]
    pub project_notes: Vec<ProjectNote>,

    /// Cached reminders.
    #[serde(default)]
    pub reminders: Vec<Reminder>,

    /// Cached saved filters.
    #[serde(default)]
    pub filters: Vec<Filter>,

    /// Cached user information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,

    /// Indexes for fast lookups (rebuilt on sync, not serialized).
    #[serde(skip)]
    indexes: CacheIndexes,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    /// Creates a new empty cache with sync_token set to "*" for initial full sync.
    pub fn new() -> Self {
        Self {
            sync_token: "*".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: Vec::new(),
            projects: Vec::new(),
            labels: Vec::new(),
            sections: Vec::new(),
            notes: Vec::new(),
            project_notes: Vec::new(),
            reminders: Vec::new(),
            filters: Vec::new(),
            user: None,
            indexes: CacheIndexes::default(),
        }
    }

    /// Creates a new cache with provided data and rebuilds indexes.
    ///
    /// This is primarily useful for testing. The indexes are automatically
    /// rebuilt after construction.
    #[allow(clippy::too_many_arguments)]
    pub fn with_data(
        sync_token: String,
        full_sync_date_utc: Option<DateTime<Utc>>,
        last_sync: Option<DateTime<Utc>>,
        items: Vec<Item>,
        projects: Vec<Project>,
        labels: Vec<Label>,
        sections: Vec<Section>,
        notes: Vec<Note>,
        project_notes: Vec<ProjectNote>,
        reminders: Vec<Reminder>,
        filters: Vec<Filter>,
        user: Option<User>,
    ) -> Self {
        let mut cache = Self {
            sync_token,
            full_sync_date_utc,
            last_sync,
            items,
            projects,
            labels,
            sections,
            notes,
            project_notes,
            reminders,
            filters,
            user,
            indexes: CacheIndexes::default(),
        };
        cache.rebuild_indexes();
        cache
    }

    /// Rebuilds all lookup indexes from current cache data.
    ///
    /// This is called automatically after applying sync responses and should
    /// be called after loading the cache from disk.
    pub fn rebuild_indexes(&mut self) {
        let mut indexes = CacheIndexes::default();

        // Pre-allocate capacity for better performance
        indexes.projects_by_id.reserve(self.projects.len());
        indexes.projects_by_name.reserve(self.projects.len());
        indexes.sections_by_id.reserve(self.sections.len());
        indexes.sections_by_name.reserve(self.sections.len());
        indexes.labels_by_id.reserve(self.labels.len());
        indexes.labels_by_name.reserve(self.labels.len());
        indexes.items_by_id.reserve(self.items.len());

        // Index projects
        for (i, project) in self.projects.iter().enumerate() {
            if !project.is_deleted {
                indexes.projects_by_id.insert(project.id.clone(), i);
                indexes
                    .projects_by_name
                    .insert(project.name.to_lowercase(), i);
            }
        }

        // Index sections
        for (i, section) in self.sections.iter().enumerate() {
            if !section.is_deleted {
                indexes.sections_by_id.insert(section.id.clone(), i);
                indexes
                    .sections_by_name
                    .entry(section.name.to_lowercase())
                    .or_default()
                    .push((section.project_id.clone(), i));
            }
        }

        // Index labels
        for (i, label) in self.labels.iter().enumerate() {
            if !label.is_deleted {
                indexes.labels_by_id.insert(label.id.clone(), i);
                indexes.labels_by_name.insert(label.name.to_lowercase(), i);
            }
        }

        // Index items
        for (i, item) in self.items.iter().enumerate() {
            if !item.is_deleted {
                indexes.items_by_id.insert(item.id.clone(), i);
            }
        }

        self.indexes = indexes;
    }

    /// Find a project by ID or name (case-insensitive). O(1) lookup.
    pub fn find_project(&self, name_or_id: &str) -> Option<&Project> {
        // Try ID first (exact match)
        if let Some(&idx) = self.indexes.projects_by_id.get(name_or_id) {
            return self.projects.get(idx);
        }

        // Try lowercase name
        let name_lower = name_or_id.to_lowercase();
        if let Some(&idx) = self.indexes.projects_by_name.get(&name_lower) {
            return self.projects.get(idx);
        }

        None
    }

    /// Find a section by ID or name (case-insensitive) within a project. O(1) lookup.
    ///
    /// If `project_id` is provided, returns the section only if it belongs to that project.
    /// If `project_id` is `None` and there's exactly one match, returns it.
    pub fn find_section(&self, name_or_id: &str, project_id: Option<&str>) -> Option<&Section> {
        // Try ID first (exact match)
        if let Some(&idx) = self.indexes.sections_by_id.get(name_or_id) {
            let section = self.sections.get(idx)?;
            // If project_id is specified, verify it matches
            if project_id.is_none() || project_id == Some(section.project_id.as_str()) {
                return Some(section);
            }
        }

        // Try name (may have multiple matches across projects)
        let name_lower = name_or_id.to_lowercase();
        if let Some(matches) = self.indexes.sections_by_name.get(&name_lower) {
            // If project specified, filter by it
            if let Some(proj_id) = project_id {
                for (section_proj_id, idx) in matches {
                    if section_proj_id == proj_id {
                        return self.sections.get(*idx);
                    }
                }
            } else if matches.len() == 1 {
                // Unambiguous single match
                return self.sections.get(matches[0].1);
            }
        }

        None
    }

    /// Find a label by ID or name (case-insensitive). O(1) lookup.
    pub fn find_label(&self, name_or_id: &str) -> Option<&Label> {
        // Try ID first (exact match)
        if let Some(&idx) = self.indexes.labels_by_id.get(name_or_id) {
            return self.labels.get(idx);
        }

        // Try lowercase name
        let name_lower = name_or_id.to_lowercase();
        if let Some(&idx) = self.indexes.labels_by_name.get(&name_lower) {
            return self.labels.get(idx);
        }

        None
    }

    /// Find an item by ID. O(1) lookup.
    pub fn find_item(&self, id: &str) -> Option<&Item> {
        if let Some(&idx) = self.indexes.items_by_id.get(id) {
            return self.items.get(idx);
        }
        None
    }

    /// Returns true if the cache has never been synced (sync_token is "*").
    pub fn is_empty(&self) -> bool {
        self.sync_token == "*"
    }

    /// Returns true if the cache requires a full sync.
    /// This is true when the sync_token is "*".
    pub fn needs_full_sync(&self) -> bool {
        self.sync_token == "*"
    }

    /// Applies a sync response to the cache, merging in changes.
    ///
    /// This method handles both full and incremental sync responses:
    /// - Updates the sync token and timestamps
    /// - For full sync: replaces all resources with the response data
    /// - For incremental sync: merges changes (add/update/delete by ID)
    ///
    /// Resources with `is_deleted: true` are removed from the cache.
    ///
    /// # Arguments
    ///
    /// * `response` - The sync response from the Todoist API
    pub fn apply_sync_response(&mut self, response: &todoist_api_rs::sync::SyncResponse) {
        merge::apply_sync_response(self, response);
    }

    /// Applies a mutation response to the cache.
    ///
    /// This method is similar to `apply_sync_response()` but is specifically
    /// designed for write operation (mutation) responses. It:
    /// - Updates the sync_token from the response
    /// - Updates the last_sync timestamp
    /// - Merges any resources returned in the response (add/update/delete by ID)
    ///
    /// Unlike full sync responses, mutation responses always use incremental
    /// merge logic since they only contain affected resources.
    ///
    /// Note: The `temp_id_mapping` from the response should be used by the caller
    /// to resolve temporary IDs before calling this method, or the caller can
    /// use the returned response's `temp_id_mapping` to look up real IDs.
    ///
    /// # Arguments
    ///
    /// * `response` - The sync response from a mutation (write) operation
    pub fn apply_mutation_response(&mut self, response: &todoist_api_rs::sync::SyncResponse) {
        merge::apply_mutation_response(self, response);
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
