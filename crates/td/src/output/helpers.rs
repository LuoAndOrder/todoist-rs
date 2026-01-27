//! Common helper functions for output formatting.

use chrono::{Local, NaiveDate};
use owo_colors::OwoColorize;

/// Truncates an ID to 6 characters for display.
pub fn truncate_id(id: &str) -> String {
    if id.len() > 6 {
        id[..6].to_string()
    } else {
        id.to_string()
    }
}

/// Truncates a string to a maximum length.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

/// Formats priority for display.
pub fn format_priority(api_priority: i32, use_colors: bool) -> String {
    let user_priority = 5 - api_priority;
    let label = format!("p{user_priority}");

    if use_colors {
        match user_priority {
            1 => label.red().to_string(),
            2 => label.yellow().to_string(),
            3 => label.blue().to_string(),
            _ => label.dimmed().to_string(),
        }
    } else {
        label
    }
}

/// Formats a due date for display.
pub fn format_due(due_date: Option<&String>, use_colors: bool) -> String {
    let Some(date_str) = due_date else {
        return String::new();
    };

    let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
        return date_str.clone();
    };

    let today = Local::now().date_naive();
    let tomorrow = today + chrono::Duration::days(1);
    let yesterday = today - chrono::Duration::days(1);

    let display = if date == today {
        "Today".to_string()
    } else if date == tomorrow {
        "Tomorrow".to_string()
    } else if date == yesterday {
        "Yesterday".to_string()
    } else if date < today {
        // Format as relative days overdue
        let days = (today - date).num_days();
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        }
    } else {
        // Format as date
        date.format("%b %d").to_string()
    };

    if use_colors {
        if date < today {
            display.red().to_string()
        } else if date == today {
            display.yellow().to_string()
        } else {
            display
        }
    } else {
        display
    }
}

/// Formats labels for display.
pub fn format_labels(labels: &[String], max_len: usize) -> String {
    if labels.is_empty() {
        return String::new();
    }

    let formatted: Vec<String> = labels.iter().map(|l| format!("@{l}")).collect();
    let joined = formatted.join(" ");

    truncate_str(&joined, max_len)
}

/// Formats priority for verbose display (show command).
pub fn format_priority_verbose(api_priority: i32, use_colors: bool) -> String {
    let user_priority = 5 - api_priority;
    let label = match user_priority {
        1 => "p1 (highest)",
        2 => "p2 (high)",
        3 => "p3 (medium)",
        _ => "p4 (normal)",
    };

    if use_colors {
        match user_priority {
            1 => label.red().to_string(),
            2 => label.yellow().to_string(),
            3 => label.blue().to_string(),
            _ => label.dimmed().to_string(),
        }
    } else {
        label.to_string()
    }
}

/// Formats a due date for verbose display (show command).
pub fn format_due_verbose(due: &todoist_api::sync::Due, use_colors: bool) -> String {
    // Try to parse and format the date nicely
    let mut result = if let Ok(date) = NaiveDate::parse_from_str(&due.date, "%Y-%m-%d") {
        let today = Local::now().date_naive();
        let tomorrow = today + chrono::Duration::days(1);

        let date_str = if date == today {
            "Today".to_string()
        } else if date == tomorrow {
            "Tomorrow".to_string()
        } else if date < today {
            let days = (today - date).num_days();
            format!("{} days overdue", days)
        } else {
            date.format("%B %d, %Y").to_string()
        };

        if use_colors {
            if date < today {
                date_str.red().to_string()
            } else if date == today {
                date_str.yellow().to_string()
            } else {
                date_str
            }
        } else {
            date_str
        }
    } else {
        due.date.clone()
    };

    // Add time if available
    if let Some(ref datetime) = due.datetime {
        // Try to extract time from datetime string
        if let Some(time_part) = datetime.split('T').nth(1) {
            // Strip timezone info and format nicely
            let time_clean = time_part.trim_end_matches('Z');
            let hm: String = time_clean.split(':').take(2).collect::<Vec<_>>().join(":");
            if !hm.is_empty() {
                result.push_str(&format!(" at {}", hm));
            }
        }
    }

    // Add recurring indicator
    if due.is_recurring {
        if let Some(ref string) = due.string {
            result.push_str(&format!(" ({})", string));
        } else {
            result.push_str(" (recurring)");
        }
    }

    result
}

/// Formats a datetime string for display.
pub fn format_datetime(datetime: &str) -> String {
    // Try to parse ISO 8601 / RFC 3339 format
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(datetime) {
        let local = dt.with_timezone(&Local);
        local.format("%Y-%m-%d %H:%M").to_string()
    } else {
        // Fallback to original string
        datetime.to_string()
    }
}

/// Formats a reminder for display.
pub fn format_reminder(reminder: &todoist_api::sync::Reminder) -> String {
    match reminder.reminder_type.as_str() {
        "relative" => {
            if let Some(offset) = reminder.minute_offset {
                if offset == 0 {
                    "At time of due date".to_string()
                } else if offset < 60 {
                    format!("{} minutes before", offset)
                } else if offset == 60 {
                    "1 hour before".to_string()
                } else if offset < 1440 {
                    format!("{} hours before", offset / 60)
                } else {
                    format!("{} days before", offset / 1440)
                }
            } else {
                "Relative reminder".to_string()
            }
        }
        "absolute" => {
            if let Some(ref due) = reminder.due {
                format!("At {}", due.date)
            } else {
                "Absolute reminder".to_string()
            }
        }
        "location" => "Location-based reminder".to_string(),
        _ => reminder.reminder_type.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_id() {
        assert_eq!(truncate_id("abcdef"), "abcdef");
        assert_eq!(truncate_id("abcdefgh"), "abcdef");
        assert_eq!(truncate_id("abc"), "abc");
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is long", 10), "this is...");
    }

    #[test]
    fn test_format_priority_no_colors() {
        assert_eq!(format_priority(4, false), "p1");
        assert_eq!(format_priority(3, false), "p2");
        assert_eq!(format_priority(2, false), "p3");
        assert_eq!(format_priority(1, false), "p4");
    }

    #[test]
    fn test_format_labels() {
        assert_eq!(format_labels(&[], 15), "");
        assert_eq!(format_labels(&["urgent".to_string()], 15), "@urgent");
        assert_eq!(
            format_labels(&["a".to_string(), "b".to_string()], 15),
            "@a @b"
        );
    }
}
