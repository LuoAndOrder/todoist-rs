//! Output formatting utilities for the td CLI.
//!
//! This module provides functions for formatting data as tables or JSON.

use chrono::{Local, NaiveDate};
use owo_colors::OwoColorize;
use serde::Serialize;
use todoist_api::sync::Item;
use todoist_cache::Cache;

use crate::commands::add::AddResult;
use crate::commands::projects::{ProjectAddResult, ProjectArchiveResult, ProjectDeleteResult, ProjectEditResult, ProjectsShowResult, ProjectUnarchiveResult};
use crate::commands::quick::QuickResult;
use crate::commands::show::ShowResult;

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
    pub reminders: Vec<ReminderOutput<'a>>,
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
pub struct ReminderOutput<'a> {
    pub id: &'a str,
    pub reminder_type: &'a str,
    pub due: Option<DueOutput<'a>>,
    pub minute_offset: Option<i32>,
}

/// JSON output for a subtask.
#[derive(Serialize)]
pub struct SubtaskOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub checked: bool,
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
            id: &r.id,
            reminder_type: &r.reminder_type,
            due: r.due.as_ref().map(|d| DueOutput {
                date: &d.date,
                datetime: d.datetime.as_deref(),
                string: d.string.as_deref(),
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

/// Formats priority for verbose display (show command).
fn format_priority_verbose(api_priority: i32, use_colors: bool) -> String {
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
fn format_due_verbose(due: &todoist_api::sync::Due, use_colors: bool) -> String {
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
fn format_datetime(datetime: &str) -> String {
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
fn format_reminder(reminder: &todoist_api::sync::Reminder) -> String {
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

// ============================================================================
// Project Output Formatting
// ============================================================================

use std::collections::HashMap;
use todoist_api::sync::Project;

/// JSON output structure for a created project.
#[derive(Serialize)]
pub struct CreatedProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<&'a str>,
    pub is_favorite: bool,
}

/// Formats a created project as JSON.
pub fn format_created_project(result: &ProjectAddResult) -> Result<String, serde_json::Error> {
    let output = CreatedProjectOutput {
        id: &result.id,
        name: &result.name,
        color: result.color.as_deref(),
        parent_id: result.parent_id.as_deref(),
        parent_name: result.parent_name.as_deref(),
        is_favorite: result.is_favorite,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an edited project.
#[derive(Serialize)]
pub struct EditedProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub updated_fields: &'a [String],
}

/// Formats an edited project as JSON.
pub fn format_edited_project(result: &ProjectEditResult) -> Result<String, serde_json::Error> {
    let output = EditedProjectOutput {
        id: &result.id,
        name: &result.name,
        updated_fields: &result.updated_fields,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an archived project.
#[derive(Serialize)]
pub struct ArchivedProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats an archived project as JSON.
pub fn format_archived_project(result: &ProjectArchiveResult) -> Result<String, serde_json::Error> {
    let output = ArchivedProjectOutput {
        id: &result.id,
        name: &result.name,
        status: "archived",
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an unarchived project.
#[derive(Serialize)]
pub struct UnarchivedProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats an unarchived project as JSON.
pub fn format_unarchived_project(result: &ProjectUnarchiveResult) -> Result<String, serde_json::Error> {
    let output = UnarchivedProjectOutput {
        id: &result.id,
        name: &result.name,
        status: "unarchived",
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a deleted project.
#[derive(Serialize)]
pub struct DeletedProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats a deleted project as JSON.
pub fn format_deleted_project(result: &ProjectDeleteResult) -> Result<String, serde_json::Error> {
    let output = DeletedProjectOutput {
        id: &result.id,
        name: &result.name,
        status: "deleted",
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for projects list command.
#[derive(Serialize)]
pub struct ProjectsListOutput<'a> {
    pub projects: Vec<ProjectOutput<'a>>,
}

/// JSON output structure for a single project.
#[derive(Serialize)]
pub struct ProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<&'a str>,
    pub is_favorite: bool,
    pub is_archived: bool,
    pub is_inbox: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_style: Option<&'a str>,
    pub task_count: usize,
}

/// Formats projects as JSON.
pub fn format_projects_json(
    projects: &[&Project],
) -> Result<String, serde_json::Error> {
    let projects_output: Vec<ProjectOutput> = projects
        .iter()
        .map(|p| ProjectOutput {
            id: &p.id,
            name: &p.name,
            color: p.color.as_deref(),
            parent_id: p.parent_id.as_deref(),
            is_favorite: p.is_favorite,
            is_archived: p.is_archived,
            is_inbox: p.inbox_project,
            view_style: p.view_style.as_deref(),
            task_count: 0, // We don't have task count in this context
        })
        .collect();

    let output = ProjectsListOutput {
        projects: projects_output,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats projects as a table.
pub fn format_projects_table(
    projects: &[&Project],
    cache: &Cache,
    use_colors: bool,
    tree: bool,
) -> String {
    if projects.is_empty() {
        return "No projects found.\n".to_string();
    }

    let mut output = String::new();

    if tree {
        // Tree view: show hierarchy with indentation
        output.push_str(&format_projects_tree(projects, cache, use_colors));
    } else {
        // Flat view: simple table
        output.push_str(&format_projects_flat(projects, cache, use_colors));
    }

    output
}

/// Formats projects as a flat table.
fn format_projects_flat(projects: &[&Project], cache: &Cache, use_colors: bool) -> String {
    let mut output = String::new();

    // Header
    let header = format!(
        "{:<8} {:<4} {:<25} {:<6} {}",
        "ID", "Fav", "Name", "Tasks", "Color"
    );
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Count tasks per project
    let task_counts = count_tasks_per_project(cache);

    // Projects
    for project in projects {
        let id_prefix = truncate_id(&project.id);
        let fav = if project.is_favorite {
            if use_colors {
                "★".yellow().to_string()
            } else {
                "★".to_string()
            }
        } else {
            " ".to_string()
        };
        let name = format_project_name(project, use_colors);
        let task_count = task_counts.get(&project.id).copied().unwrap_or(0);
        let color = project.color.as_deref().unwrap_or("");

        let line = format!(
            "{:<8} {:<4} {:<25} {:<6} {}",
            id_prefix,
            fav,
            truncate_str(&name, 25),
            task_count,
            color
        );
        output.push_str(&line);
        output.push('\n');
    }

    output
}

/// Formats projects as a tree with indentation.
fn format_projects_tree(projects: &[&Project], cache: &Cache, use_colors: bool) -> String {
    let mut output = String::new();

    // Build parent-child relationships
    let mut children_map: HashMap<Option<&str>, Vec<&Project>> = HashMap::new();
    for project in projects {
        children_map
            .entry(project.parent_id.as_deref())
            .or_default()
            .push(project);
    }

    // Sort children by child_order
    for children in children_map.values_mut() {
        children.sort_by_key(|p| p.child_order);
    }

    // Count tasks per project
    let task_counts = count_tasks_per_project(cache);

    // Recursively print tree starting from root projects
    fn print_tree(
        output: &mut String,
        parent_id: Option<&str>,
        children_map: &HashMap<Option<&str>, Vec<&Project>>,
        task_counts: &HashMap<String, usize>,
        depth: usize,
        use_colors: bool,
    ) {
        if let Some(children) = children_map.get(&parent_id) {
            for project in children {
                let indent = "  ".repeat(depth);
                let prefix = if depth > 0 { "└─ " } else { "" };
                let fav = if project.is_favorite { "★ " } else { "" };
                let task_count = task_counts.get(&project.id).copied().unwrap_or(0);
                let id_prefix = truncate_id(&project.id);

                let name_display = if use_colors {
                    if project.inbox_project {
                        project.name.cyan().to_string()
                    } else if project.is_favorite {
                        format!("{}{}", fav.yellow(), project.name)
                    } else {
                        project.name.clone()
                    }
                } else {
                    format!("{}{}", fav, project.name)
                };

                let line = format!(
                    "{}{}{} ({}) [{}]",
                    indent, prefix, name_display, task_count, id_prefix
                );
                output.push_str(&line);
                output.push('\n');

                // Recursively print children
                print_tree(
                    output,
                    Some(&project.id),
                    children_map,
                    task_counts,
                    depth + 1,
                    use_colors,
                );
            }
        }
    }

    print_tree(&mut output, None, &children_map, &task_counts, 0, use_colors);

    output
}

/// Counts tasks per project.
fn count_tasks_per_project(cache: &Cache) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for item in &cache.items {
        if !item.is_deleted && !item.checked {
            *counts.entry(item.project_id.clone()).or_insert(0) += 1;
        }
    }
    counts
}

/// Formats a project name with special indicators.
fn format_project_name(project: &Project, use_colors: bool) -> String {
    let mut name = project.name.clone();

    if project.inbox_project {
        if use_colors {
            name = name.cyan().to_string();
        } else {
            name = format!("{} (Inbox)", name);
        }
    }

    if project.is_archived {
        if use_colors {
            name = name.strikethrough().dimmed().to_string();
        } else {
            name = format!("{} [archived]", name);
        }
    }

    name
}

// ============================================================================
// Project Details Output Formatting (projects show command)
// ============================================================================

/// JSON output structure for project details (projects show command).
#[derive(Serialize)]
pub struct ProjectDetailsOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<&'a str>,
    pub is_favorite: bool,
    pub is_archived: bool,
    pub is_inbox: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_style: Option<&'a str>,
    pub task_count: usize,
    pub section_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SectionOutput<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<ProjectTaskOutput<'a>>,
}

/// JSON output for a section in project details.
#[derive(Serialize)]
pub struct SectionOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub order: i32,
}

/// JSON output for a task in project details.
#[derive(Serialize)]
pub struct ProjectTaskOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub priority: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<&'a str>,
}

/// Formats project details as JSON (projects show command).
pub fn format_project_details_json(result: &ProjectsShowResult) -> Result<String, serde_json::Error> {
    let sections: Vec<SectionOutput> = result
        .sections
        .iter()
        .map(|s| SectionOutput {
            id: &s.id,
            name: &s.name,
            order: s.section_order,
        })
        .collect();

    let tasks: Vec<ProjectTaskOutput> = result
        .tasks
        .iter()
        .map(|t| ProjectTaskOutput {
            id: &t.id,
            content: &t.content,
            // Convert API priority (4=highest) to user priority (1=highest)
            priority: (5 - t.priority) as u8,
            due: t.due.as_ref().map(|d| d.date.as_str()),
            section_id: t.section_id.as_deref(),
        })
        .collect();

    let output = ProjectDetailsOutput {
        id: &result.project.id,
        name: &result.project.name,
        color: result.project.color.as_deref(),
        parent_id: result.project.parent_id.as_deref(),
        parent_name: result.parent_name.as_deref(),
        is_favorite: result.project.is_favorite,
        is_archived: result.project.is_archived,
        is_inbox: result.project.inbox_project,
        view_style: result.project.view_style.as_deref(),
        task_count: result.task_count,
        section_count: result.section_count,
        sections,
        tasks,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats project details as a human-readable table (projects show command).
pub fn format_project_details_table(result: &ProjectsShowResult, use_colors: bool) -> String {
    let mut output = String::new();

    // Project header
    let name_label = if use_colors {
        "Project:".bold().to_string()
    } else {
        "Project:".to_string()
    };
    output.push_str(&format!("{} {}\n", name_label, result.project.name));

    // ID
    output.push_str(&format!("ID: {}\n", result.project.id));

    // Parent project
    if let Some(ref parent_name) = result.parent_name {
        output.push_str(&format!("Parent: {}\n", parent_name));
    }

    // Color
    if let Some(ref color) = result.project.color {
        output.push_str(&format!("Color: {}\n", color));
    }

    // View style
    if let Some(ref view_style) = result.project.view_style {
        output.push_str(&format!("View style: {}\n", view_style));
    }

    // Favorite
    if result.project.is_favorite {
        let fav = if use_colors {
            "★ Yes".yellow().to_string()
        } else {
            "Yes".to_string()
        };
        output.push_str(&format!("Favorite: {}\n", fav));
    }

    // Inbox indicator
    if result.project.inbox_project {
        let inbox = if use_colors {
            "Yes".cyan().to_string()
        } else {
            "Yes".to_string()
        };
        output.push_str(&format!("Inbox: {}\n", inbox));
    }

    // Archived indicator
    if result.project.is_archived {
        let archived = if use_colors {
            "Yes".dimmed().to_string()
        } else {
            "Yes".to_string()
        };
        output.push_str(&format!("Archived: {}\n", archived));
    }

    // Task and section counts
    output.push_str(&format!("Tasks: {}\n", result.task_count));
    output.push_str(&format!("Sections: {}\n", result.section_count));

    // Sections list (if requested)
    if !result.sections.is_empty() {
        output.push_str(&format!("\nSections ({}):\n", result.sections.len()));
        let mut sorted_sections = result.sections.clone();
        sorted_sections.sort_by_key(|s| s.section_order);
        for section in &sorted_sections {
            let id_prefix = truncate_id(&section.id);
            output.push_str(&format!("  {} {}\n", id_prefix, section.name));
        }
    }

    // Tasks list (if requested)
    if !result.tasks.is_empty() {
        output.push_str(&format!("\nTasks ({}):\n", result.tasks.len()));
        for task in &result.tasks {
            let id_prefix = truncate_id(&task.id);
            let priority = format_priority(task.priority, use_colors);
            let due = format_due(task.due.as_ref().map(|d| &d.date), use_colors);
            let due_str = if due.is_empty() {
                String::new()
            } else {
                format!(" [{}]", due)
            };
            output.push_str(&format!("  {} {} {}{}\n", id_prefix, priority, task.content, due_str));
        }
    }

    output
}

// ============================================================================
// Label Output Formatting
// ============================================================================

use crate::commands::labels::{LabelAddResult, LabelDeleteResult, LabelEditResult};
use todoist_api::sync::Label;

/// JSON output structure for labels list command.
#[derive(Serialize)]
pub struct LabelsListOutput<'a> {
    pub labels: Vec<LabelOutput<'a>>,
}

/// JSON output structure for a single label.
#[derive(Serialize)]
pub struct LabelOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    pub is_favorite: bool,
    pub item_order: i32,
}

/// Formats labels as JSON.
pub fn format_labels_json(labels: &[&Label]) -> Result<String, serde_json::Error> {
    let labels_output: Vec<LabelOutput> = labels
        .iter()
        .map(|l| LabelOutput {
            id: &l.id,
            name: &l.name,
            color: l.color.as_deref(),
            is_favorite: l.is_favorite,
            item_order: l.item_order,
        })
        .collect();

    let output = LabelsListOutput {
        labels: labels_output,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats labels as a table.
pub fn format_labels_table(labels: &[&Label], use_colors: bool) -> String {
    if labels.is_empty() {
        return "No labels found.\n".to_string();
    }

    let mut output = String::new();

    // Header
    let header = format!("{:<8} {:<4} {:<20} {}", "ID", "Fav", "Name", "Color");
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Labels
    for label in labels {
        let id_prefix = truncate_id(&label.id);
        let fav = if label.is_favorite {
            if use_colors {
                "★".yellow().to_string()
            } else {
                "★".to_string()
            }
        } else {
            " ".to_string()
        };
        let name = format!("@{}", label.name);
        let color = label.color.as_deref().unwrap_or("");

        let line = format!("{:<8} {:<4} {:<20} {}", id_prefix, fav, truncate_str(&name, 20), color);
        output.push_str(&line);
        output.push('\n');
    }

    output
}

/// JSON output structure for a created label.
#[derive(Serialize)]
pub struct CreatedLabelOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    pub is_favorite: bool,
}

/// Formats a created label as JSON.
pub fn format_created_label(result: &LabelAddResult) -> Result<String, serde_json::Error> {
    let output = CreatedLabelOutput {
        id: &result.id,
        name: &result.name,
        color: result.color.as_deref(),
        is_favorite: result.is_favorite,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an edited label.
#[derive(Serialize)]
pub struct EditedLabelOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub updated_fields: &'a [String],
}

/// Formats an edited label as JSON.
pub fn format_edited_label(result: &LabelEditResult) -> Result<String, serde_json::Error> {
    let output = EditedLabelOutput {
        id: &result.id,
        name: &result.name,
        updated_fields: &result.updated_fields,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a deleted label.
#[derive(Serialize)]
pub struct DeletedLabelOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats a deleted label as JSON.
pub fn format_deleted_label(result: &LabelDeleteResult) -> Result<String, serde_json::Error> {
    let output = DeletedLabelOutput {
        id: &result.id,
        name: &result.name,
        status: "deleted",
    };

    serde_json::to_string_pretty(&output)
}

// ============================================================================
// Section Output Formatting
// ============================================================================

use crate::commands::sections::{SectionAddResult, SectionDeleteResult, SectionEditResult};
use todoist_api::sync::Section;

/// JSON output structure for sections list command.
#[derive(Serialize)]
pub struct SectionsListOutput<'a> {
    pub sections: Vec<SectionListOutput<'a>>,
}

/// JSON output structure for a single section in list.
#[derive(Serialize)]
pub struct SectionListOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub project_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<&'a str>,
    pub section_order: i32,
    pub is_archived: bool,
}

/// Formats sections as JSON.
pub fn format_sections_json(sections: &[&Section], cache: &Cache) -> Result<String, serde_json::Error> {
    let sections_output: Vec<SectionListOutput> = sections
        .iter()
        .map(|s| {
            let project_name = cache
                .projects
                .iter()
                .find(|p| p.id == s.project_id)
                .map(|p| p.name.as_str());

            SectionListOutput {
                id: &s.id,
                name: &s.name,
                project_id: &s.project_id,
                project_name,
                section_order: s.section_order,
                is_archived: s.is_archived,
            }
        })
        .collect();

    let output = SectionsListOutput {
        sections: sections_output,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats sections as a table.
pub fn format_sections_table(sections: &[&Section], cache: &Cache, use_colors: bool) -> String {
    if sections.is_empty() {
        return "No sections found.\n".to_string();
    }

    let mut output = String::new();

    // Header
    let header = format!("{:<8} {:<25} {:<20}", "ID", "Name", "Project");
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Sections
    for section in sections {
        let id_prefix = truncate_id(&section.id);
        let project_name = cache
            .projects
            .iter()
            .find(|p| p.id == section.project_id)
            .map(|p| truncate_str(&p.name, 20))
            .unwrap_or_default();

        let name = if section.is_archived {
            if use_colors {
                format!("{} [archived]", section.name).dimmed().to_string()
            } else {
                format!("{} [archived]", section.name)
            }
        } else {
            section.name.clone()
        };

        let line = format!(
            "{:<8} {:<25} {:<20}",
            id_prefix,
            truncate_str(&name, 25),
            project_name
        );
        output.push_str(&line);
        output.push('\n');
    }

    output
}

/// JSON output structure for a created section.
#[derive(Serialize)]
pub struct CreatedSectionOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub project_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<&'a str>,
}

/// Formats a created section as JSON.
pub fn format_created_section(result: &SectionAddResult) -> Result<String, serde_json::Error> {
    let output = CreatedSectionOutput {
        id: &result.id,
        name: &result.name,
        project_id: &result.project_id,
        project_name: result.project_name.as_deref(),
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an edited section.
#[derive(Serialize)]
pub struct EditedSectionOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub updated_fields: &'a [String],
}

/// Formats an edited section as JSON.
pub fn format_edited_section(result: &SectionEditResult) -> Result<String, serde_json::Error> {
    let output = EditedSectionOutput {
        id: &result.id,
        name: &result.name,
        updated_fields: &result.updated_fields,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a deleted section.
#[derive(Serialize)]
pub struct DeletedSectionOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats a deleted section as JSON.
pub fn format_deleted_section(result: &SectionDeleteResult) -> Result<String, serde_json::Error> {
    let output = DeletedSectionOutput {
        id: &result.id,
        name: &result.name,
        status: "deleted",
    };

    serde_json::to_string_pretty(&output)
}

// ============================================================================
// Comment Output Formatting
// ============================================================================

use crate::commands::comments::{Comment, CommentAddResult};

/// JSON output structure for comments list command.
#[derive(Serialize)]
pub struct CommentsListOutput<'a> {
    pub comments: Vec<CommentListOutput<'a>>,
}

/// JSON output structure for a single comment in list.
#[derive(Serialize)]
pub struct CommentListOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<&'a str>,
    pub parent_id: &'a str,
    pub parent_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<&'a str>,
}

/// Formats comments as JSON.
pub fn format_comments_json(
    comments: &[Comment],
    cache: &Cache,
) -> Result<String, serde_json::Error> {
    let comments_output: Vec<CommentListOutput> = comments
        .iter()
        .map(|c| {
            let (parent_type, parent_name) = if c.is_task_comment() {
                let task_name = cache
                    .items
                    .iter()
                    .find(|i| i.id == c.parent_id())
                    .map(|i| i.content.as_str());
                ("task", task_name)
            } else {
                let project_name = cache
                    .projects
                    .iter()
                    .find(|p| p.id == c.parent_id())
                    .map(|p| p.name.as_str());
                ("project", project_name)
            };

            CommentListOutput {
                id: c.id(),
                content: c.content(),
                posted_at: c.posted_at(),
                parent_id: c.parent_id(),
                parent_type,
                parent_name,
            }
        })
        .collect();

    let output = CommentsListOutput {
        comments: comments_output,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a created comment.
#[derive(Serialize)]
pub struct CreatedCommentOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub parent_id: &'a str,
    pub parent_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<&'a str>,
}

/// Formats a created comment as JSON.
pub fn format_created_comment(result: &CommentAddResult) -> Result<String, serde_json::Error> {
    let parent_type = if result.is_task_comment { "task" } else { "project" };
    let output = CreatedCommentOutput {
        id: &result.id,
        content: &result.content,
        parent_id: &result.parent_id,
        parent_type,
        parent_name: result.parent_name.as_deref(),
    };

    serde_json::to_string_pretty(&output)
}

/// Formats comments as a table.
pub fn format_comments_table(
    comments: &[Comment],
    parent_name: Option<&str>,
    use_colors: bool,
) -> String {
    if comments.is_empty() {
        return "No comments found.\n".to_string();
    }

    let mut output = String::new();

    // Header with parent context
    if let Some(name) = parent_name {
        let header = format!("Comments for: {}", name);
        if use_colors {
            output.push_str(&format!("{}\n\n", header.bold()));
        } else {
            output.push_str(&header);
            output.push_str("\n\n");
        }
    }

    // Column header
    let header = format!("{:<8} {:<20} {}", "ID", "Posted", "Content");
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Comments
    for comment in comments {
        let id_prefix = truncate_id(comment.id());
        let posted = comment
            .posted_at()
            .map(format_datetime)
            .unwrap_or_default();
        let posted_display = truncate_str(&posted, 20);

        // Truncate content to first line and max 50 chars for table view
        let content_first_line = comment.content().lines().next().unwrap_or("");
        let content_display = truncate_str(content_first_line, 50);

        let line = format!("{:<8} {:<20} {}", id_prefix, posted_display, content_display);
        output.push_str(&line);
        output.push('\n');
    }

    output
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
