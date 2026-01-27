//! Task output formatting.

use owo_colors::OwoColorize;
use serde::Serialize;
use todoist_api_rs::sync::Item;
use todoist_cache_rs::Cache;

use crate::commands::add::AddResult;
use crate::commands::quick::QuickResult;
use crate::commands::show::ShowResult;

use super::helpers::{
    format_datetime, format_due, format_due_verbose, format_priority, format_priority_verbose,
    format_reminder, truncate_id, truncate_str,
};

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

/// JSON output structure for a quick add result.
#[derive(Serialize)]
pub struct QuickAddOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub project_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<&'a str>,
    pub priority: u8,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: &'a Vec<String>,
}

/// JSON output structure for task details (show command).
#[derive(Serialize)]
pub struct TaskDetailsOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub description: &'a str,
    pub priority: u8,
    pub due: Option<DueOutput<'a>>,
    pub project_id: &'a str,
    pub project_name: Option<&'a str>,
    pub section_id: Option<&'a str>,
    pub section_name: Option<&'a str>,
    pub parent_id: Option<&'a str>,
    pub labels: &'a [String],
    pub checked: bool,
    pub created_at: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<CommentOutput<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reminders: Vec<ReminderOutput>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subtasks: Vec<SubtaskOutput<'a>>,
}

/// JSON output for due date.
#[derive(Serialize)]
pub struct DueOutput<'a> {
    pub date: &'a str,
    pub datetime: Option<&'a str>,
    pub string: Option<&'a str>,
    pub is_recurring: bool,
}

/// JSON output for a comment.
#[derive(Serialize)]
pub struct CommentOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub posted_at: Option<&'a str>,
}

/// JSON output for a reminder.
#[derive(Serialize)]
pub struct ReminderOutput {
    pub id: String,
    pub reminder_type: todoist_api_rs::models::ReminderType,
    pub due: Option<DueOutputOwned>,
    pub minute_offset: Option<i32>,
}

/// Owned version of DueOutput for use in ReminderOutput.
#[derive(Serialize)]
pub struct DueOutputOwned {
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datetime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string: Option<String>,
    pub is_recurring: bool,
}

/// JSON output for a subtask.
#[derive(Serialize)]
pub struct SubtaskOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub checked: bool,
}

/// Formats items as JSON.
pub fn format_items_json(items: &[&Item], cache: &Cache) -> Result<String, serde_json::Error> {
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
        cursor: None,    // Pagination not implemented yet
        has_more: false, // Pagination not implemented yet
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

/// Formats a quick add result as JSON.
pub fn format_quick_add_result(result: &QuickResult) -> Result<String, serde_json::Error> {
    let output = QuickAddOutput {
        id: &result.id,
        content: &result.content,
        project_id: &result.project_id,
        project_name: result.project_name.as_deref(),
        due: result.due_string.as_deref(),
        priority: result.priority,
        labels: &result.labels,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats item details as JSON (show command).
pub fn format_item_details_json(result: &ShowResult) -> Result<String, serde_json::Error> {
    let due = result.item.due.as_ref().map(|d| DueOutput {
        date: &d.date,
        datetime: d.datetime.as_deref(),
        string: d.string.as_deref(),
        is_recurring: d.is_recurring,
    });

    let comments: Vec<CommentOutput> = result
        .comments
        .iter()
        .map(|n| CommentOutput {
            id: &n.id,
            content: &n.content,
            posted_at: n.posted_at.as_deref(),
        })
        .collect();

    let reminders: Vec<ReminderOutput> = result
        .reminders
        .iter()
        .map(|r| ReminderOutput {
            id: r.id.clone(),
            reminder_type: r.reminder_type,
            due: r.due.as_ref().map(|d| DueOutputOwned {
                date: d.date.clone(),
                datetime: d.datetime.clone(),
                string: d.string.clone(),
                is_recurring: d.is_recurring,
            }),
            minute_offset: r.minute_offset,
        })
        .collect();

    let subtasks: Vec<SubtaskOutput> = result
        .subtasks
        .iter()
        .map(|i| SubtaskOutput {
            id: &i.id,
            content: &i.content,
            checked: i.checked,
        })
        .collect();

    let output = TaskDetailsOutput {
        id: &result.item.id,
        content: &result.item.content,
        description: &result.item.description,
        // Convert API priority (4=highest) to user priority (1=highest)
        priority: (5 - result.item.priority) as u8,
        due,
        project_id: &result.item.project_id,
        project_name: result.project_name.as_deref(),
        section_id: result.item.section_id.as_deref(),
        section_name: result.section_name.as_deref(),
        parent_id: result.item.parent_id.as_deref(),
        labels: &result.labels,
        checked: result.item.checked,
        created_at: result.item.added_at.as_deref(),
        comments,
        reminders,
        subtasks,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats item details as a human-readable table (show command).
pub fn format_item_details_table(result: &ShowResult, use_colors: bool) -> String {
    let mut output = String::new();

    // Task header
    let content_label = if use_colors {
        "Task:".bold().to_string()
    } else {
        "Task:".to_string()
    };
    output.push_str(&format!("{} {}\n", content_label, result.item.content));

    // ID
    output.push_str(&format!("ID: {}\n", result.item.id));

    // Project
    if let Some(ref project_name) = result.project_name {
        output.push_str(&format!("Project: {}\n", project_name));
    }

    // Section
    if let Some(ref section_name) = result.section_name {
        output.push_str(&format!("Section: {}\n", section_name));
    }

    // Priority
    let priority_display = format_priority_verbose(result.item.priority, use_colors);
    output.push_str(&format!("Priority: {}\n", priority_display));

    // Due date
    if let Some(ref due) = result.item.due {
        let due_display = format_due_verbose(due, use_colors);
        output.push_str(&format!("Due: {}\n", due_display));
    }

    // Labels
    if !result.labels.is_empty() {
        let labels_str: Vec<String> = result.labels.iter().map(|l| format!("@{}", l)).collect();
        output.push_str(&format!("Labels: {}\n", labels_str.join(", ")));
    }

    // Created at
    if let Some(ref created) = result.item.added_at {
        output.push_str(&format!("Created: {}\n", format_datetime(created)));
    }

    // Description
    if !result.item.description.is_empty() {
        output.push_str("Description:\n");
        for line in result.item.description.lines() {
            output.push_str(&format!("  {}\n", line));
        }
    }

    // Subtasks
    if !result.subtasks.is_empty() {
        output.push_str(&format!("\nSubtasks ({}):\n", result.subtasks.len()));
        for subtask in &result.subtasks {
            let checkbox = if subtask.checked { "[x]" } else { "[ ]" };
            output.push_str(&format!("  {} {}\n", checkbox, subtask.content));
        }
    }

    // Comments
    if !result.comments.is_empty() {
        output.push_str(&format!("\nComments ({}):\n", result.comments.len()));
        for comment in &result.comments {
            let timestamp = comment
                .posted_at
                .as_ref()
                .map(|t| format_datetime(t))
                .unwrap_or_default();
            if !timestamp.is_empty() {
                output.push_str(&format!("  [{}]\n", timestamp));
            }
            for line in comment.content.lines() {
                output.push_str(&format!("  {}\n", line));
            }
            output.push('\n');
        }
    }

    // Reminders
    if !result.reminders.is_empty() {
        output.push_str(&format!("\nReminders ({}):\n", result.reminders.len()));
        for reminder in &result.reminders {
            let reminder_desc = format_reminder(reminder);
            output.push_str(&format!("  - {}\n", reminder_desc));
        }
    }

    output
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
        let labels = super::helpers::format_labels(&item.labels, 15);
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
