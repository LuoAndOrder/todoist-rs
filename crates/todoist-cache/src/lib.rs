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
//! use todoist_cache::{Cache, CacheStore};
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
//! # Ok::<(), todoist_cache::CacheStoreError>(())
//! ```

pub mod filter;
mod store;
mod sync_manager;

pub use store::{CacheStore, CacheStoreError, Result as CacheStoreResult};
pub use sync_manager::{Result as SyncResult, SyncError, SyncManager};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use todoist_api::sync::{
    Filter, Item, Label, Note, Project, ProjectNote, Reminder, Section, User,
};

/// Local cache for Todoist data.
///
/// The cache structure mirrors the Sync API response for easy updates from sync operations.
/// It stores all relevant resources and metadata about the last sync.
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
        }
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
    pub fn apply_sync_response(&mut self, response: &todoist_api::sync::SyncResponse) {
        let now = Utc::now();

        // Update sync token
        self.sync_token = response.sync_token.clone();
        self.last_sync = Some(now);

        // If this is a full sync, update full_sync_date_utc
        if response.full_sync {
            // Use the server-provided timestamp if available, otherwise use current time
            self.full_sync_date_utc = response
                .full_sync_date_utc
                .as_ref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .or(Some(now));
        }

        if response.full_sync {
            // Full sync: replace all data (filter out deleted items)
            self.items = response
                .items
                .iter()
                .filter(|i| !i.is_deleted)
                .cloned()
                .collect();
            self.projects = response
                .projects
                .iter()
                .filter(|p| !p.is_deleted)
                .cloned()
                .collect();
            self.labels = response
                .labels
                .iter()
                .filter(|l| !l.is_deleted)
                .cloned()
                .collect();
            self.sections = response
                .sections
                .iter()
                .filter(|s| !s.is_deleted)
                .cloned()
                .collect();
            self.notes = response
                .notes
                .iter()
                .filter(|n| !n.is_deleted)
                .cloned()
                .collect();
            self.project_notes = response
                .project_notes
                .iter()
                .filter(|n| !n.is_deleted)
                .cloned()
                .collect();
            self.reminders = response
                .reminders
                .iter()
                .filter(|r| !r.is_deleted)
                .cloned()
                .collect();
            self.filters = response
                .filters
                .iter()
                .filter(|f| !f.is_deleted)
                .cloned()
                .collect();
        } else {
            // Incremental sync: merge changes
            Self::merge_resources(&mut self.items, &response.items, |i| &i.id, |i| i.is_deleted);
            Self::merge_resources(
                &mut self.projects,
                &response.projects,
                |p| &p.id,
                |p| p.is_deleted,
            );
            Self::merge_resources(
                &mut self.labels,
                &response.labels,
                |l| &l.id,
                |l| l.is_deleted,
            );
            Self::merge_resources(
                &mut self.sections,
                &response.sections,
                |s| &s.id,
                |s| s.is_deleted,
            );
            Self::merge_resources(&mut self.notes, &response.notes, |n| &n.id, |n| n.is_deleted);
            Self::merge_resources(
                &mut self.project_notes,
                &response.project_notes,
                |n| &n.id,
                |n| n.is_deleted,
            );
            Self::merge_resources(
                &mut self.reminders,
                &response.reminders,
                |r| &r.id,
                |r| r.is_deleted,
            );
            Self::merge_resources(
                &mut self.filters,
                &response.filters,
                |f| &f.id,
                |f| f.is_deleted,
            );
        }

        // User is always replaced if present in response
        if response.user.is_some() {
            self.user = response.user.clone();
        }
    }

    /// Merges a list of resources from a sync response into the cache.
    ///
    /// For each resource in the response:
    /// - If `is_deleted` is true: remove from cache
    /// - If resource exists in cache: update it
    /// - Otherwise: add it
    fn merge_resources<T, F, D>(existing: &mut Vec<T>, incoming: &[T], get_id: F, is_deleted: D)
    where
        T: Clone,
        F: Fn(&T) -> &String,
        D: Fn(&T) -> bool,
    {
        for item in incoming {
            let id = get_id(item);
            let pos = existing.iter().position(|e| get_id(e) == id);

            if is_deleted(item) {
                // Remove if deleted
                if let Some(idx) = pos {
                    existing.remove(idx);
                }
            } else if let Some(idx) = pos {
                // Update existing
                existing[idx] = item.clone();
            } else {
                // Add new
                existing.push(item.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn test_cache_new_defaults() {
        let cache = Cache::new();

        assert_eq!(cache.sync_token, "*");
        assert!(cache.full_sync_date_utc.is_none());
        assert!(cache.last_sync.is_none());
        assert!(cache.items.is_empty());
        assert!(cache.projects.is_empty());
        assert!(cache.labels.is_empty());
        assert!(cache.sections.is_empty());
        assert!(cache.notes.is_empty());
        assert!(cache.project_notes.is_empty());
        assert!(cache.reminders.is_empty());
        assert!(cache.filters.is_empty());
        assert!(cache.user.is_none());
    }

    #[test]
    fn test_cache_default_impl() {
        let cache = Cache::default();
        assert_eq!(cache.sync_token, "*");
        assert!(cache.is_empty());
        assert!(cache.needs_full_sync());
    }

    #[test]
    fn test_cache_is_empty() {
        let mut cache = Cache::new();
        assert!(cache.is_empty());

        cache.sync_token = "token123".to_string();
        assert!(!cache.is_empty());
    }

    #[test]
    fn test_cache_needs_full_sync() {
        let mut cache = Cache::new();
        assert!(cache.needs_full_sync());

        cache.sync_token = "token123".to_string();
        assert!(!cache.needs_full_sync());
    }

    #[test]
    fn test_cache_serde_roundtrip_empty() {
        let cache = Cache::new();

        let json = serde_json::to_string(&cache).unwrap();
        let deserialized: Cache = serde_json::from_str(&json).unwrap();

        assert_eq!(cache, deserialized);
    }

    #[test]
    fn test_cache_serde_roundtrip_with_data() {
        let now = Utc::now();
        let cache = Cache {
            sync_token: "abc123token".to_string(),
            full_sync_date_utc: Some(now),
            last_sync: Some(now),
            items: vec![Item {
                id: "item-1".to_string(),
                user_id: None,
                project_id: "proj-1".to_string(),
                content: "Buy milk".to_string(),
                description: "From the store".to_string(),
                priority: 1,
                due: None,
                deadline: None,
                parent_id: None,
                child_order: 0,
                section_id: None,
                day_order: 0,
                is_collapsed: false,
                labels: vec!["shopping".to_string()],
                added_by_uid: None,
                assigned_by_uid: None,
                responsible_uid: None,
                checked: false,
                is_deleted: false,
                added_at: None,
                updated_at: None,
                completed_at: None,
                duration: None,
            }],
            projects: vec![Project {
                id: "proj-1".to_string(),
                name: "Personal".to_string(),
                color: Some("blue".to_string()),
                parent_id: None,
                child_order: 0,
                is_collapsed: false,
                shared: false,
                can_assign_tasks: false,
                is_deleted: false,
                is_archived: false,
                is_favorite: true,
                view_style: Some("list".to_string()),
                inbox_project: false,
                folder_id: None,
                created_at: None,
                updated_at: None,
            }],
            labels: vec![Label {
                id: "label-1".to_string(),
                name: "shopping".to_string(),
                color: Some("green".to_string()),
                item_order: 0,
                is_deleted: false,
                is_favorite: false,
            }],
            sections: vec![Section {
                id: "section-1".to_string(),
                name: "Groceries".to_string(),
                project_id: "proj-1".to_string(),
                section_order: 0,
                is_collapsed: false,
                is_deleted: false,
                is_archived: false,
                archived_at: None,
                added_at: None,
                updated_at: None,
            }],
            notes: vec![Note {
                id: "note-1".to_string(),
                item_id: "item-1".to_string(),
                content: "Remember expiration date".to_string(),
                posted_at: None,
                is_deleted: false,
                posted_uid: None,
                file_attachment: None,
            }],
            project_notes: Vec::new(),
            reminders: vec![Reminder {
                id: "reminder-1".to_string(),
                item_id: "item-1".to_string(),
                reminder_type: "relative".to_string(),
                due: None,
                minute_offset: Some(30),
                is_deleted: false,
            }],
            filters: vec![Filter {
                id: "filter-1".to_string(),
                name: "Today".to_string(),
                query: "today | overdue".to_string(),
                color: Some("red".to_string()),
                item_order: 0,
                is_deleted: false,
                is_favorite: true,
            }],
            user: Some(User {
                id: "user-1".to_string(),
                email: Some("test@example.com".to_string()),
                full_name: Some("Test User".to_string()),
                timezone: Some("America/New_York".to_string()),
                inbox_project_id: Some("inbox-123".to_string()),
                start_page: None,
                start_day: None,
                date_format: None,
                time_format: None,
                is_premium: false,
            }),
        };

        let json = serde_json::to_string_pretty(&cache).unwrap();
        let deserialized: Cache = serde_json::from_str(&json).unwrap();

        assert_eq!(cache, deserialized);
    }

    #[test]
    fn test_cache_deserialize_minimal() {
        let json = r#"{
            "sync_token": "token123"
        }"#;

        let cache: Cache = serde_json::from_str(json).unwrap();
        assert_eq!(cache.sync_token, "token123");
        assert!(cache.items.is_empty());
        assert!(cache.projects.is_empty());
        assert!(cache.user.is_none());
    }

    #[test]
    fn test_cache_deserialize_with_timestamps() {
        let json = r#"{
            "sync_token": "token123",
            "full_sync_date_utc": "2025-01-25T10:30:00Z",
            "last_sync": "2025-01-25T12:00:00Z"
        }"#;

        let cache: Cache = serde_json::from_str(json).unwrap();
        assert_eq!(cache.sync_token, "token123");
        assert!(cache.full_sync_date_utc.is_some());
        assert!(cache.last_sync.is_some());

        // Verify the timestamps are parsed correctly
        let full_sync = cache.full_sync_date_utc.unwrap();
        assert_eq!(full_sync.hour(), 10);
        assert_eq!(full_sync.minute(), 30);
    }

    #[test]
    fn test_cache_serialize_skips_none_values() {
        let cache = Cache::new();
        let json = serde_json::to_string(&cache).unwrap();

        // Should not contain full_sync_date_utc or last_sync when None
        assert!(!json.contains("full_sync_date_utc"));
        assert!(!json.contains("last_sync"));
        assert!(!json.contains("user"));
    }

    #[test]
    fn test_cache_clone() {
        let cache = Cache {
            sync_token: "token".to_string(),
            full_sync_date_utc: Some(Utc::now()),
            last_sync: Some(Utc::now()),
            items: vec![],
            projects: vec![],
            labels: vec![],
            sections: vec![],
            notes: vec![],
            project_notes: vec![],
            reminders: vec![],
            filters: vec![],
            user: None,
        };

        let cloned = cache.clone();
        assert_eq!(cache, cloned);
    }

    // Helper functions for creating test resources
    mod test_helpers {
        use super::*;
        use todoist_api::sync::SyncResponse;
        use std::collections::HashMap;

        pub fn make_item(id: &str, content: &str, is_deleted: bool) -> Item {
            Item {
                id: id.to_string(),
                user_id: None,
                project_id: "proj-1".to_string(),
                content: content.to_string(),
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
                is_deleted,
                added_at: None,
                updated_at: None,
                completed_at: None,
                duration: None,
            }
        }

        pub fn make_project(id: &str, name: &str, is_deleted: bool) -> Project {
            Project {
                id: id.to_string(),
                name: name.to_string(),
                color: None,
                parent_id: None,
                child_order: 0,
                is_collapsed: false,
                shared: false,
                can_assign_tasks: false,
                is_deleted,
                is_archived: false,
                is_favorite: false,
                view_style: None,
                inbox_project: false,
                folder_id: None,
                created_at: None,
                updated_at: None,
            }
        }

        pub fn make_label(id: &str, name: &str, is_deleted: bool) -> Label {
            Label {
                id: id.to_string(),
                name: name.to_string(),
                color: None,
                item_order: 0,
                is_deleted,
                is_favorite: false,
            }
        }

        pub fn make_section(id: &str, name: &str, is_deleted: bool) -> Section {
            Section {
                id: id.to_string(),
                name: name.to_string(),
                project_id: "proj-1".to_string(),
                section_order: 0,
                is_collapsed: false,
                is_deleted,
                is_archived: false,
                archived_at: None,
                added_at: None,
                updated_at: None,
            }
        }

        pub fn make_note(id: &str, content: &str, is_deleted: bool) -> Note {
            Note {
                id: id.to_string(),
                item_id: "item-1".to_string(),
                content: content.to_string(),
                posted_at: None,
                is_deleted,
                posted_uid: None,
                file_attachment: None,
            }
        }

        pub fn make_reminder(id: &str, is_deleted: bool) -> Reminder {
            Reminder {
                id: id.to_string(),
                item_id: "item-1".to_string(),
                reminder_type: "relative".to_string(),
                due: None,
                minute_offset: Some(30),
                is_deleted,
            }
        }

        pub fn make_filter(id: &str, name: &str, is_deleted: bool) -> Filter {
            Filter {
                id: id.to_string(),
                name: name.to_string(),
                query: "today".to_string(),
                color: None,
                item_order: 0,
                is_deleted,
                is_favorite: false,
            }
        }

        pub fn make_user(id: &str) -> User {
            User {
                id: id.to_string(),
                email: Some("test@example.com".to_string()),
                full_name: Some("Test User".to_string()),
                timezone: Some("UTC".to_string()),
                inbox_project_id: None,
                start_page: None,
                start_day: None,
                date_format: None,
                time_format: None,
                is_premium: false,
            }
        }

        pub fn make_sync_response(full_sync: bool, sync_token: &str) -> SyncResponse {
            SyncResponse {
                sync_token: sync_token.to_string(),
                full_sync,
                full_sync_date_utc: if full_sync {
                    Some("2025-01-25T10:00:00Z".to_string())
                } else {
                    None
                },
                items: vec![],
                projects: vec![],
                labels: vec![],
                sections: vec![],
                notes: vec![],
                project_notes: vec![],
                reminders: vec![],
                filters: vec![],
                user: None,
                collaborators: vec![],
                collaborator_states: vec![],
                sync_status: HashMap::new(),
                temp_id_mapping: HashMap::new(),
                day_orders: None,
                live_notifications: vec![],
                live_notifications_last_read_id: None,
                user_settings: None,
                user_plan_limits: None,
                stats: None,
                completed_info: vec![],
                locations: vec![],
            }
        }
    }

    // ==================== Full Sync Tests ====================

    #[test]
    fn test_apply_full_sync_updates_sync_token() {
        use test_helpers::*;

        let mut cache = Cache::new();
        let response = make_sync_response(true, "new_token_123");

        cache.apply_sync_response(&response);

        assert_eq!(cache.sync_token, "new_token_123");
        assert!(cache.last_sync.is_some());
        assert!(cache.full_sync_date_utc.is_some());
    }

    #[test]
    fn test_apply_full_sync_replaces_all_items() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.items = vec![make_item("old-1", "Old task", false)];

        let mut response = make_sync_response(true, "token");
        response.items = vec![
            make_item("new-1", "New task 1", false),
            make_item("new-2", "New task 2", false),
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.items.len(), 2);
        assert_eq!(cache.items[0].id, "new-1");
        assert_eq!(cache.items[1].id, "new-2");
    }

    #[test]
    fn test_apply_full_sync_filters_deleted_items() {
        use test_helpers::*;

        let mut cache = Cache::new();
        let mut response = make_sync_response(true, "token");
        response.items = vec![
            make_item("item-1", "Active task", false),
            make_item("item-2", "Deleted task", true),
            make_item("item-3", "Another active", false),
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.items.len(), 2);
        assert!(cache.items.iter().all(|i| !i.is_deleted));
        assert!(cache.items.iter().any(|i| i.id == "item-1"));
        assert!(cache.items.iter().any(|i| i.id == "item-3"));
    }

    #[test]
    fn test_apply_full_sync_replaces_all_projects() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.projects = vec![make_project("old-proj", "Old Project", false)];

        let mut response = make_sync_response(true, "token");
        response.projects = vec![
            make_project("proj-1", "Project 1", false),
            make_project("proj-2", "Project 2", false),
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.projects.len(), 2);
        assert!(cache.projects.iter().any(|p| p.id == "proj-1"));
        assert!(cache.projects.iter().any(|p| p.id == "proj-2"));
    }

    #[test]
    fn test_apply_full_sync_filters_deleted_projects() {
        use test_helpers::*;

        let mut cache = Cache::new();
        let mut response = make_sync_response(true, "token");
        response.projects = vec![
            make_project("proj-1", "Active", false),
            make_project("proj-2", "Deleted", true),
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.projects.len(), 1);
        assert_eq!(cache.projects[0].id, "proj-1");
    }

    #[test]
    fn test_apply_full_sync_replaces_all_labels() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.labels = vec![make_label("old-label", "Old", false)];

        let mut response = make_sync_response(true, "token");
        response.labels = vec![
            make_label("label-1", "Work", false),
            make_label("label-2", "Personal", false),
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.labels.len(), 2);
    }

    #[test]
    fn test_apply_full_sync_replaces_all_sections() {
        use test_helpers::*;

        let mut cache = Cache::new();
        let mut response = make_sync_response(true, "token");
        response.sections = vec![
            make_section("sec-1", "Section 1", false),
            make_section("sec-2", "Section 2", true), // deleted
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.sections.len(), 1);
        assert_eq!(cache.sections[0].id, "sec-1");
    }

    #[test]
    fn test_apply_full_sync_replaces_all_notes() {
        use test_helpers::*;

        let mut cache = Cache::new();
        let mut response = make_sync_response(true, "token");
        response.notes = vec![
            make_note("note-1", "Comment 1", false),
            make_note("note-2", "Deleted comment", true),
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.notes.len(), 1);
        assert_eq!(cache.notes[0].id, "note-1");
    }

    #[test]
    fn test_apply_full_sync_replaces_all_reminders() {
        use test_helpers::*;

        let mut cache = Cache::new();
        let mut response = make_sync_response(true, "token");
        response.reminders = vec![
            make_reminder("rem-1", false),
            make_reminder("rem-2", true), // deleted
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.reminders.len(), 1);
        assert_eq!(cache.reminders[0].id, "rem-1");
    }

    #[test]
    fn test_apply_full_sync_replaces_all_filters() {
        use test_helpers::*;

        let mut cache = Cache::new();
        let mut response = make_sync_response(true, "token");
        response.filters = vec![
            make_filter("filter-1", "Today", false),
            make_filter("filter-2", "Deleted", true),
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.filters.len(), 1);
        assert_eq!(cache.filters[0].id, "filter-1");
    }

    #[test]
    fn test_apply_full_sync_updates_user() {
        use test_helpers::*;

        let mut cache = Cache::new();
        let mut response = make_sync_response(true, "token");
        response.user = Some(make_user("user-1"));

        cache.apply_sync_response(&response);

        assert!(cache.user.is_some());
        assert_eq!(cache.user.as_ref().unwrap().id, "user-1");
    }

    // ==================== Incremental Sync Tests ====================

    #[test]
    fn test_apply_incremental_sync_adds_new_items() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.sync_token = "old_token".to_string();
        cache.items = vec![make_item("item-1", "Existing", false)];

        let mut response = make_sync_response(false, "new_token");
        response.items = vec![make_item("item-2", "New task", false)];

        cache.apply_sync_response(&response);

        assert_eq!(cache.items.len(), 2);
        assert!(cache.items.iter().any(|i| i.id == "item-1"));
        assert!(cache.items.iter().any(|i| i.id == "item-2"));
    }

    #[test]
    fn test_apply_incremental_sync_updates_existing_items() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.items = vec![make_item("item-1", "Original content", false)];

        let mut response = make_sync_response(false, "token");
        response.items = vec![make_item("item-1", "Updated content", false)];

        cache.apply_sync_response(&response);

        assert_eq!(cache.items.len(), 1);
        assert_eq!(cache.items[0].content, "Updated content");
    }

    #[test]
    fn test_apply_incremental_sync_removes_deleted_items() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.items = vec![
            make_item("item-1", "Task 1", false),
            make_item("item-2", "Task 2", false),
        ];

        let mut response = make_sync_response(false, "token");
        response.items = vec![make_item("item-1", "Task 1", true)]; // deleted

        cache.apply_sync_response(&response);

        assert_eq!(cache.items.len(), 1);
        assert_eq!(cache.items[0].id, "item-2");
    }

    #[test]
    fn test_apply_incremental_sync_adds_new_projects() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.projects = vec![make_project("proj-1", "Existing", false)];

        let mut response = make_sync_response(false, "token");
        response.projects = vec![make_project("proj-2", "New Project", false)];

        cache.apply_sync_response(&response);

        assert_eq!(cache.projects.len(), 2);
    }

    #[test]
    fn test_apply_incremental_sync_updates_existing_projects() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.projects = vec![make_project("proj-1", "Original", false)];

        let mut response = make_sync_response(false, "token");
        response.projects = vec![make_project("proj-1", "Renamed", false)];

        cache.apply_sync_response(&response);

        assert_eq!(cache.projects.len(), 1);
        assert_eq!(cache.projects[0].name, "Renamed");
    }

    #[test]
    fn test_apply_incremental_sync_removes_deleted_projects() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.projects = vec![
            make_project("proj-1", "Project 1", false),
            make_project("proj-2", "Project 2", false),
        ];

        let mut response = make_sync_response(false, "token");
        response.projects = vec![make_project("proj-2", "Project 2", true)];

        cache.apply_sync_response(&response);

        assert_eq!(cache.projects.len(), 1);
        assert_eq!(cache.projects[0].id, "proj-1");
    }

    #[test]
    fn test_apply_incremental_sync_adds_new_labels() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.labels = vec![make_label("label-1", "Work", false)];

        let mut response = make_sync_response(false, "token");
        response.labels = vec![make_label("label-2", "Personal", false)];

        cache.apply_sync_response(&response);

        assert_eq!(cache.labels.len(), 2);
    }

    #[test]
    fn test_apply_incremental_sync_updates_existing_labels() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.labels = vec![make_label("label-1", "Work", false)];

        let mut response = make_sync_response(false, "token");
        response.labels = vec![make_label("label-1", "Office", false)];

        cache.apply_sync_response(&response);

        assert_eq!(cache.labels.len(), 1);
        assert_eq!(cache.labels[0].name, "Office");
    }

    #[test]
    fn test_apply_incremental_sync_removes_deleted_labels() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.labels = vec![
            make_label("label-1", "Work", false),
            make_label("label-2", "Personal", false),
        ];

        let mut response = make_sync_response(false, "token");
        response.labels = vec![make_label("label-1", "Work", true)];

        cache.apply_sync_response(&response);

        assert_eq!(cache.labels.len(), 1);
        assert_eq!(cache.labels[0].id, "label-2");
    }

    #[test]
    fn test_apply_incremental_sync_sections() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.sections = vec![make_section("sec-1", "Section 1", false)];

        let mut response = make_sync_response(false, "token");
        response.sections = vec![
            make_section("sec-1", "Updated Section", false), // update
            make_section("sec-2", "New Section", false),     // add
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.sections.len(), 2);
        assert_eq!(
            cache.sections.iter().find(|s| s.id == "sec-1").unwrap().name,
            "Updated Section"
        );
    }

    #[test]
    fn test_apply_incremental_sync_removes_deleted_sections() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.sections = vec![make_section("sec-1", "Section", false)];

        let mut response = make_sync_response(false, "token");
        response.sections = vec![make_section("sec-1", "Section", true)];

        cache.apply_sync_response(&response);

        assert!(cache.sections.is_empty());
    }

    #[test]
    fn test_apply_incremental_sync_notes() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.notes = vec![make_note("note-1", "Comment 1", false)];

        let mut response = make_sync_response(false, "token");
        response.notes = vec![
            make_note("note-1", "Updated comment", false),
            make_note("note-2", "New comment", false),
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.notes.len(), 2);
        assert_eq!(
            cache.notes.iter().find(|n| n.id == "note-1").unwrap().content,
            "Updated comment"
        );
    }

    #[test]
    fn test_apply_incremental_sync_removes_deleted_notes() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.notes = vec![make_note("note-1", "Comment", false)];

        let mut response = make_sync_response(false, "token");
        response.notes = vec![make_note("note-1", "Comment", true)];

        cache.apply_sync_response(&response);

        assert!(cache.notes.is_empty());
    }

    #[test]
    fn test_apply_incremental_sync_reminders() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.reminders = vec![make_reminder("rem-1", false)];

        let mut response = make_sync_response(false, "token");
        response.reminders = vec![make_reminder("rem-2", false)];

        cache.apply_sync_response(&response);

        assert_eq!(cache.reminders.len(), 2);
    }

    #[test]
    fn test_apply_incremental_sync_removes_deleted_reminders() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.reminders = vec![make_reminder("rem-1", false)];

        let mut response = make_sync_response(false, "token");
        response.reminders = vec![make_reminder("rem-1", true)];

        cache.apply_sync_response(&response);

        assert!(cache.reminders.is_empty());
    }

    #[test]
    fn test_apply_incremental_sync_filters() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.filters = vec![make_filter("filter-1", "Today", false)];

        let mut response = make_sync_response(false, "token");
        response.filters = vec![
            make_filter("filter-1", "Today's Tasks", false), // update
            make_filter("filter-2", "Overdue", false),       // add
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.filters.len(), 2);
        assert_eq!(
            cache.filters.iter().find(|f| f.id == "filter-1").unwrap().name,
            "Today's Tasks"
        );
    }

    #[test]
    fn test_apply_incremental_sync_removes_deleted_filters() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.filters = vec![make_filter("filter-1", "Today", false)];

        let mut response = make_sync_response(false, "token");
        response.filters = vec![make_filter("filter-1", "Today", true)];

        cache.apply_sync_response(&response);

        assert!(cache.filters.is_empty());
    }

    #[test]
    fn test_apply_incremental_sync_updates_user() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.user = Some(make_user("user-1"));

        let mut response = make_sync_response(false, "token");
        let mut new_user = make_user("user-1");
        new_user.full_name = Some("Updated Name".to_string());
        response.user = Some(new_user);

        cache.apply_sync_response(&response);

        assert_eq!(
            cache.user.as_ref().unwrap().full_name,
            Some("Updated Name".to_string())
        );
    }

    #[test]
    fn test_apply_incremental_sync_preserves_user_when_not_in_response() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.user = Some(make_user("user-1"));

        let response = make_sync_response(false, "token"); // no user in response

        cache.apply_sync_response(&response);

        assert!(cache.user.is_some());
        assert_eq!(cache.user.as_ref().unwrap().id, "user-1");
    }

    #[test]
    fn test_apply_incremental_sync_does_not_update_full_sync_date() {
        use test_helpers::*;

        let mut cache = Cache::new();
        let original_date = DateTime::parse_from_rfc3339("2025-01-20T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        cache.full_sync_date_utc = Some(original_date);

        let response = make_sync_response(false, "token");

        cache.apply_sync_response(&response);

        assert_eq!(cache.full_sync_date_utc, Some(original_date));
    }

    #[test]
    fn test_apply_sync_response_mixed_operations() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.items = vec![
            make_item("item-1", "Task 1", false),
            make_item("item-2", "Task 2", false),
            make_item("item-3", "Task 3", false),
        ];

        let mut response = make_sync_response(false, "token");
        response.items = vec![
            make_item("item-1", "Updated Task 1", false), // update
            make_item("item-2", "Task 2", true),          // delete
            make_item("item-4", "New Task 4", false),     // add
        ];

        cache.apply_sync_response(&response);

        assert_eq!(cache.items.len(), 3);
        assert!(cache.items.iter().any(|i| i.id == "item-1" && i.content == "Updated Task 1"));
        assert!(!cache.items.iter().any(|i| i.id == "item-2"));
        assert!(cache.items.iter().any(|i| i.id == "item-3"));
        assert!(cache.items.iter().any(|i| i.id == "item-4"));
    }

    #[test]
    fn test_apply_incremental_sync_delete_nonexistent_item_is_noop() {
        use test_helpers::*;

        let mut cache = Cache::new();
        cache.items = vec![make_item("item-1", "Task 1", false)];

        let mut response = make_sync_response(false, "token");
        response.items = vec![make_item("item-999", "Nonexistent", true)];

        cache.apply_sync_response(&response);

        // Should not error, cache unchanged
        assert_eq!(cache.items.len(), 1);
        assert_eq!(cache.items[0].id, "item-1");
    }
}
