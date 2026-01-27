//! Sync API response types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// Re-export common types that are used by sync API consumers
pub use crate::models::{Deadline, Due, Duration, DurationUnit, LocationTrigger, ReminderType};

/// Response from the Sync API endpoint.
///
/// Contains all requested resources and metadata about the sync operation.
///
/// # Examples
///
/// ## Check for command errors
///
/// ```
/// use todoist_api::sync::SyncResponse;
///
/// let json = r#"{
///     "sync_token": "new-token",
///     "full_sync": false,
///     "sync_status": {
///         "cmd-1": "ok",
///         "cmd-2": {"error_code": 15, "error": "Invalid temporary id"}
///     }
/// }"#;
///
/// let response: SyncResponse = serde_json::from_str(json).unwrap();
/// assert!(response.has_errors());
/// let errors = response.errors();
/// assert_eq!(errors.len(), 1);
/// ```
///
/// ## Look up real IDs from temp IDs
///
/// ```
/// use todoist_api::sync::SyncResponse;
///
/// let json = r#"{
///     "sync_token": "token",
///     "full_sync": false,
///     "temp_id_mapping": {
///         "temp-123": "real-id-456"
///     }
/// }"#;
///
/// let response: SyncResponse = serde_json::from_str(json).unwrap();
/// assert_eq!(response.real_id("temp-123"), Some(&"real-id-456".to_string()));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncResponse {
    /// New sync token for subsequent incremental syncs.
    pub sync_token: String,

    /// Whether this was a full sync (true) or incremental (false).
    #[serde(default)]
    pub full_sync: bool,

    /// UTC timestamp when the full sync data was generated.
    /// For large accounts, this may lag behind real-time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_sync_date_utc: Option<String>,

    /// Array of task (item) objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<Item>,

    /// Array of project objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<Project>,

    /// Array of personal label objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<Label>,

    /// Array of section objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,

    /// Array of task comment (note) objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,

    /// Array of project comment objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_notes: Vec<ProjectNote>,

    /// Array of reminder objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reminders: Vec<Reminder>,

    /// Array of filter objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<Filter>,

    /// User object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,

    /// Array of collaborator objects for shared projects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collaborators: Vec<Collaborator>,

    /// Array of collaborator state objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collaborator_states: Vec<CollaboratorState>,

    /// Command execution results, keyed by command UUID.
    /// Values are either "ok" or an error object.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub sync_status: HashMap<String, CommandResult>,

    /// Mapping of temporary IDs to real IDs for created resources.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub temp_id_mapping: HashMap<String, String>,

    /// Day orders for tasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day_orders: Option<serde_json::Value>,

    /// Live notifications array.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub live_notifications: Vec<serde_json::Value>,

    /// Last read live notification ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_notifications_last_read_id: Option<String>,

    /// User settings object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_settings: Option<serde_json::Value>,

    /// User plan limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_plan_limits: Option<serde_json::Value>,

    /// Productivity stats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<serde_json::Value>,

    /// Completed info for projects/sections.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completed_info: Vec<serde_json::Value>,

    /// Location-based reminders.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<serde_json::Value>,
}

/// Result of a command execution.
///
/// # Examples
///
/// ```
/// use todoist_api::sync::CommandResult;
///
/// // Success case
/// let ok: CommandResult = serde_json::from_str(r#""ok""#).unwrap();
/// assert!(ok.is_ok());
///
/// // Error case
/// let err: CommandResult = serde_json::from_str(
///     r#"{"error_code": 15, "error": "Invalid id"}"#
/// ).unwrap();
/// assert!(!err.is_ok());
/// assert_eq!(err.error().unwrap().error_code, 15);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandResult {
    /// Command succeeded.
    Ok(String),
    /// Command failed with error details.
    Error(CommandError),
}

impl CommandResult {
    /// Returns true if the command succeeded.
    pub fn is_ok(&self) -> bool {
        matches!(self, CommandResult::Ok(s) if s == "ok")
    }

    /// Returns the error if the command failed.
    pub fn error(&self) -> Option<&CommandError> {
        match self {
            CommandResult::Error(e) => Some(e),
            _ => None,
        }
    }
}

/// Error details for a failed command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandError {
    /// Error code.
    pub error_code: i32,
    /// Error message.
    pub error: String,
}

