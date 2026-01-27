//! Quick Add API types for the Todoist API.
//!
//! The Quick Add endpoint (`POST /api/v1/tasks/quick`) provides NLP-based task creation
//! that parses natural language input to extract project, labels, priority, due date, etc.

use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::models::Due;
use crate::sync::Item;

/// Request body for the Quick Add endpoint.
///
/// The `text` field supports Todoist quick add notation for specifying projects,
/// priority, labels, etc., just as if you were using the Todoist quick add window.
///
/// # Example
///
/// ```no_run
/// use todoist_api_rs::quick_add::QuickAddRequest;
///
/// // Create a simple quick add request
/// let request = QuickAddRequest::new("Buy milk tomorrow #Shopping p1 @errands").unwrap();
///
/// // With a note attachment
/// let request = QuickAddRequest::new("Call mom tomorrow at 5pm")
///     .unwrap()
///     .with_note("Don't forget to ask about Sunday dinner");
///
/// // With auto reminder enabled
/// let request = QuickAddRequest::new("Meeting at 3pm")
///     .unwrap()
///     .with_auto_reminder(true);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QuickAddRequest {
    /// The text to parse using natural language processing.
    /// Supports Todoist quick add notation:
    /// - `#Project` - assign to project
    /// - `@label` - add label
    /// - `p1`/`p2`/`p3`/`p4` - set priority
    /// - Natural language dates like "tomorrow", "next monday", "at 3pm"
    pub text: String,

    /// Optional text to attach as a comment with the task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,

    /// Optional natural language date for creating a task reminder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminder: Option<String>,

    /// When true, the default reminder will be added to the task if it has
    /// a due date with time set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_reminder: Option<bool>,
}

impl QuickAddRequest {
    /// Creates a new QuickAddRequest with the given text.
    ///
    /// # Arguments
    ///
    /// * `text` - The natural language text to parse for creating the task.
    ///
    /// # Errors
    ///
    /// Returns `ApiError::Validation` if the text is empty or contains only whitespace.
    ///
    /// # Example
    ///
    /// ```
    /// use todoist_api_rs::quick_add::QuickAddRequest;
    ///
    /// // Valid text
    /// let request = QuickAddRequest::new("Buy milk").unwrap();
    ///
    /// // Empty text returns an error
    /// let result = QuickAddRequest::new("");
    /// assert!(result.is_err());
    ///
    /// // Whitespace-only text returns an error
    /// let result = QuickAddRequest::new("   ");
    /// assert!(result.is_err());
    /// ```
    pub fn new(text: impl Into<String>) -> Result<Self, ApiError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(ApiError::Validation {
                field: Some("text".to_string()),
                message: "task text cannot be empty".to_string(),
            });
        }
        Ok(Self {
            text,
            note: None,
            reminder: None,
            auto_reminder: None,
        })
    }

    /// Adds a note/comment to attach to the created task.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Adds a reminder with the given natural language date.
    pub fn with_reminder(mut self, reminder: impl Into<String>) -> Self {
        self.reminder = Some(reminder.into());
        self
    }

    /// Enables or disables auto reminder for the task.
    pub fn with_auto_reminder(mut self, auto_reminder: bool) -> Self {
        self.auto_reminder = Some(auto_reminder);
        self
    }

    /// Converts the request to form-urlencoded format for the API.
    pub fn to_form_body(&self) -> String {
        serde_urlencoded::to_string(self).expect("form serialization should not fail")
    }
}

