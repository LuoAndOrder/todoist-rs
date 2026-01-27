//! Reminders command implementation.
//!
//! Lists and manages reminders via the Sync API.
//! Uses SyncManager::execute_commands() to automatically update the cache.

use todoist_api::client::TodoistClient;
use todoist_api::models::ReminderType;
use todoist_api::sync::{Reminder, SyncCommand};
use todoist_cache::{Cache, CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};
use crate::output::{format_created_reminder, format_reminders_json, format_reminders_table};

/// Options for the reminders list command.
#[derive(Debug, Default)]
pub struct RemindersListOptions {
    /// Filter by task ID.
    pub task: Option<String>,
}

/// Executes the reminders list command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Reminders list command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails or if --task is not specified.
pub async fn execute(ctx: &CommandContext, opts: &RemindersListOptions, token: &str) -> Result<()> {
    // Require --task
    if opts.task.is_none() {
        return Err(CommandError::Config(
            "--task is required to list reminders.".to_string(),
        ));
    }

    // Initialize sync manager
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Only sync if explicitly requested with --sync flag
    if ctx.sync_first {
        if ctx.verbose {
            eprintln!("Syncing with Todoist...");
        }
        manager.sync().await?;
    }

    let cache = manager.cache();

    // Resolve task filter
    let task_id = if let Some(ref task) = opts.task {
        resolve_task_id(cache, task)?
    } else {
        unreachable!("Already validated that task is provided");
    };

    // Get reminders for this task
    let reminders = filter_reminders(cache, &task_id);

    // Get task name for display
    let task_name = cache
        .items
        .iter()
        .find(|i| i.id == task_id)
        .map(|i| i.content.clone());

    // Output
    if ctx.json_output {
        let output = format_reminders_json(&reminders, cache)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_reminders_table(&reminders, task_name.as_deref(), ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

/// Resolves a task name/ID to a task ID.
fn resolve_task_id(cache: &Cache, task: &str) -> Result<String> {
    // First try exact ID match
    if let Some(i) = cache.items.iter().find(|i| i.id == task && !i.is_deleted) {
        return Ok(i.id.clone());
    }

    // Try ID prefix match (6+ chars)
    if task.len() >= 6 {
        let prefix_matches: Vec<_> = cache
            .items
            .iter()
            .filter(|i| i.id.starts_with(task) && !i.is_deleted)
            .collect();

        if prefix_matches.len() == 1 {
            return Ok(prefix_matches[0].id.clone());
        }

        if prefix_matches.len() > 1 {
            let mut msg = format!(
                "Ambiguous task ID \"{task}\"\n\nMultiple tasks match this prefix:"
            );
            for item in prefix_matches.iter().take(5) {
                let prefix = &item.id[..6.min(item.id.len())];
                msg.push_str(&format!("\n  {}  {}", prefix, item.content));
            }
            if prefix_matches.len() > 5 {
                msg.push_str(&format!("\n  ... and {} more", prefix_matches.len() - 5));
            }
            msg.push_str("\n\nPlease use a longer prefix.");
            return Err(CommandError::Config(msg));
        }
    }

    Err(CommandError::Config(format!("Task not found: {task}")))
}

/// Filters reminders based on task_id.
fn filter_reminders<'a>(cache: &'a Cache, task_id: &str) -> Vec<&'a Reminder> {
    cache
        .reminders
        .iter()
        .filter(|r| !r.is_deleted && r.item_id == task_id)
        .collect()
}

// ============================================================================
// Reminders Add Command
// ============================================================================

/// Options for the reminders add command.
#[derive(Debug)]
pub struct RemindersAddOptions {
    /// Task ID (required).
    pub task: String,
    /// Absolute due date/time for the reminder.
    pub due: Option<String>,
    /// Minutes before task due time (for relative reminders).
    pub offset: Option<i32>,
}

/// Result of a successful reminder add operation.
#[derive(Debug)]
pub struct ReminderAddResult {
    /// The real ID of the created reminder.
    pub id: String,
    /// The task ID the reminder is attached to.
    pub task_id: String,
    /// The task name (content).
    pub task_name: Option<String>,
    /// The reminder type.
    pub reminder_type: ReminderType,
    /// The due date/time (for absolute reminders).
    pub due: Option<String>,
    /// Minutes before due time (for relative reminders).
    pub minute_offset: Option<i32>,
}

/// Executes the reminders add command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Reminders add command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if neither --due nor --offset is specified,
/// or if the API returns an error.
pub async fn execute_add(
    ctx: &CommandContext,
    opts: &RemindersAddOptions,
    token: &str,
) -> Result<()> {
    // Require at least one of --due or --offset
    if opts.due.is_none() && opts.offset.is_none() {
        return Err(CommandError::Config(
            "Either --due or --offset is required to create a reminder.".to_string(),
        ));
    }

    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Resolve task ID and get task name before mutation
    let (task_id, task_name) = {
        let cache = manager.cache();
        let resolved_id = resolve_task_id(cache, &opts.task)?;
        let name = cache
            .items
            .iter()
            .find(|i| i.id == resolved_id)
            .map(|i| i.content.clone());
        (resolved_id, name)
    };

    // Determine reminder type based on provided options
    let (reminder_type, args) = if let Some(ref due) = opts.due {
        // Absolute reminder
        let due_obj = serde_json::json!({
            "date": due,
        });
        let args = serde_json::json!({
            "item_id": task_id,
            "type": "absolute",
            "due": due_obj,
        });
        (ReminderType::Absolute, args)
    } else if let Some(offset) = opts.offset {
        // Relative reminder
        let args = serde_json::json!({
            "item_id": task_id,
            "type": "relative",
            "minute_offset": offset,
        });
        (ReminderType::Relative, args)
    } else {
        unreachable!("Already validated that one of due or offset is provided");
    };

    // Create the command
    let temp_id = uuid::Uuid::new_v4().to_string();
    let command = SyncCommand::with_temp_id("reminder_add", &temp_id, args);

    // Execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let response = manager.execute_commands(vec![command]).await?;

    // Check for errors
    if response.has_errors() {
        let errors = response.errors();
        if let Some((_, error)) = errors.first() {
            return Err(CommandError::Api(todoist_api::error::Error::Api(
                todoist_api::error::ApiError::Validation {
                    field: None,
                    message: format!("Error {}: {}", error.error_code, error.error),
                },
            )));
        }
    }

    // Get the real ID from the temp_id_mapping
    let real_id = response
        .real_id(&temp_id)
        .ok_or_else(|| {
            CommandError::Config("Reminder created but no ID returned in response".to_string())
        })?
        .clone();

    let result = ReminderAddResult {
        id: real_id,
        task_id,
        task_name,
        reminder_type,
        due: opts.due.clone(),
        minute_offset: opts.offset,
    };

    // Output
    if ctx.json_output {
        let output = format_created_reminder(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        let task_display = result
            .task_name
            .as_deref()
            .unwrap_or(&result.task_id);
        if ctx.verbose {
            println!("Created reminder: {}", result.id);
            println!("  Task: {}", task_display);
            println!("  Type: {}", result.reminder_type);
            if let Some(ref due) = result.due {
                println!("  Due: {}", due);
            }
            if let Some(offset) = result.minute_offset {
                println!("  Offset: {} minutes before", offset);
            }
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            let when = if let Some(ref due) = result.due {
                format!("at {}", due)
            } else if let Some(offset) = result.minute_offset {
                format_offset(offset)
            } else {
                "reminder".to_string()
            };
            println!(
                "Added {} ({}) to task: {}",
                when, prefix, task_display
            );
        }
    }

    Ok(())
}

/// Formats minute offset as a human-readable string.
fn format_offset(minutes: i32) -> String {
    if minutes == 0 {
        "at time of due date".to_string()
    } else if minutes < 60 {
        format!("{} minutes before", minutes)
    } else if minutes == 60 {
        "1 hour before".to_string()
    } else if minutes < 1440 {
        let hours = minutes / 60;
        if hours == 1 {
            "1 hour before".to_string()
        } else {
            format!("{} hours before", hours)
        }
    } else {
        let days = minutes / 1440;
        if days == 1 {
            "1 day before".to_string()
        } else {
            format!("{} days before", days)
        }
    }
}

// ============================================================================
// Reminders Delete Command
// ============================================================================

/// Options for the reminders delete command.
#[derive(Debug)]
pub struct RemindersDeleteOptions {
    /// Reminder ID (full ID or prefix).
    pub reminder_id: String,
    /// Skip confirmation.
    pub force: bool,
}

/// Result of a successful reminder delete operation.
#[derive(Debug)]
pub struct ReminderDeleteResult {
    /// The ID of the deleted reminder.
    pub id: String,
    /// The task ID the reminder was attached to.
    pub task_id: String,
    /// The task name (content).
    pub task_name: Option<String>,
    /// The reminder type.
    pub reminder_type: ReminderType,
}

/// Executes the reminders delete command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Reminders delete command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails, reminder lookup fails, or the API returns an error.
pub async fn execute_delete(
    ctx: &CommandContext,
    opts: &RemindersDeleteOptions,
    token: &str,
) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the reminder by ID or prefix and extract owned data before mutation
    let (reminder_id, task_id, reminder_type, reminder_offset, reminder_due, task_name) = {
        let cache = manager.cache();
        let reminder = find_reminder_by_id_or_prefix(cache, &opts.reminder_id)?;
        let r_id = reminder.id.clone();
        let t_id = reminder.item_id.clone();
        let r_type = reminder.reminder_type;
        let r_offset = reminder.minute_offset;
        let r_due = reminder.due.clone();
        let t_name = cache
            .items
            .iter()
            .find(|i| i.id == t_id)
            .map(|i| i.content.clone());
        (r_id, t_id, r_type, r_offset, r_due, t_name)
    };

    // Confirm if not forced
    if !opts.force && !ctx.quiet {
        let task_display = task_name
            .as_deref()
            .unwrap_or(&task_id);
        let reminder_desc = format_reminder_description(reminder_type, reminder_offset, reminder_due.as_ref());
        eprintln!(
            "Delete reminder '{}' for task '{}'?",
            reminder_desc,
            task_display
        );
        eprintln!("Use --force to skip this confirmation.");
        return Err(CommandError::Config("Operation cancelled. Use --force to confirm.".to_string()));
    }

    // Build the reminder_delete command arguments
    let args = serde_json::json!({
        "id": reminder_id,
    });

    // Create the command
    let command = SyncCommand::new("reminder_delete", args);

    // Execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let response = manager.execute_commands(vec![command]).await?;

    // Check for errors
    if response.has_errors() {
        let errors = response.errors();
        if let Some((_, error)) = errors.first() {
            return Err(CommandError::Api(todoist_api::error::Error::Api(
                todoist_api::error::ApiError::Validation {
                    field: None,
                    message: format!("Error {}: {}", error.error_code, error.error),
                },
            )));
        }
    }

    let result = ReminderDeleteResult {
        id: reminder_id,
        task_id,
        task_name,
        reminder_type,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_deleted_reminder(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        let task_display = result.task_name.as_deref().unwrap_or(&result.task_id);
        if ctx.verbose {
            println!("Deleted reminder: {} ({})", result.reminder_type, result.id);
            println!("  Task: {}", task_display);
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Deleted reminder ({}) from task: {}", prefix, task_display);
        }
    }

    Ok(())
}

/// Finds a reminder by full ID or unique prefix.
fn find_reminder_by_id_or_prefix<'a>(cache: &'a Cache, id: &str) -> Result<&'a Reminder> {
    // First try exact match
    if let Some(reminder) = cache.reminders.iter().find(|r| r.id == id && !r.is_deleted) {
        return Ok(reminder);
    }

    // Try prefix match
    let matches: Vec<&Reminder> = cache
        .reminders
        .iter()
        .filter(|r| r.id.starts_with(id) && !r.is_deleted)
        .collect();

    match matches.len() {
        0 => Err(CommandError::Config(format!("Reminder not found: {id}"))),
        1 => Ok(matches[0]),
        _ => {
            // Ambiguous prefix - provide helpful error message
            let mut msg = format!("Ambiguous reminder ID \"{id}\"\n\nMultiple reminders match this prefix:");
            for reminder in matches.iter().take(5) {
                let prefix = &reminder.id[..6.min(reminder.id.len())];
                let desc = format_reminder_description(reminder.reminder_type, reminder.minute_offset, reminder.due.as_ref());
                msg.push_str(&format!("\n  {}  {}", prefix, desc));
            }
            if matches.len() > 5 {
                msg.push_str(&format!("\n  ... and {} more", matches.len() - 5));
            }
            msg.push_str("\n\nPlease use a longer prefix.");
            Err(CommandError::Config(msg))
        }
    }
}