/// A task (called "item" in the Sync API).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// The ID of the task.
    pub id: String,

    /// The ID of the user who owns this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// The ID of the project this task belongs to.
    pub project_id: String,

    /// The text content of the task.
    pub content: String,

    /// A description for the task.
    #[serde(default)]
    pub description: String,

    /// Task priority (1 = natural, 4 = very urgent).
    #[serde(default = "default_priority")]
    pub priority: i32,

    /// Due date information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<Due>,

    /// Deadline information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<Deadline>,

    /// Parent task ID for subtasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    /// Order among siblings.
    #[serde(default)]
    pub child_order: i32,

    /// Section ID if the task is in a section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,

    /// Order in Today/Next 7 days view.
    #[serde(default)]
    pub day_order: i32,

    /// Whether subtasks are collapsed.
    #[serde(default)]
    pub is_collapsed: bool,

    /// Labels attached to this task (names, not IDs).
    #[serde(default)]
    pub labels: Vec<String>,

    /// ID of user who added this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_by_uid: Option<String>,

    /// ID of user who assigned this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_by_uid: Option<String>,

    /// ID of user responsible for this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responsible_uid: Option<String>,

    /// Whether the task is completed.
    #[serde(default)]
    pub checked: bool,

    /// Whether the task is deleted.
    #[serde(default)]
    pub is_deleted: bool,

    /// When the task was added.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at: Option<String>,

    /// When the task was last updated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,

    /// When the task was completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,

    /// Task duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<Duration>,
}

fn default_priority() -> i32 {
    1
}

/// A project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// The ID of the project.
    pub id: String,

    /// The name of the project.
    pub name: String,

    /// The color of the project icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// Parent project ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    /// Order among siblings.
    #[serde(default)]
    pub child_order: i32,

    /// Whether subprojects are collapsed.
    #[serde(default)]
    pub is_collapsed: bool,

    /// Whether the project is shared.
    #[serde(default)]
    pub shared: bool,

    /// Whether the project can have assigned tasks.
    #[serde(default)]
    pub can_assign_tasks: bool,

    /// Whether the project is deleted.
    #[serde(default)]
    pub is_deleted: bool,

    /// Whether the project is archived.
    #[serde(default)]
    pub is_archived: bool,

    /// Whether the project is a favorite.
    #[serde(default)]
    pub is_favorite: bool,

    /// View style: "list", "board", or "calendar".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_style: Option<String>,

    /// Whether this is the inbox project.
    #[serde(default)]
    pub inbox_project: bool,

    /// Folder ID (for workspaces).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,

    /// When the project was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// When the project was last updated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// A section within a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// The ID of the section.
    pub id: String,

    /// The name of the section.
    pub name: String,

    /// The project this section belongs to.
    pub project_id: String,

    /// Order within the project.
    #[serde(default)]
    pub section_order: i32,

    /// Whether tasks are collapsed.
    #[serde(default)]
    pub is_collapsed: bool,

    /// Whether the section is deleted.
    #[serde(default)]
    pub is_deleted: bool,

    /// Whether the section is archived.
    #[serde(default)]
    pub is_archived: bool,

    /// When the section was archived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,

    /// When the section was added.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at: Option<String>,

    /// When the section was last updated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// A personal label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    /// The ID of the label.
    pub id: String,

    /// The name of the label.
    pub name: String,

    /// The color of the label icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// Order in the label list.
    #[serde(default)]
    pub item_order: i32,

    /// Whether the label is deleted.
    #[serde(default)]
    pub is_deleted: bool,

    /// Whether the label is a favorite.
    #[serde(default)]
    pub is_favorite: bool,
}

/// A task comment (called "note" in the Sync API).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// The ID of the note.
    pub id: String,

    /// The task this note belongs to.
    pub item_id: String,

    /// The content of the note.
    pub content: String,

    /// When the note was posted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<String>,

    /// Whether the note is deleted.
    #[serde(default)]
    pub is_deleted: bool,

    /// ID of the user who posted this note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posted_uid: Option<String>,

    /// File attachment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_attachment: Option<FileAttachment>,
}

/// A project comment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectNote {
    /// The ID of the note.
    pub id: String,

    /// The project this note belongs to.
    pub project_id: String,

    /// The content of the note.
    pub content: String,

    /// When the note was posted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<String>,

    /// Whether the note is deleted.
    #[serde(default)]
    pub is_deleted: bool,

    /// ID of the user who posted this note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posted_uid: Option<String>,

    /// File attachment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_attachment: Option<FileAttachment>,
}

/// File attachment metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileAttachment {
    /// Resource type (always "file").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,

    /// File name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,

    /// File size in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,

    /// File type/MIME type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,

    /// URL to download the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,

    /// Upload state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_state: Option<String>,
}

