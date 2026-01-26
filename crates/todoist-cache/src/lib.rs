//! Local cache for Todoist data.
//!
//! This crate provides a local cache that mirrors the Sync API response structure,
//! enabling efficient incremental updates and offline read access.

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
}
