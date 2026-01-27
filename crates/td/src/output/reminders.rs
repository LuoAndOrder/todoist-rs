//! Reminder output formatting.

use owo_colors::OwoColorize;
use serde::Serialize;
use todoist_api::sync::Reminder;
use todoist_cache::Cache;

use crate::commands::reminders::{ReminderAddResult, ReminderDeleteResult};

use super::helpers::{format_reminder, truncate_id};
use super::tasks::DueOutput;

/// JSON output structure for reminders list command.
#[derive(Serialize)]
pub struct RemindersListOutput<'a> {
    pub reminders: Vec<ReminderListOutput<'a>>,
}

/// JSON output structure for a single reminder in list.
#[derive(Serialize)]
pub struct ReminderListOutput<'a> {
    pub id: &'a str,
    pub item_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_name: Option<&'a str>,
    pub reminder_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<DueOutput<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minute_offset: Option<i32>,
}

/// Formats reminders as JSON.
pub fn format_reminders_json(
    reminders: &[&Reminder],
    cache: &Cache,
) -> Result<String, serde_json::Error> {
    let reminders_output: Vec<ReminderListOutput> = reminders
        .iter()
        .map(|r| {
            let task_name = cache
                .items
                .iter()
                .find(|i| i.id == r.item_id)
                .map(|i| i.content.as_str());

            let due = r.due.as_ref().map(|d| DueOutput {
                date: &d.date,
                datetime: d.datetime.as_deref(),
                string: d.string.as_deref(),
                is_recurring: d.is_recurring,
            });

            ReminderListOutput {
                id: &r.id,
                item_id: &r.item_id,
                task_name,
                reminder_type: &r.reminder_type,
                due,
                minute_offset: r.minute_offset,
            }
        })
        .collect();

    let output = RemindersListOutput {
        reminders: reminders_output,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats reminders as a table.
pub fn format_reminders_table(
    reminders: &[&Reminder],
    task_name: Option<&str>,
    use_colors: bool,
) -> String {
    if reminders.is_empty() {
        return "No reminders found.\n".to_string();
    }

    let mut output = String::new();

    // Header with task context
    if let Some(name) = task_name {
        let header = format!("Reminders for: {}", name);
        if use_colors {
            output.push_str(&format!("{}\n\n", header.bold()));
        } else {
            output.push_str(&header);
            output.push_str("\n\n");
        }
    }

    // Column header
    let header = format!("{:<8} {:<12} {}", "ID", "Type", "When");
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Reminders
    for reminder in reminders {
        let id_prefix = truncate_id(&reminder.id);
        let reminder_desc = format_reminder(reminder);

        let line = format!(
            "{:<8} {:<12} {}",
            id_prefix, reminder.reminder_type, reminder_desc
        );
        output.push_str(&line);
        output.push('\n');
    }

    output
}

/// JSON output structure for a created reminder.
#[derive(Serialize)]
pub struct CreatedReminderOutput<'a> {
    pub id: &'a str,
    pub task_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_name: Option<&'a str>,
    pub reminder_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minute_offset: Option<i32>,
}

/// Formats a created reminder as JSON.
pub fn format_created_reminder(result: &ReminderAddResult) -> Result<String, serde_json::Error> {
    let output = CreatedReminderOutput {
        id: &result.id,
        task_id: &result.task_id,
        task_name: result.task_name.as_deref(),
        reminder_type: &result.reminder_type,
        due: result.due.as_deref(),
        minute_offset: result.minute_offset,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a deleted reminder.
#[derive(Serialize)]
pub struct DeletedReminderOutput<'a> {
    pub id: &'a str,
    pub task_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_name: Option<&'a str>,
    pub reminder_type: &'a str,
    pub status: &'static str,
}

/// Formats a deleted reminder as JSON.
pub fn format_deleted_reminder(result: &ReminderDeleteResult) -> Result<String, serde_json::Error> {
    let output = DeletedReminderOutput {
        id: &result.id,
        task_id: &result.task_id,
        task_name: result.task_name.as_deref(),
        reminder_type: &result.reminder_type,
        status: "deleted",
    };

    serde_json::to_string_pretty(&output)
}