/// A reminder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reminder {
    /// The ID of the reminder.
    pub id: String,

    /// The task this reminder is for.
    pub item_id: String,

    /// Reminder type: relative, absolute, or location.
    #[serde(rename = "type")]
    pub reminder_type: ReminderType,

    /// Due information for the reminder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<Due>,

    /// Minutes before due (for relative reminders).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minute_offset: Option<i32>,

    /// Whether the reminder is deleted.
    #[serde(default)]
    pub is_deleted: bool,

    /// User ID to notify (typically the current user).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_uid: Option<String>,

    /// Location name (for location reminders).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Location latitude (for location reminders).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loc_lat: Option<String>,

    /// Location longitude (for location reminders).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loc_long: Option<String>,

    /// Location trigger: on_enter or on_leave (for location reminders).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loc_trigger: Option<LocationTrigger>,

    /// Radius around the location in meters (for location reminders).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<i32>,
}

/// A saved filter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    /// The ID of the filter.
    pub id: String,

    /// The name of the filter.
    pub name: String,

    /// The filter query string.
    pub query: String,

    /// The color of the filter icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// Order in the filter list.
    #[serde(default)]
    pub item_order: i32,

    /// Whether the filter is deleted.
    #[serde(default)]
    pub is_deleted: bool,

    /// Whether the filter is a favorite.
    #[serde(default)]
    pub is_favorite: bool,
}

/// User information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    /// The user's ID.
    pub id: String,

    /// The user's email.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// The user's full name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,

    /// The user's timezone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    /// Inbox project ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbox_project_id: Option<String>,

    /// Start page preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_page: Option<String>,

    /// Week start day (1 = Monday, 7 = Sunday).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_day: Option<i32>,

    /// Date format preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_format: Option<i32>,

    /// Time format preference (12 or 24).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_format: Option<i32>,

    /// Whether user has premium.
    #[serde(default)]
    pub is_premium: bool,
}

/// A collaborator on a shared project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Collaborator {
    /// The collaborator's user ID.
    pub id: String,

    /// The collaborator's email.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// The collaborator's full name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,

    /// The collaborator's timezone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    /// URL to the collaborator's avatar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
}

/// State of a collaborator in a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollaboratorState {
    /// Project ID.
    pub project_id: String,

    /// User ID of the collaborator.
    pub user_id: String,

    /// State: "invited", "active", "inactive", or "deleted".
    pub state: String,
}

impl SyncResponse {
    /// Returns true if any commands failed.
    pub fn has_errors(&self) -> bool {
        self.sync_status.values().any(|r| !r.is_ok())
    }

    /// Returns all command errors.
    pub fn errors(&self) -> Vec<(&String, &CommandError)> {
        self.sync_status
            .iter()
            .filter_map(|(uuid, result)| result.error().map(|e| (uuid, e)))
            .collect()
    }

    /// Looks up the real ID for a temporary ID.
    pub fn real_id(&self, temp_id: &str) -> Option<&String> {
        self.temp_id_mapping.get(temp_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_response_deserialize_minimal() {
        let json = r#"{
            "sync_token": "abc123",
            "full_sync": true
        }"#;

        let response: SyncResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.sync_token, "abc123");
        assert!(response.full_sync);
        assert!(response.items.is_empty());
        assert!(response.projects.is_empty());
    }

    #[test]
    fn test_sync_response_deserialize_with_items() {
        let json = r#"{
            "sync_token": "token123",
            "full_sync": false,
            "items": [
                {
                    "id": "item-1",
                    "project_id": "proj-1",
                    "content": "Buy milk",
                    "description": "",
                    "priority": 1,
                    "checked": false,
                    "is_deleted": false
                }
            ]
        }"#;