/// Formats a reminder description for display.
fn format_reminder_description(reminder_type: ReminderType, minute_offset: Option<i32>, due: Option<&todoist_api::sync::Due>) -> String {
    match reminder_type {
        ReminderType::Relative => {
            if let Some(offset) = minute_offset {
                format_offset(offset)
            } else {
                "relative reminder".to_string()
            }
        }
        ReminderType::Absolute => {
            if let Some(d) = due {
                format!("at {}", d.date)
            } else {
                "absolute reminder".to_string()
            }
        }
        ReminderType::Location => "location-based reminder".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use todoist_api::models::Due;
    use todoist_api::sync::{Item, Project};

    #[test]
    fn test_reminders_list_options_defaults() {
        let opts = RemindersListOptions::default();
        assert!(opts.task.is_none());
    }

    #[test]
    fn test_reminders_list_options_with_task() {
        let opts = RemindersListOptions {
            task: Some("task-123".to_string()),
        };
        assert_eq!(opts.task, Some("task-123".to_string()));
    }

    #[test]
    fn test_filter_reminders_by_task() {
        let cache = make_test_cache();
        let reminders = filter_reminders(&cache, "task-1");

        assert_eq!(reminders.len(), 2);
    }

    #[test]
    fn test_filter_reminders_excludes_deleted() {
        let mut cache = make_test_cache();
        cache.reminders[0].is_deleted = true;

        let reminders = filter_reminders(&cache, "task-1");

        assert_eq!(reminders.len(), 1);
    }

    #[test]
    fn test_filter_reminders_no_match() {
        let cache = make_test_cache();
        let reminders = filter_reminders(&cache, "nonexistent");

        assert!(reminders.is_empty());
    }

    #[test]
    fn test_resolve_task_id_exact_match() {
        let cache = make_test_cache();
        let result = resolve_task_id(&cache, "task-1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "task-1");
    }

    #[test]
    fn test_resolve_task_id_not_found() {
        let cache = make_test_cache();
        let result = resolve_task_id(&cache, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Task not found"));
    }

    #[test]
    fn test_resolve_task_id_prefix_match() {
        let mut cache = make_test_cache();
        cache.items.push(Item {
            id: "task-abc123def456".to_string(),
            user_id: None,
            project_id: "project-1".to_string(),
            content: "Prefixed task".to_string(),
            description: String::new(),
            priority: 1,
            due: None,
            deadline: None,
            parent_id: None,
            child_order: 0,
            section_id: None,
            day_order: 0,
            is_collapsed: false,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            updated_at: None,
            completed_at: None,
            duration: None,
        });

        // Should match with 6+ char prefix
        let result = resolve_task_id(&cache, "task-a");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "task-abc123def456");
    }

    // Helper function to create a test cache
    fn make_test_cache() -> Cache {
        Cache {
            sync_token: "test".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: vec![make_test_item("task-1", "Test Task", "project-1")],
            projects: vec![make_test_project("project-1", "Test Project")],
            labels: vec![],
            sections: vec![],
            notes: vec![],
            project_notes: vec![],
            reminders: vec![
                Reminder {
                    id: "reminder-1".to_string(),
                    item_id: "task-1".to_string(),
                    reminder_type: ReminderType::Relative,
                    due: None,
                    minute_offset: Some(30),
                    is_deleted: false,
                    notify_uid: None,
                    name: None,
                    loc_lat: None,
                    loc_long: None,
                    loc_trigger: None,
                    radius: None,
                },
                Reminder {
                    id: "reminder-2".to_string(),
                    item_id: "task-1".to_string(),
                    reminder_type: ReminderType::Absolute,
                    due: Some(Due {
                        date: "2025-01-26".to_string(),
                        datetime: Some("2025-01-26T10:00:00Z".to_string()),
                        timezone: Some("UTC".to_string()),
                        string: None,
                        is_recurring: false,
                        lang: None,
                    }),
                    minute_offset: None,
                    is_deleted: false,
                    notify_uid: None,
                    name: None,
                    loc_lat: None,
                    loc_long: None,
                    loc_trigger: None,
                    radius: None,
                },
            ],
            filters: vec![],
            user: None,
        }
    }

    fn make_test_item(id: &str, content: &str, project_id: &str) -> Item {
        Item {
            id: id.to_string(),
            user_id: None,
            project_id: project_id.to_string(),
            content: content.to_string(),
            description: String::new(),
            priority: 1,
            due: None,
            deadline: None,
            parent_id: None,
            child_order: 0,
            section_id: None,
            day_order: 0,
            is_collapsed: false,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            updated_at: None,
            completed_at: None,
            duration: None,
        }
    }

    fn make_test_project(id: &str, name: &str) -> Project {
        Project {
            id: id.to_string(),
            name: name.to_string(),
            color: None,
            parent_id: None,
            child_order: 0,
            is_collapsed: false,
            shared: false,
            can_assign_tasks: false,
            is_deleted: false,
            is_archived: false,
            is_favorite: false,
            view_style: None,
            inbox_project: false,
            folder_id: None,
            created_at: None,
            updated_at: None,
        }
    }

    // ========================================================================
    // Reminders Add Tests
    // ========================================================================

    #[test]
    fn test_reminders_add_options_with_due() {
        let opts = RemindersAddOptions {
            task: "task-123".to_string(),
            due: Some("2025-01-26T10:00:00".to_string()),
            offset: None,
        };

        assert_eq!(opts.task, "task-123");
        assert_eq!(opts.due, Some("2025-01-26T10:00:00".to_string()));
        assert!(opts.offset.is_none());
    }

    #[test]
    fn test_reminders_add_options_with_offset() {
        let opts = RemindersAddOptions {
            task: "task-456".to_string(),
            due: None,
            offset: Some(30),
        };

        assert_eq!(opts.task, "task-456");
        assert!(opts.due.is_none());
        assert_eq!(opts.offset, Some(30));
    }

    #[test]
    fn test_reminder_add_result_absolute() {
        let result = ReminderAddResult {
            id: "reminder-123".to_string(),
            task_id: "task-1".to_string(),
            task_name: Some("My Task".to_string()),
            reminder_type: ReminderType::Absolute,
            due: Some("2025-01-26T10:00:00".to_string()),
            minute_offset: None,
        };

        assert_eq!(result.id, "reminder-123");
        assert_eq!(result.task_id, "task-1");
        assert_eq!(result.task_name, Some("My Task".to_string()));
        assert_eq!(result.reminder_type, ReminderType::Absolute);
        assert_eq!(result.due, Some("2025-01-26T10:00:00".to_string()));
        assert!(result.minute_offset.is_none());
    }

    #[test]
    fn test_reminder_add_result_relative() {
        let result = ReminderAddResult {
            id: "reminder-456".to_string(),
            task_id: "task-2".to_string(),
            task_name: Some("Another Task".to_string()),
            reminder_type: ReminderType::Relative,
            due: None,
            minute_offset: Some(60),
        };

        assert_eq!(result.id, "reminder-456");
        assert_eq!(result.task_id, "task-2");
        assert_eq!(result.task_name, Some("Another Task".to_string()));
        assert_eq!(result.reminder_type, ReminderType::Relative);
        assert!(result.due.is_none());
        assert_eq!(result.minute_offset, Some(60));
    }

    #[test]
    fn test_format_offset_zero() {
        assert_eq!(format_offset(0), "at time of due date");
    }

    #[test]
    fn test_format_offset_minutes() {
        assert_eq!(format_offset(30), "30 minutes before");
        assert_eq!(format_offset(45), "45 minutes before");
    }

    #[test]
    fn test_format_offset_one_hour() {
        assert_eq!(format_offset(60), "1 hour before");
    }

    #[test]
    fn test_format_offset_hours() {
        assert_eq!(format_offset(120), "2 hours before");
        assert_eq!(format_offset(180), "3 hours before");
    }

    #[test]
    fn test_format_offset_one_day() {
        assert_eq!(format_offset(1440), "1 day before");
    }

    #[test]
    fn test_format_offset_days() {
        assert_eq!(format_offset(2880), "2 days before");
        assert_eq!(format_offset(4320), "3 days before");
    }

    // ========================================================================
    // Reminders Delete Tests
    // ========================================================================

    #[test]
    fn test_reminders_delete_options() {
        let opts = RemindersDeleteOptions {
            reminder_id: "reminder-123".to_string(),
            force: false,
        };

        assert_eq!(opts.reminder_id, "reminder-123");
        assert!(!opts.force);
    }

    #[test]
    fn test_reminders_delete_options_with_force() {
        let opts = RemindersDeleteOptions {
            reminder_id: "reminder-456".to_string(),
            force: true,
        };

        assert_eq!(opts.reminder_id, "reminder-456");
        assert!(opts.force);
    }

    #[test]
    fn test_reminder_delete_result() {
        let result = ReminderDeleteResult {
            id: "reminder-789".to_string(),
            task_id: "task-1".to_string(),
            task_name: Some("Test Task".to_string()),
            reminder_type: ReminderType::Relative,
        };

        assert_eq!(result.id, "reminder-789");
        assert_eq!(result.task_id, "task-1");
        assert_eq!(result.task_name, Some("Test Task".to_string()));
        assert_eq!(result.reminder_type, ReminderType::Relative);
    }

    #[test]
    fn test_find_reminder_by_id_or_prefix_exact_match() {
        let cache = make_test_cache();
        let result = find_reminder_by_id_or_prefix(&cache, "reminder-1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "reminder-1");
    }

    #[test]
    fn test_find_reminder_by_id_or_prefix_prefix_match() {
        let cache = make_test_cache();
        let result = find_reminder_by_id_or_prefix(&cache, "reminder-2");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "reminder-2");
    }

    #[test]
    fn test_find_reminder_by_id_or_prefix_not_found() {
        let cache = make_test_cache();
        let result = find_reminder_by_id_or_prefix(&cache, "nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Reminder not found"));
    }

    #[test]
    fn test_find_reminder_by_id_or_prefix_ambiguous() {
        let cache = make_test_cache();
        // Both reminder-1 and reminder-2 start with "reminder-"
        let result = find_reminder_by_id_or_prefix(&cache, "reminder-");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ambiguous"));
    }

    #[test]
    fn test_find_reminder_by_id_or_prefix_ignores_deleted() {
        let mut cache = make_test_cache();
        cache.reminders[0].is_deleted = true;

        let result = find_reminder_by_id_or_prefix(&cache, "reminder-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_format_reminder_description_relative() {
        assert_eq!(
            format_reminder_description(ReminderType::Relative, Some(30), None),
            "30 minutes before"
        );
        assert_eq!(
            format_reminder_description(ReminderType::Relative, Some(60), None),
            "1 hour before"
        );
        assert_eq!(
            format_reminder_description(ReminderType::Relative, None, None),
            "relative reminder"
        );
    }

    #[test]
    fn test_format_reminder_description_absolute() {
        let due = Due {
            date: "2025-01-26".to_string(),
            datetime: Some("2025-01-26T10:00:00Z".to_string()),
            timezone: Some("UTC".to_string()),
            string: None,
            is_recurring: false,
            lang: None,
        };
        assert_eq!(
            format_reminder_description(ReminderType::Absolute, None, Some(&due)),
            "at 2025-01-26"
        );
        assert_eq!(
            format_reminder_description(ReminderType::Absolute, None, None),
            "absolute reminder"
        );
    }

    #[test]
    fn test_format_reminder_description_location() {
        assert_eq!(
            format_reminder_description(ReminderType::Location, None, None),
            "location-based reminder"
        );
    }
}
