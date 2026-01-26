//! Delete command implementation.
//!
//! Deletes task(s) via the Sync API's `item_delete` command.
//! Uses SyncManager::execute_commands() to automatically update the cache.

use todoist_api::client::TodoistClient;
use todoist_api::sync::{Item, SyncCommand};
use todoist_cache::{Cache, CacheStore, SyncManager};

use super::{confirm_bulk_operation, CommandContext, CommandError, ConfirmResult, Result};

/// Options for the delete command.
#[derive(Debug)]
pub struct DeleteOptions {
    /// Task IDs (full IDs or prefixes).
    pub task_ids: Vec<String>,
    /// Skip confirmation prompt.
    pub force: bool,
}

/// Result of deleting a single task.
#[derive(Debug)]
pub struct DeleteResult {
    /// The task ID.
    pub id: String,
    /// The task content.
    pub content: String,
    /// Whether the operation succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Executes the delete command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Delete command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails, task lookup fails, or the API returns an error.
pub async fn execute(ctx: &CommandContext, opts: &DeleteOptions, token: &str) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Resolve all task IDs and collect owned data (supporting prefix matching)
    // We clone the data we need so we don't hold references to the cache
    let resolved_items: Vec<(String, String)> = {
        let cache = manager.cache();
        let mut items = Vec::new();
        for task_id in &opts.task_ids {
            let item = find_item_by_id_or_prefix(cache, task_id)?;
            items.push((item.id.clone(), item.content.clone()));
        }
        items
    };

    // Prompt for confirmation if multiple tasks
    let items_for_confirm: Vec<(&str, &str)> = resolved_items
        .iter()
        .map(|(id, content)| {
            let id_prefix = &id[..6.min(id.len())];
            (id_prefix, content.as_str())
        })
        .collect();

    match confirm_bulk_operation("delete", &items_for_confirm, opts.force, ctx.quiet)? {
        ConfirmResult::Confirmed => {}
        ConfirmResult::Aborted => {
            if !ctx.quiet {
                eprintln!("Aborted.");
            }
            return Ok(());
        }
    }

    // Build commands for all tasks using item_delete
    let commands: Vec<SyncCommand> = resolved_items
        .iter()
        .map(|(id, _)| SyncCommand::new("item_delete", serde_json::json!({ "id": id })))
        .collect();

    // Execute the commands via SyncManager
    // This sends the commands, applies the response to cache, and saves to disk
    let response = manager.execute_commands(commands).await?;

    // Process results
    let mut results: Vec<DeleteResult> = Vec::new();
    let mut success_count = 0;
    let mut error_count = 0;

    for (id, content) in &resolved_items {
        // Check sync_status for this command
        let has_error = response.errors().iter().any(|(_, err)| {
            err.error.contains(id)
        });

        if has_error {
            let error_msg = response
                .errors()
                .iter()
                .find(|(_, err)| err.error.contains(id))
                .map(|(_, err)| format!("{}: {}", err.error_code, err.error));

            results.push(DeleteResult {
                id: id.clone(),
                content: content.clone(),
                success: false,
                error: error_msg,
            });
            error_count += 1;
        } else {
            results.push(DeleteResult {
                id: id.clone(),
                content: content.clone(),
                success: true,
                error: None,
            });
            success_count += 1;
        }
    }

    // Output results
    if ctx.json_output {
        let output = format_delete_results_json(&results)?;
        println!("{output}");
    } else if !ctx.quiet {
        for result in &results {
            let id_prefix = &result.id[..6.min(result.id.len())];
            if result.success {
                println!("Deleted: {} ({})", result.content, id_prefix);
            } else if let Some(ref err) = result.error {
                eprintln!(
                    "Failed to delete {} ({}): {}",
                    result.content, id_prefix, err
                );
            }
        }

        if ctx.verbose && results.len() > 1 {
            println!("\n{} deleted, {} failed", success_count, error_count);
        }
    }

    // Return error if all tasks failed
    if error_count > 0 && success_count == 0 {
        return Err(CommandError::Config(format!(
            "Failed to delete {} task(s)",
            error_count
        )));
    }

    Ok(())
}

