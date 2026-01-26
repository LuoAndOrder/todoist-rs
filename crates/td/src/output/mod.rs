//! Output formatting utilities for the td CLI.
//!
//! This module provides functions for formatting data as tables or JSON.

use chrono::{Local, NaiveDate};
use owo_colors::OwoColorize;
use serde::Serialize;
use todoist_api::sync::Item;
use todoist_cache::Cache;

use crate::commands::add::AddResult;

/// JSON output structure for list command.
#[derive(Serialize)]
pub struct ListOutput<'a> {
    pub tasks: Vec<TaskOutput<'a>>,
    pub cursor: Option<String>,
    pub has_more: bool,
}

/// JSON output structure for a single task.
#[derive(Serialize)]
pub struct TaskOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub description: &'a str,
    pub priority: u8,
    pub due: Option<&'a str>,
    pub project_id: &'a str,
    pub project_name: Option<&'a str>,
    pub section_id: Option<&'a str>,
    pub labels: &'a [String],
}

/// JSON output structure for a created item.
#[derive(Serialize)]
pub struct CreatedItemOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub project_id: &'a str,
    pub project_name: Option<&'a str>,
}

/// Formats items as JSON.
pub fn format_items_json(
    items: &[&Item],
    cache: &Cache,
) -> Result<String, serde_json::Error> {
    let tasks: Vec<TaskOutput> = items
        .iter()
        .map(|item| {
            let project_name = cache
                .projects
                .iter()
                .find(|p| p.id == item.project_id)
                .map(|p| p.name.as_str());

            TaskOutput {
                id: &item.id,
                content: &item.content,
                description: &item.description,
                // Convert API priority (4=highest) to user priority (1=highest)
                priority: (5 - item.priority) as u8,
                due: item.due.as_ref().map(|d| d.date.as_str()),
                project_id: &item.project_id,
                project_name,
                section_id: item.section_id.as_deref(),
                labels: &item.labels,
            }
        })
        .collect();

    let output = ListOutput {
        tasks,
        cursor: None,     // Pagination not implemented yet
        has_more: false,  // Pagination not implemented yet
    };

    serde_json::to_string_pretty(&output)
}

/// Formats a created item as JSON.
pub fn format_created_item(result: &AddResult) -> Result<String, serde_json::Error> {
    let output = CreatedItemOutput {
        id: &result.id,
        content: &result.content,
        project_id: &result.project_id,
        project_name: result.project_name.as_deref(),
    };

    serde_json::to_string_pretty(&output)
}

/// Formats items as a table.
pub fn format_items_table(items: &[&Item], cache: &Cache, use_colors: bool) -> String {
    if items.is_empty() {
        return "No tasks found.\n".to_string();
    }

    let mut output = String::new();

    // Header
    let header = format!(
        "{:<8} {:<4} {:<12} {:<15} {:<15} {}",
        "ID", "Pri", "Due", "Project", "Labels", "Content"
    );
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Items
    for item in items {
        let id_prefix = truncate_id(&item.id);
        let priority = format_priority(item.priority, use_colors);
        let due = format_due(item.due.as_ref().map(|d| &d.date), use_colors);
        let project = cache
            .projects
            .iter()
            .find(|p| p.id == item.project_id)
            .map(|p| truncate_str(&p.name, 15))
            .unwrap_or_default();
        let labels = format_labels(&item.labels, 15);
        let content = &item.content;

        let line = format!(
            "{:<8} {:<4} {:<12} {:<15} {:<15} {}",
            id_prefix, priority, due, project, labels, content
        );
        output.push_str(&line);
        output.push('\n');
    }

    output
}

/// Truncates an ID to 6 characters for display.
fn truncate_id(id: &str) -> String {
    if id.len() > 6 {
        id[..6].to_string()
    } else {
        id.to_string()
    }
}

/// Truncates a string to a maximum length.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

/// Formats priority for display.
fn format_priority(api_priority: i32, use_colors: bool) -> String {
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
fn format_due(due_date: Option<&String>, use_colors: bool) -> String {
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
fn format_labels(labels: &[String], max_len: usize) -> String {
    if labels.is_empty() {
        return String::new();
    }

    let formatted: Vec<String> = labels.iter().map(|l| format!("@{l}")).collect();
    let joined = formatted.join(" ");

    truncate_str(&joined, max_len)
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
