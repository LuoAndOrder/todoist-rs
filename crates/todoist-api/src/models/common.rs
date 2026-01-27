//! Common model types shared across REST and Sync APIs.
//!
//! These types represent domain concepts that appear in both the REST API v2
//! and Sync API v1, ensuring consistent handling across the codebase.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Due date/time information for a task.
///
/// This struct is used by both the REST API and Sync API to represent when
/// a task is due. It supports both date-only and datetime values, as well as
/// recurring schedules.
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

/// Deadline for a task (separate from due date).
///
/// Deadlines represent hard cutoff dates that are distinct from the "due date"
/// which may indicate when you plan to work on a task.
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

/// Unit of time for task duration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DurationUnit {
    /// Duration in minutes.
    Minute,
    /// Duration in days.
    Day,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

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
    fn test_due_deserialize() {
        let json = r#"{
            "date": "2026-01-25",
            "datetime": "2026-01-25T15:00:00Z",
            "is_recurring": false,
            "string": "tomorrow at 3pm",
            "timezone": "America/New_York"
        }"#;

        let due: Due = serde_json::from_str(json).unwrap();
        assert_eq!(due.date, "2026-01-25");
        assert!(due.has_time());
        assert!(!due.is_recurring);
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
    fn test_duration_deserialize() {
        let json = r#"{"amount": 15, "unit": "minute"}"#;
        let duration: Duration = serde_json::from_str(json).unwrap();
        assert_eq!(duration.amount, 15);
        assert_eq!(duration.unit, DurationUnit::Minute);
    }
}
