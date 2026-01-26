//! Show command implementation.
//!
//! Displays detailed information about a task from the local cache.

use chrono::Utc;
use todoist_api::sync::{Item, Note, Reminder};
use todoist_cache::{Cache, CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};
use crate::output::{format_item_details_json, format_item_details_table};

/// Options for the show command.
#[derive(Debug)]
pub struct ShowOptions {
    /// Task ID (full ID or prefix).
    pub task_id: String,
    /// Include comments.
    pub comments: bool,
    /// Include reminders.
    pub reminders: bool,
}

/// Result data for the show command.
pub struct ShowResult<'a> {
    /// The task item.
    pub item: &'a Item,
    /// The project name.
    pub project_name: Option<String>,
    /// The section name.
    pub section_name: Option<String>,
    /// Labels (as names).
    pub labels: Vec<String>,
    /// Comments for this task.
    pub comments: Vec<&'a Note>,
    /// Reminders for this task.
    pub reminders: Vec<&'a Reminder>,
    /// Subtasks of this task.
    pub subtasks: Vec<&'a Item>,
}

/// Executes the show command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Show command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails or if the task is not found.
pub async fn execute(ctx: &CommandContext, opts: &ShowOptions, token: &str) -> Result<()> {
    // Initialize sync manager
    let client = todoist_api::client::TodoistClient::new(token);
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

    // Find the task by ID or prefix
    let item = find_item_by_id_or_prefix(cache, &opts.task_id)?;

    // Get related data
    let project_name = cache
        .projects
        .iter()
        .find(|p| p.id == item.project_id)
        .map(|p| p.name.clone());

    let section_name = item.section_id.as_ref().and_then(|sid| {
        cache
            .sections
            .iter()
            .find(|s| &s.id == sid)
            .map(|s| s.name.clone())
    });

    // Get comments for this task if requested
    let comments: Vec<&Note> = if opts.comments {
        cache
            .notes
            .iter()
            .filter(|n| n.item_id == item.id && !n.is_deleted)
            .collect()
    } else {
        vec![]
    };

    // Get reminders for this task if requested
    let reminders: Vec<&Reminder> = if opts.reminders {
        cache
            .reminders
            .iter()
            .filter(|r| r.item_id == item.id && !r.is_deleted)
            .collect()
    } else {
        vec![]
    };

    // Get subtasks
    let subtasks: Vec<&Item> = cache
        .items
        .iter()
        .filter(|i| i.parent_id.as_ref() == Some(&item.id) && !i.is_deleted && !i.checked)
        .collect();

    let result = ShowResult {
        item,
        project_name,
        section_name,
        labels: item.labels.clone(),
        comments,
        reminders,
        subtasks,
    };

    // Output
    if ctx.json_output {
        let output = format_item_details_json(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_item_details_table(&result, ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

/// Finds an item by full ID or unique prefix.
fn find_item_by_id_or_prefix<'a>(cache: &'a Cache, id: &str) -> Result<&'a Item> {
    // First try exact match
    if let Some(item) = cache.items.iter().find(|i| i.id == id && !i.is_deleted) {
        return Ok(item);
    }

    // Try prefix match
    let matches: Vec<&Item> = cache
        .items
        .iter()
        .filter(|i| i.id.starts_with(id) && !i.is_deleted)
        .collect();

    match matches.len() {
        0 => Err(CommandError::Config(format!("Task not found: {id}"))),
        1 => Ok(matches[0]),
        _ => {
            // Ambiguous prefix - provide helpful error message
            let mut msg = format!("Ambiguous task ID \"{id}\"\n\nMultiple tasks match this prefix:");
            for item in matches.iter().take(5) {
                let prefix = &item.id[..6.min(item.id.len())];
                msg.push_str(&format!("\n  {}  {}", prefix, item.content));
            }
            if matches.len() > 5 {
                msg.push_str(&format!("\n  ... and {} more", matches.len() - 5));
            }
            msg.push_str("\n\nPlease use a longer prefix.");
            Err(CommandError::Config(msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_options_defaults() {
        let opts = ShowOptions {
            task_id: "abc123".to_string(),
            comments: false,
            reminders: false,
        };

        assert_eq!(opts.task_id, "abc123");
        assert!(!opts.comments);
        assert!(!opts.reminders);
    }

    #[test]
    fn test_show_options_with_all_flags() {
        let opts = ShowOptions {
            task_id: "abc123def456".to_string(),
            comments: true,
            reminders: true,
        };

        assert_eq!(opts.task_id, "abc123def456");
        assert!(opts.comments);
        assert!(opts.reminders);
    }

    #[test]
    fn test_find_item_by_id_or_prefix_exact_match() {
        let cache = make_test_cache();
        let result = find_item_by_id_or_prefix(&cache, "item-123-abc");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "item-123-abc");
    }

    #[test]
    fn test_find_item_by_id_or_prefix_unique_prefix() {
        let cache = make_test_cache();
        let result = find_item_by_id_or_prefix(&cache, "item-123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "item-123-abc");
    }

    #[test]
    fn test_find_item_by_id_or_prefix_not_found() {
        let cache = make_test_cache();
        let result = find_item_by_id_or_prefix(&cache, "nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Task not found"));
    }

    #[test]
    fn test_find_item_by_id_or_prefix_ambiguous() {
        let cache = make_cache_with_ambiguous_ids();
        let result = find_item_by_id_or_prefix(&cache, "item-");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ambiguous"));
    }

    #[test]
    fn test_find_item_by_id_or_prefix_ignores_deleted() {
        let mut cache = make_test_cache();
        // Mark the item as deleted
        cache.items[0].is_deleted = true;

        let result = find_item_by_id_or_prefix(&cache, "item-123");
        assert!(result.is_err());
    }

    // Helper function to create a test cache
    fn make_test_cache() -> Cache {
        Cache {
            sync_token: "test".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: vec![make_test_item("item-123-abc", "Test task")],
            projects: vec![],
            labels: vec![],
            sections: vec![],
            notes: vec![],
            project_notes: vec![],
            reminders: vec![],
            filters: vec![],
            user: None,
        }
    }

    fn make_cache_with_ambiguous_ids() -> Cache {
        Cache {
            sync_token: "test".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: vec![
                make_test_item("item-aaa-111", "Task 1"),
                make_test_item("item-aaa-222", "Task 2"),
                make_test_item("item-bbb-333", "Task 3"),
            ],
            projects: vec![],
            labels: vec![],
            sections: vec![],
            notes: vec![],
            project_notes: vec![],
            reminders: vec![],
            filters: vec![],
            user: None,
        }
    }

    fn make_test_item(id: &str, content: &str) -> todoist_api::sync::Item {
        todoist_api::sync::Item {
            id: id.to_string(),
            user_id: None,
            project_id: "proj-1".to_string(),
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
}