/// Response from the Quick Add endpoint.
///
/// Contains the created task along with metadata about how the input was parsed.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct QuickAddResponse {
    /// The ID of the created task (legacy format from sync/v9).
    pub id: String,

    /// The v2 ID of the created task (works with API v1).
    #[serde(default)]
    pub v2_id: Option<String>,

    /// The ID of the project the task was added to.
    pub project_id: String,

    /// The v2 project ID (works with API v1).
    #[serde(default)]
    pub v2_project_id: Option<String>,

    /// The parsed content of the task (with quick add notation removed).
    pub content: String,

    /// A description for the task.
    #[serde(default)]
    pub description: String,

    /// Task priority (1 = natural, 4 = very urgent).
    #[serde(default = "default_priority")]
    pub priority: i32,

    /// Due date information parsed from the input.
    #[serde(default)]
    pub due: Option<Due>,

    /// The ID of the section (if specified or parsed).
    #[serde(default)]
    pub section_id: Option<String>,

    /// Parent task ID (if specified).
    #[serde(default)]
    pub parent_id: Option<String>,

    /// Order among siblings.
    #[serde(default)]
    pub child_order: i32,

    /// Labels parsed from the input.
    #[serde(default)]
    pub labels: Vec<String>,

    /// ID of the user who added this task.
    #[serde(default)]
    pub added_by_uid: Option<String>,

    /// ID of the user who assigned this task.
    #[serde(default)]
    pub assigned_by_uid: Option<String>,

    /// ID of the user responsible for this task.
    #[serde(default)]
    pub responsible_uid: Option<String>,

    /// Whether the task is completed.
    #[serde(default)]
    pub checked: bool,

    /// Whether the task is deleted.
    #[serde(default)]
    pub is_deleted: bool,

    /// When the task was added.
    #[serde(default)]
    pub added_at: Option<String>,

    /// Resolved project name (for debugging/display).
    #[serde(default)]
    pub resolved_project_name: Option<String>,

    /// Resolved assignee name (for shared projects).
    #[serde(default)]
    pub resolved_assignee_name: Option<String>,
}

fn default_priority() -> i32 {
    1
}

impl QuickAddResponse {
    /// Returns the ID suitable for use with the v1 Sync API.
    /// Prefers `v2_id` if available, falling back to `id`.
    pub fn api_id(&self) -> &str {
        self.v2_id.as_deref().unwrap_or(&self.id)
    }

    /// Returns the project ID suitable for use with the v1 Sync API.
    /// Prefers `v2_project_id` if available, falling back to `project_id`.
    pub fn api_project_id(&self) -> &str {
        self.v2_project_id.as_deref().unwrap_or(&self.project_id)
    }

    /// Returns true if labels were parsed from the input.
    pub fn has_labels(&self) -> bool {
        !self.labels.is_empty()
    }

    /// Returns true if a due date was parsed from the input.
    pub fn has_due_date(&self) -> bool {
        self.due.is_some()
    }

    /// Converts this response to an Item for use with the Sync API types.
    /// Uses v2_id and v2_project_id if available for API v1 compatibility.
    pub fn into_item(self) -> Item {
        Item {
            id: self.v2_id.unwrap_or(self.id),
            user_id: self.added_by_uid.clone(),
            project_id: self.v2_project_id.unwrap_or(self.project_id),
            content: self.content,
            description: self.description,
            priority: self.priority,
            due: self.due,
            deadline: None,
            parent_id: self.parent_id,
            child_order: self.child_order,
            section_id: self.section_id,
            day_order: 0,
            is_collapsed: false,
            labels: self.labels,
            added_by_uid: self.added_by_uid,
            assigned_by_uid: self.assigned_by_uid,
            responsible_uid: self.responsible_uid,
            checked: self.checked,
            is_deleted: self.is_deleted,
            added_at: self.added_at,
            updated_at: None,
            completed_at: None,
            duration: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_add_request_new() {
        let request = QuickAddRequest::new("Buy milk tomorrow").unwrap();
        assert_eq!(request.text, "Buy milk tomorrow");
        assert!(request.note.is_none());
        assert!(request.reminder.is_none());
        assert!(request.auto_reminder.is_none());
    }

    #[test]
    fn test_quick_add_request_new_empty_text_returns_error() {
        let result = QuickAddRequest::new("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ApiError::Validation { field, message } => {
                assert_eq!(field, Some("text".to_string()));
                assert!(message.contains("empty"));
            }
            _ => panic!("Expected Validation error, got {:?}", err),
        }
    }

    #[test]
    fn test_quick_add_request_new_whitespace_only_returns_error() {
        let result = QuickAddRequest::new("   \t\n  ");
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ApiError::Validation { field, message } => {
                assert_eq!(field, Some("text".to_string()));
                assert!(message.contains("empty"));
            }
            _ => panic!("Expected Validation error, got {:?}", err),
        }
    }