/// Finds an item by full ID or unique prefix.
/// Searches both completed and uncompleted tasks (delete works on any non-deleted task).
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

/// Formats delete results as JSON.
fn format_delete_results_json(results: &[DeleteResult]) -> Result<String> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct DeleteOutput<'a> {
        deleted: Vec<DeletedTaskOutput<'a>>,
        failed: Vec<FailedTaskOutput<'a>>,
        total_deleted: usize,
        total_failed: usize,
    }

    #[derive(Serialize)]
    struct DeletedTaskOutput<'a> {
        id: &'a str,
        content: &'a str,
    }

    #[derive(Serialize)]
    struct FailedTaskOutput<'a> {
        id: &'a str,
        content: &'a str,
        error: Option<&'a str>,
    }

    let deleted: Vec<DeletedTaskOutput> = results
        .iter()
        .filter(|r| r.success)
        .map(|r| DeletedTaskOutput {
            id: &r.id,
            content: &r.content,
        })
        .collect();

    let failed: Vec<FailedTaskOutput> = results
        .iter()
        .filter(|r| !r.success)
        .map(|r| FailedTaskOutput {
            id: &r.id,
            content: &r.content,
            error: r.error.as_deref(),
        })
        .collect();

    let output = DeleteOutput {
        total_deleted: deleted.len(),
        total_failed: failed.len(),
        deleted,
        failed,
    };

    serde_json::to_string_pretty(&output).map_err(CommandError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_options_single_task() {
        let opts = DeleteOptions {
            task_ids: vec!["abc123".to_string()],
            force: false,
        };

        assert_eq!(opts.task_ids.len(), 1);
        assert!(!opts.force);
    }

    #[test]
    fn test_delete_options_multiple_tasks() {
        let opts = DeleteOptions {
            task_ids: vec![
                "abc123".to_string(),
                "def456".to_string(),
                "ghi789".to_string(),
            ],
            force: true,
        };

        assert_eq!(opts.task_ids.len(), 3);
        assert!(opts.force);
    }

    #[test]
    fn test_delete_result_success() {
        let result = DeleteResult {
            id: "abc123".to_string(),
            content: "Test task".to_string(),
            success: true,
            error: None,
        };

        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_delete_result_failure() {
        let result = DeleteResult {
            id: "abc123".to_string(),
            content: "Test task".to_string(),
            success: false,
            error: Some("Task not found".to_string()),
        };

        assert!(!result.success);
        assert!(result.error.is_some());
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
        cache.items[0].is_deleted = true;

        let result = find_item_by_id_or_prefix(&cache, "item-123");
        assert!(result.is_err());
    }

    #[test]
    fn test_find_item_by_id_or_prefix_includes_completed() {
        let mut cache = make_test_cache();
        cache.items[0].checked = true;

        // Delete should work on completed tasks too
        let result = find_item_by_id_or_prefix(&cache, "item-123");
        assert!(result.is_ok());
    }

    #[test]
    fn test_format_delete_results_json() {
        let results = vec![
            DeleteResult {
                id: "abc123".to_string(),
                content: "Task 1".to_string(),
                success: true,
                error: None,
            },
            DeleteResult {
                id: "def456".to_string(),
                content: "Task 2".to_string(),
                success: false,
                error: Some("Not found".to_string()),
            },
        ];

        let json = format_delete_results_json(&results).unwrap();
        assert!(json.contains("\"total_deleted\": 1"));
        assert!(json.contains("\"total_failed\": 1"));
        assert!(json.contains("Task 1"));
        assert!(json.contains("Task 2"));
        assert!(json.contains("Not found"));
    }

    // Helper function to create a test cache
    fn make_test_cache() -> Cache {
        Cache {
            sync_token: "test".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: vec![make_test_item("item-123-abc", "Test task", false)],
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
                make_test_item("item-aaa-111", "Task 1", false),
                make_test_item("item-aaa-222", "Task 2", false),
                make_test_item("item-bbb-333", "Task 3", false),
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

    fn make_test_item(id: &str, content: &str, checked: bool) -> todoist_api::sync::Item {
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
            checked,
            is_deleted: false,
            added_at: None,
            updated_at: None,
            completed_at: None,
            duration: None,
        }
    }
}
