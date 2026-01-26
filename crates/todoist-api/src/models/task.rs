//! Task model for the Todoist API.
//!
//! This module defines the Task struct and related types that represent
//! tasks from the Todoist API v1/REST v2.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// A task in Todoist.
///
/// Tasks are the core entity in Todoist, representing items that can be
/// completed, assigned, scheduled, and organized into projects and sections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// The unique identifier for the task.
    pub id: String,

    /// The text content of the task.
    pub content: String,

    /// A detailed description of the task (supports Markdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The ID of the project the task belongs to.
    pub project_id: String,

    /// The ID of the section the task belongs to (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,

    /// The ID of the parent task (if this is a subtask).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    /// The order of the task within its parent (project, section, or parent task).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i32>,

    /// Labels attached to the task.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,

    /// Task priority from 1 (normal) to 4 (urgent).
    /// Note: In the API, 1 is normal and 4 is urgent (opposite of UI display).
    #[serde(default = "default_priority")]
    pub priority: i32,

    /// The due date/time information for the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<Due>,

    /// The deadline for the task (separate from due date).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<Deadline>,

    /// The estimated duration for the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<Duration>,

    /// Whether the task is completed.
    #[serde(default)]
    pub is_completed: bool,

    /// The URL to view the task in Todoist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// The number of comments on the task.
    #[serde(default)]
    pub comment_count: i32,

    /// When the task was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,

    /// The ID of the user who created the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_id: Option<String>,

    /// The ID of the user the task is assigned to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee_id: Option<String>,

    /// The ID of the user who assigned the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigner_id: Option<String>,
}

fn default_priority() -> i32 {
    1
}

/// Due date/time information for a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Due {
    /// The date in YYYY-MM-DD format (always present).
    pub date: String,

    /// The full datetime in RFC3339 format (if a time is set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datetime: Option<String>,

    /// Whether this is a recurring due date.
    #[serde(default)]
    pub is_recurring: bool,

    /// Human-readable representation of the due date (e.g., "every day").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string: Option<String>,

    /// The timezone for the due datetime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    /// The language used for parsing the date string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// Deadline for a task (separate from due date).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deadline {
    /// The deadline date in YYYY-MM-DD format.
    pub date: String,

    /// The language used for the deadline string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// Estimated duration for completing a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Duration {
    /// The amount of time (positive integer).
    pub amount: i32,

    /// The unit of time ("minute" or "day").
    pub unit: DurationUnit,
}

/// Unit of time for task duration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DurationUnit {
    /// Duration in minutes.
    Minute,
    /// Duration in days.
    Day,
}

impl Task {
    /// Returns true if the task has a due date set.
    pub fn has_due_date(&self) -> bool {
        self.due.is_some()
    }

    /// Returns true if the task is a subtask (has a parent).
    pub fn is_subtask(&self) -> bool {
        self.parent_id.is_some()
    }

    /// Returns true if the task is recurring.
    pub fn is_recurring(&self) -> bool {
        self.due.as_ref().is_some_and(|d| d.is_recurring)
    }

    /// Returns the due date as a NaiveDate if set.
    pub fn due_date(&self) -> Option<NaiveDate> {
        self.due.as_ref().and_then(|d| {
            NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok()
        })
    }

    /// Returns true if this is a high priority task (priority 3 or 4).
    pub fn is_high_priority(&self) -> bool {
        self.priority >= 3
    }
}

impl Due {
    /// Creates a new Due with just a date.
    pub fn from_date(date: impl Into<String>) -> Self {
        Self {
            date: date.into(),
            datetime: None,
            is_recurring: false,
            string: None,
            timezone: None,
            lang: None,
        }
    }

    /// Creates a new Due with a datetime.
    pub fn from_datetime(date: impl Into<String>, datetime: impl Into<String>) -> Self {
        Self {
            date: date.into(),
            datetime: Some(datetime.into()),
            is_recurring: false,
            string: None,
            timezone: None,
            lang: None,
        }
    }

    /// Returns the due date as a NaiveDate.
    pub fn as_naive_date(&self) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(&self.date, "%Y-%m-%d").ok()
    }

    /// Returns true if a specific time is set.
    pub fn has_time(&self) -> bool {
        self.datetime.is_some()
    }
}

impl Duration {
    /// Creates a duration in minutes.
    pub fn minutes(amount: i32) -> Self {
        Self {
            amount,
            unit: DurationUnit::Minute,
        }
    }

    /// Creates a duration in days.
    pub fn days(amount: i32) -> Self {
        Self {
            amount,
            unit: DurationUnit::Day,
        }
    }