    #[test]
    fn test_quick_add_request_new_valid_text_with_whitespace() {
        // Text with leading/trailing whitespace is valid (whitespace is preserved)
        let request = QuickAddRequest::new("  Buy milk  ").unwrap();
        assert_eq!(request.text, "  Buy milk  ");
    }

    #[test]
    fn test_quick_add_request_with_note() {
        let request = QuickAddRequest::new("Call mom")
            .unwrap()
            .with_note("Ask about dinner plans");
        assert_eq!(request.note, Some("Ask about dinner plans".to_string()));
    }

    #[test]
    fn test_quick_add_request_with_reminder() {
        let request = QuickAddRequest::new("Meeting at 3pm")
            .unwrap()
            .with_reminder("30 minutes before");
        assert_eq!(request.reminder, Some("30 minutes before".to_string()));
    }

    #[test]
    fn test_quick_add_request_with_auto_reminder() {
        let request = QuickAddRequest::new("Meeting at 3pm")
            .unwrap()
            .with_auto_reminder(true);
        assert_eq!(request.auto_reminder, Some(true));
    }

    #[test]
    fn test_quick_add_request_builder_chain() {
        let request = QuickAddRequest::new("Buy groceries tomorrow #Shopping @errands p2")
            .unwrap()
            .with_note("Don't forget the milk")
            .with_reminder("1 hour before")
            .with_auto_reminder(true);

        assert_eq!(request.text, "Buy groceries tomorrow #Shopping @errands p2");
        assert_eq!(request.note, Some("Don't forget the milk".to_string()));
        assert_eq!(request.reminder, Some("1 hour before".to_string()));
        assert_eq!(request.auto_reminder, Some(true));
    }

    #[test]
    fn test_quick_add_request_to_form_body_minimal() {
        let request = QuickAddRequest::new("Test task").unwrap();
        let body = request.to_form_body();

        // Decode and verify the text field is correctly encoded
        let decoded: std::collections::HashMap<String, String> =
            serde_urlencoded::from_str(&body).unwrap();
        assert_eq!(decoded.get("text").unwrap(), "Test task");
    }

    #[test]
    fn test_quick_add_request_to_form_body_full() {
        let request = QuickAddRequest::new("Test task")
            .unwrap()
            .with_note("A note")
            .with_reminder("tomorrow")
            .with_auto_reminder(true);
        let body = request.to_form_body();

        // Decode and verify all fields are correctly encoded
        let decoded: std::collections::HashMap<String, String> =
            serde_urlencoded::from_str(&body).unwrap();
        assert_eq!(decoded.get("text").unwrap(), "Test task");
        assert_eq!(decoded.get("note").unwrap(), "A note");
        assert_eq!(decoded.get("reminder").unwrap(), "tomorrow");
        assert_eq!(decoded.get("auto_reminder").unwrap(), "true");
    }

    #[test]
    fn test_quick_add_request_to_form_body_with_special_chars() {
        let request = QuickAddRequest::new("Buy milk #Shopping @errands").unwrap();
        let body = request.to_form_body();

        // Decode and verify special characters are handled correctly
        let decoded: std::collections::HashMap<String, String> =
            serde_urlencoded::from_str(&body).unwrap();
        assert_eq!(decoded.get("text").unwrap(), "Buy milk #Shopping @errands");
    }

    #[test]
    fn test_quick_add_response_deserialize_minimal() {
        let json = r#"{
            "id": "task-123",
            "project_id": "proj-456",
            "content": "Buy milk"
        }"#;

