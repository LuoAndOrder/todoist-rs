//! Reminders command implementation.
//!
//! Lists reminders via the Sync API.

use chrono::Utc;
use todoist_api::client::TodoistClient;
use todoist_api::sync::Reminder;
use todoist_cache::{Cache, CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};
use crate::output::{format_reminders_json, format_reminders_table};

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

    // Sync if needed
    let now = Utc::now();
    if manager.needs_sync(now) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use todoist_api::sync::{Due, Item, Project};

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
                    reminder_type: "relative".to_string(),
                    due: None,
                    minute_offset: Some(30),
                    is_deleted: false,
                },
                Reminder {
                    id: "reminder-2".to_string(),
                    item_id: "task-1".to_string(),
                    reminder_type: "absolute".to_string(),
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
}