    /// Returns the duration in minutes.
    pub fn as_minutes(&self) -> i32 {
        match self.unit {
            DurationUnit::Minute => self.amount,
            DurationUnit::Day => self.amount * 24 * 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_task_deserialize_minimal() {
        let json = r#"{
            "id": "123",
            "content": "Buy milk",
            "project_id": "456"
        }"#;

        let task: Task = serde_json::from_str(json).unwrap();
        assert_eq!(task.id, "123");
        assert_eq!(task.content, "Buy milk");
        assert_eq!(task.project_id, "456");
        assert_eq!(task.priority, 1);
        assert!(!task.is_completed);
        assert!(task.labels.is_empty());
    }

    #[test]
    fn test_task_deserialize_full() {
        let json = r#"{
            "id": "123",
            "content": "Buy milk",
            "description": "From the store",
            "project_id": "456",
            "section_id": "789",
            "parent_id": null,
            "order": 1,
            "labels": ["shopping", "urgent"],
            "priority": 4,
            "due": {
                "date": "2026-01-25",
                "datetime": "2026-01-25T15:00:00Z",
                "is_recurring": false,
                "string": "Jan 25 at 3pm",
                "timezone": "America/New_York"
            },
            "is_completed": false,
            "url": "https://todoist.com/app/task/123",
            "comment_count": 2,
            "created_at": "2026-01-20T10:00:00Z",
            "creator_id": "user1",
            "assignee_id": "user2",
            "assigner_id": "user1"
        }"#;

        let task: Task = serde_json::from_str(json).unwrap();
        assert_eq!(task.id, "123");
        assert_eq!(task.content, "Buy milk");
        assert_eq!(task.description, Some("From the store".to_string()));
        assert_eq!(task.section_id, Some("789".to_string()));
        assert_eq!(task.priority, 4);
        assert!(task.has_due_date());
        assert!(task.is_high_priority());

        let due = task.due.as_ref().unwrap();
        assert_eq!(due.date, "2026-01-25");
        assert!(due.has_time());
        assert!(!due.is_recurring);
    }