        let response: SyncResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].content, "Buy milk");
    }

    #[test]
    fn test_sync_response_deserialize_with_projects() {
        let json = r#"{
            "sync_token": "token",
            "full_sync": true,
            "projects": [
                {
                    "id": "proj-1",
                    "name": "Work",
                    "color": "blue",
                    "is_deleted": false,
                    "is_archived": false,
                    "is_favorite": true
                }
            ]
        }"#;

        let response: SyncResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.projects.len(), 1);
        assert_eq!(response.projects[0].name, "Work");
        assert!(response.projects[0].is_favorite);
    }

    #[test]
    fn test_sync_response_deserialize_with_sync_status() {
        let json = r#"{
            "sync_token": "token",
            "full_sync": false,
            "sync_status": {
                "cmd-1": "ok",
                "cmd-2": {"error_code": 15, "error": "Invalid temporary id"}
            }
        }"#;

        let response: SyncResponse = serde_json::from_str(json).unwrap();
        assert!(response.sync_status.get("cmd-1").unwrap().is_ok());
        assert!(!response.sync_status.get("cmd-2").unwrap().is_ok());

        let error = response.sync_status.get("cmd-2").unwrap().error().unwrap();
        assert_eq!(error.error_code, 15);
        assert_eq!(error.error, "Invalid temporary id");
    }

    #[test]
    fn test_sync_response_deserialize_with_temp_id_mapping() {
        let json = r#"{
            "sync_token": "token",
            "full_sync": false,
            "temp_id_mapping": {
                "temp-1": "real-id-1",
                "temp-2": "real-id-2"
            }
        }"#;

        let response: SyncResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.real_id("temp-1"), Some(&"real-id-1".to_string()));
        assert_eq!(response.real_id("temp-2"), Some(&"real-id-2".to_string()));
        assert_eq!(response.real_id("unknown"), None);
    }

    #[test]
    fn test_sync_response_has_errors() {
        let json = r#"{
            "sync_token": "token",
            "full_sync": false,
            "sync_status": {
                "cmd-1": "ok",
                "cmd-2": {"error_code": 15, "error": "Error"}
            }
        }"#;

        let response: SyncResponse = serde_json::from_str(json).unwrap();
        assert!(response.has_errors());

        let errors = response.errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].0, "cmd-2");
    }

    #[test]
    fn test_sync_response_no_errors() {
        let json = r#"{
            "sync_token": "token",
            "full_sync": false,
            "sync_status": {
                "cmd-1": "ok",
                "cmd-2": "ok"
            }
        }"#;

        let response: SyncResponse = serde_json::from_str(json).unwrap();
        assert!(!response.has_errors());
        assert!(response.errors().is_empty());
    }

    #[test]
    fn test_item_deserialize_full() {
        let json = r#"{
            "id": "6X7rM8997g3RQmvh",
            "user_id": "2671355",
            "project_id": "6Jf8VQXxpwv56VQ7",
            "content": "Buy Milk",
            "description": "From the store",
            "priority": 4,
            "due": {
                "date": "2025-01-21",
                "datetime": "2025-01-21T10:00:00Z",
                "string": "tomorrow at 10am",
                "timezone": "America/New_York",
                "is_recurring": false
            },
            "parent_id": null,
            "child_order": 1,
            "section_id": "3Ty8VQXxpwv28PK3",
            "day_order": -1,
            "is_collapsed": false,
            "labels": ["Food", "Shopping"],
            "checked": false,
            "is_deleted": false,
            "added_at": "2025-01-21T21:28:43.841504Z",
            "duration": {"amount": 15, "unit": "minute"}
        }"#;

        let item: Item = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, "6X7rM8997g3RQmvh");
        assert_eq!(item.content, "Buy Milk");
        assert_eq!(item.description, "From the store");
        assert_eq!(item.priority, 4);
        assert!(item.due.is_some());
        assert_eq!(item.labels, vec!["Food", "Shopping"]);

        let due = item.due.unwrap();
        assert_eq!(due.date, "2025-01-21");
        assert_eq!(due.datetime, Some("2025-01-21T10:00:00Z".to_string()));

        let duration = item.duration.unwrap();
        assert_eq!(duration.amount, 15);
        assert_eq!(duration.unit, DurationUnit::Minute);
    }

    #[test]
    fn test_project_deserialize() {
        let json = r#"{
            "id": "6Jf8VQXxpwv56VQ7",
            "name": "Shopping List",
            "color": "lime_green",
            "parent_id": null,
            "child_order": 1,
            "is_collapsed": false,
            "shared": false,
            "can_assign_tasks": false,
            "is_deleted": false,
            "is_archived": false,
            "is_favorite": false,
            "view_style": "list",
            "inbox_project": true
        }"#;

        let project: Project = serde_json::from_str(json).unwrap();
        assert_eq!(project.id, "6Jf8VQXxpwv56VQ7");
        assert_eq!(project.name, "Shopping List");
        assert_eq!(project.color, Some("lime_green".to_string()));
        assert!(project.inbox_project);
        assert!(!project.is_favorite);
    }

    #[test]
    fn test_section_deserialize() {
        let json = r#"{
            "id": "6Jf8VQXxpwv56VQ7",
            "name": "Groceries",
            "project_id": "9Bw8VQXxpwv56ZY2",
            "section_order": 1,
            "is_collapsed": false,
            "is_deleted": false,
            "is_archived": false
        }"#;

        let section: Section = serde_json::from_str(json).unwrap();
        assert_eq!(section.id, "6Jf8VQXxpwv56VQ7");
        assert_eq!(section.name, "Groceries");
        assert_eq!(section.project_id, "9Bw8VQXxpwv56ZY2");
    }

    #[test]
    fn test_label_deserialize() {
        let json = r#"{
            "id": "2156154810",
            "name": "Food",
            "color": "lime_green",
            "item_order": 0,
            "is_deleted": false,
            "is_favorite": false
        }"#;

        let label: Label = serde_json::from_str(json).unwrap();
        assert_eq!(label.id, "2156154810");
        assert_eq!(label.name, "Food");
        assert_eq!(label.color, Some("lime_green".to_string()));
    }

    #[test]
    fn test_filter_deserialize() {
        let json = r#"{
            "id": "filter-1",
            "name": "Today's Tasks",
            "query": "today | overdue",
            "color": "red",
            "item_order": 0,
            "is_deleted": false,
            "is_favorite": true
        }"#;

        let filter: Filter = serde_json::from_str(json).unwrap();
        assert_eq!(filter.id, "filter-1");
        assert_eq!(filter.name, "Today's Tasks");
        assert_eq!(filter.query, "today | overdue");
        assert!(filter.is_favorite);
    }

    #[test]
    fn test_reminder_deserialize_relative() {
        let json = r#"{
            "id": "reminder-1",
            "item_id": "item-1",
            "type": "relative",
            "minute_offset": 30,
            "is_deleted": false
        }"#;

        let reminder: Reminder = serde_json::from_str(json).unwrap();
        assert_eq!(reminder.id, "reminder-1");
        assert_eq!(reminder.item_id, "item-1");
        assert_eq!(reminder.reminder_type, ReminderType::Relative);
        assert_eq!(reminder.minute_offset, Some(30));
    }

    #[test]
    fn test_reminder_deserialize_absolute() {
        let json = r#"{
            "id": "reminder-2",
            "item_id": "item-1",
            "type": "absolute",
            "due": {
                "date": "2025-01-26",
                "datetime": "2025-01-26T10:00:00Z"
            },
            "is_deleted": false
        }"#;

        let reminder: Reminder = serde_json::from_str(json).unwrap();
        assert_eq!(reminder.id, "reminder-2");
        assert_eq!(reminder.reminder_type, ReminderType::Absolute);
        assert!(reminder.due.is_some());
    }

    #[test]
    fn test_reminder_deserialize_location() {
        let json = r#"{
            "id": "reminder-3",
            "item_id": "item-1",
            "type": "location",
            "name": "Home",
            "loc_lat": "37.7749",
            "loc_long": "-122.4194",
            "loc_trigger": "on_enter",
            "radius": 100,
            "is_deleted": false
        }"#;

        let reminder: Reminder = serde_json::from_str(json).unwrap();
        assert_eq!(reminder.id, "reminder-3");
        assert_eq!(reminder.reminder_type, ReminderType::Location);
        assert_eq!(reminder.name, Some("Home".to_string()));
        assert_eq!(reminder.loc_lat, Some("37.7749".to_string()));
        assert_eq!(reminder.loc_long, Some("-122.4194".to_string()));
        assert_eq!(reminder.loc_trigger, Some(LocationTrigger::OnEnter));
        assert_eq!(reminder.radius, Some(100));
    }

    #[test]
    fn test_note_deserialize() {
        let json = r#"{
            "id": "note-1",
            "item_id": "item-1",
            "content": "Remember to check expiration dates",
            "posted_at": "2025-01-21T10:00:00Z",
            "is_deleted": false
        }"#;

        let note: Note = serde_json::from_str(json).unwrap();
        assert_eq!(note.id, "note-1");
        assert_eq!(note.item_id, "item-1");
        assert_eq!(note.content, "Remember to check expiration dates");
    }

    #[test]
    fn test_user_deserialize() {
        let json = r#"{
            "id": "user-1",
            "email": "test@example.com",
            "full_name": "Test User",
            "timezone": "America/New_York",
            "inbox_project_id": "inbox-123",
            "is_premium": true
        }"#;

        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.id, "user-1");
        assert_eq!(user.email, Some("test@example.com".to_string()));
        assert_eq!(user.full_name, Some("Test User".to_string()));
        assert!(user.is_premium);
    }

    #[test]
    fn test_command_result_ok() {
        let result: CommandResult = serde_json::from_str(r#""ok""#).unwrap();
        assert!(result.is_ok());
        assert!(result.error().is_none());
    }

    #[test]
    fn test_command_result_error() {
        let result: CommandResult =
            serde_json::from_str(r#"{"error_code": 15, "error": "Invalid temporary id"}"#).unwrap();
        assert!(!result.is_ok());
        let error = result.error().unwrap();
        assert_eq!(error.error_code, 15);
        assert_eq!(error.error, "Invalid temporary id");
    }
}