        let response: QuickAddResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "task-123");
        assert_eq!(response.project_id, "proj-456");
        assert_eq!(response.content, "Buy milk");
        assert_eq!(response.priority, 1);
        assert!(!response.checked);
        assert!(response.labels.is_empty());
        assert!(!response.has_labels());
        assert!(!response.has_due_date());
    }

    #[test]
    fn test_quick_add_response_deserialize_full() {
        let json = r#"{
            "id": "task-123",
            "project_id": "proj-456",
            "content": "Buy groceries",
            "description": "For the party",
            "priority": 3,
            "due": {
                "date": "2026-01-26",
                "datetime": "2026-01-26T15:00:00Z",
                "string": "tomorrow at 3pm",
                "is_recurring": false
            },
            "section_id": "section-789",
            "labels": ["shopping", "urgent"],
            "child_order": 1,
            "checked": false,
            "is_deleted": false,
            "added_at": "2026-01-25T10:00:00Z",
            "resolved_project_name": "Shopping List"
        }"#;

        let response: QuickAddResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "task-123");
        assert_eq!(response.content, "Buy groceries");
        assert_eq!(response.description, "For the party");
        assert_eq!(response.priority, 3);
        assert!(response.has_due_date());

        let due = response.due.as_ref().unwrap();
        assert_eq!(due.date, "2026-01-26");
        assert_eq!(due.datetime, Some("2026-01-26T15:00:00Z".to_string()));

        assert_eq!(response.section_id, Some("section-789".to_string()));
        assert!(response.has_labels());
        assert_eq!(response.labels.len(), 2);
        assert!(response.labels.contains(&"shopping".to_string()));
        assert!(response.labels.contains(&"urgent".to_string()));

        assert_eq!(response.resolved_project_name, Some("Shopping List".to_string()));
    }

    #[test]
    fn test_quick_add_response_into_item() {
        let response = QuickAddResponse {
            id: "task-123".to_string(),
            v2_id: Some("v2-task-123".to_string()),
            project_id: "proj-456".to_string(),
            v2_project_id: Some("v2-proj-456".to_string()),
            content: "Test task".to_string(),
            description: "Description".to_string(),
            priority: 2,
            due: Some(Due {
                date: "2026-01-26".to_string(),
                datetime: None,
                string: Some("tomorrow".to_string()),
                timezone: None,
                is_recurring: false,
                lang: None,
            }),
            section_id: Some("section-1".to_string()),
            parent_id: None,
            child_order: 1,
            labels: vec!["work".to_string()],
            added_by_uid: Some("user-1".to_string()),
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: Some("2026-01-25T10:00:00Z".to_string()),
            resolved_project_name: None,
            resolved_assignee_name: None,
        };

        // into_item should use v2_id and v2_project_id when available
        let item = response.into_item();
        assert_eq!(item.id, "v2-task-123");
        assert_eq!(item.project_id, "v2-proj-456");
        assert_eq!(item.content, "Test task");
        assert_eq!(item.priority, 2);
        assert!(item.due.is_some());
        assert_eq!(item.labels, vec!["work".to_string()]);
    }

    #[test]
    fn test_quick_add_response_api_id() {
        let response_with_v2 = QuickAddResponse {
            id: "old-id".to_string(),
            v2_id: Some("v2-id".to_string()),
            project_id: "proj".to_string(),
            v2_project_id: Some("v2-proj".to_string()),
            content: "test".to_string(),
            description: String::new(),
            priority: 1,
            due: None,
            section_id: None,
            parent_id: None,
            child_order: 0,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            resolved_project_name: None,
            resolved_assignee_name: None,
        };

        assert_eq!(response_with_v2.api_id(), "v2-id");
        assert_eq!(response_with_v2.api_project_id(), "v2-proj");

        let response_without_v2 = QuickAddResponse {
            id: "old-id".to_string(),
            v2_id: None,
            project_id: "proj".to_string(),
            v2_project_id: None,
            content: "test".to_string(),
            description: String::new(),
            priority: 1,
            due: None,
            section_id: None,
            parent_id: None,
            child_order: 0,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            resolved_project_name: None,
            resolved_assignee_name: None,
        };

        assert_eq!(response_without_v2.api_id(), "old-id");
        assert_eq!(response_without_v2.api_project_id(), "proj");
    }
}