    #[test]
    fn test_task_serialize() {
        let task = Task {
            id: "123".to_string(),
            content: "Test task".to_string(),
            description: None,
            project_id: "456".to_string(),
            section_id: None,
            parent_id: None,
            order: None,
            labels: vec![],
            priority: 1,
            due: None,
            deadline: None,
            duration: None,
            is_completed: false,
            url: None,
            comment_count: 0,
            created_at: None,
            creator_id: None,
            assignee_id: None,
            assigner_id: None,
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"id\":\"123\""));
        assert!(json.contains("\"content\":\"Test task\""));
        // Optional None fields should be skipped
        assert!(!json.contains("description"));
        assert!(!json.contains("section_id"));
    }

    #[test]
    fn test_task_is_subtask() {
        let mut task = Task {
            id: "123".to_string(),
            content: "Test".to_string(),
            description: None,
            project_id: "456".to_string(),
            section_id: None,
            parent_id: None,
            order: None,
            labels: vec![],
            priority: 1,
            due: None,
            deadline: None,
            duration: None,
            is_completed: false,
            url: None,
            comment_count: 0,
            created_at: None,
            creator_id: None,
            assignee_id: None,
            assigner_id: None,
        };

        assert!(!task.is_subtask());
        task.parent_id = Some("parent123".to_string());
        assert!(task.is_subtask());
    }

    #[test]
    fn test_task_is_recurring() {
        let task_no_due = Task {
            id: "123".to_string(),
            content: "Test".to_string(),
            description: None,
            project_id: "456".to_string(),
            section_id: None,
            parent_id: None,
            order: None,
            labels: vec![],
            priority: 1,
            due: None,
            deadline: None,
            duration: None,
            is_completed: false,
            url: None,
            comment_count: 0,
            created_at: None,
            creator_id: None,
            assignee_id: None,
            assigner_id: None,
        };
        assert!(!task_no_due.is_recurring());

        let task_recurring = Task {
            due: Some(Due {
                date: "2026-01-25".to_string(),
                datetime: None,
                is_recurring: true,
                string: Some("every day".to_string()),
                timezone: None,
                lang: None,
            }),
            ..task_no_due.clone()
        };
        assert!(task_recurring.is_recurring());
    }

    #[test]
    fn test_task_due_date() {
        let task = Task {
            id: "123".to_string(),
            content: "Test".to_string(),
            description: None,
            project_id: "456".to_string(),
            section_id: None,
            parent_id: None,
            order: None,
            labels: vec![],
            priority: 1,
            due: Some(Due::from_date("2026-01-25")),
            deadline: None,
            duration: None,
            is_completed: false,
            url: None,
            comment_count: 0,
            created_at: None,
            creator_id: None,
            assignee_id: None,
            assigner_id: None,
        };

        let due_date = task.due_date().unwrap();
        assert_eq!(due_date.year(), 2026);
        assert_eq!(due_date.month(), 1);
        assert_eq!(due_date.day(), 25);
    }

    #[test]
    fn test_due_from_date() {
        let due = Due::from_date("2026-01-25");
        assert_eq!(due.date, "2026-01-25");
        assert!(!due.is_recurring);
        assert!(!due.has_time());
    }

    #[test]
    fn test_due_from_datetime() {
        let due = Due::from_datetime("2026-01-25", "2026-01-25T15:00:00Z");
        assert_eq!(due.date, "2026-01-25");
        assert_eq!(due.datetime, Some("2026-01-25T15:00:00Z".to_string()));
        assert!(due.has_time());
    }

    #[test]
    fn test_due_as_naive_date() {
        let due = Due::from_date("2026-01-25");
        let date = due.as_naive_date().unwrap();
        assert_eq!(date.year(), 2026);
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 25);
    }

    #[test]
    fn test_deadline_deserialize() {
        let json = r#"{"date": "2026-01-30"}"#;
        let deadline: Deadline = serde_json::from_str(json).unwrap();
        assert_eq!(deadline.date, "2026-01-30");
    }

    #[test]
    fn test_duration_minutes() {
        let duration = Duration::minutes(30);
        assert_eq!(duration.amount, 30);
        assert_eq!(duration.unit, DurationUnit::Minute);
        assert_eq!(duration.as_minutes(), 30);
    }

    #[test]
    fn test_duration_days() {
        let duration = Duration::days(2);
        assert_eq!(duration.amount, 2);
        assert_eq!(duration.unit, DurationUnit::Day);
        assert_eq!(duration.as_minutes(), 2 * 24 * 60);
    }

    #[test]
    fn test_duration_unit_serialize() {
        let minute = DurationUnit::Minute;
        let day = DurationUnit::Day;

        assert_eq!(serde_json::to_string(&minute).unwrap(), "\"minute\"");
        assert_eq!(serde_json::to_string(&day).unwrap(), "\"day\"");
    }

    #[test]
    fn test_duration_unit_deserialize() {
        let minute: DurationUnit = serde_json::from_str("\"minute\"").unwrap();
        let day: DurationUnit = serde_json::from_str("\"day\"").unwrap();

        assert_eq!(minute, DurationUnit::Minute);
        assert_eq!(day, DurationUnit::Day);
    }

    #[test]
    fn test_task_with_duration() {
        let json = r#"{
            "id": "123",
            "content": "Meeting",
            "project_id": "456",
            "duration": {
                "amount": 60,
                "unit": "minute"
            }
        }"#;

        let task: Task = serde_json::from_str(json).unwrap();
        let duration = task.duration.unwrap();
        assert_eq!(duration.amount, 60);
        assert_eq!(duration.unit, DurationUnit::Minute);
    }

    #[test]
    fn test_task_with_deadline() {
        let json = r#"{
            "id": "123",
            "content": "Project deadline",
            "project_id": "456",
            "deadline": {
                "date": "2026-02-15"
            }
        }"#;

        let task: Task = serde_json::from_str(json).unwrap();
        let deadline = task.deadline.unwrap();
        assert_eq!(deadline.date, "2026-02-15");
    }

    #[test]
    fn test_task_priority_levels() {
        let task_normal = Task {
            id: "1".to_string(),
            content: "Normal".to_string(),
            description: None,
            project_id: "p".to_string(),
            section_id: None,
            parent_id: None,
            order: None,
            labels: vec![],
            priority: 1,
            due: None,
            deadline: None,
            duration: None,
            is_completed: false,
            url: None,
            comment_count: 0,
            created_at: None,
            creator_id: None,
            assignee_id: None,
            assigner_id: None,
        };
        assert!(!task_normal.is_high_priority());

        let task_high = Task {
            priority: 3,
            ..task_normal.clone()
        };
        assert!(task_high.is_high_priority());

        let task_urgent = Task {
            priority: 4,
            ..task_normal
        };
        assert!(task_urgent.is_high_priority());
    }

    #[test]
    fn test_task_labels_deserialization() {
        let json = r#"{
            "id": "123",
            "content": "Task with labels",
            "project_id": "456",
            "labels": ["work", "urgent", "review"]
        }"#;

        let task: Task = serde_json::from_str(json).unwrap();
        assert_eq!(task.labels.len(), 3);
        assert!(task.labels.contains(&"work".to_string()));
        assert!(task.labels.contains(&"urgent".to_string()));
        assert!(task.labels.contains(&"review".to_string()));
    }
}
