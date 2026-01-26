//! Reopen command implementation.
//!
//! Reopens completed task(s) via the Sync API's `item_uncomplete` command.

use todoist_api::client::TodoistClient;
use todoist_api::sync::{Item, SyncCommand, SyncRequest};
use todoist_cache::{Cache, CacheStore, SyncManager};

use super::{confirm_bulk_operation, CommandContext, CommandError, ConfirmResult, Result};

/// Options for the reopen command.
#[derive(Debug)]
pub struct ReopenOptions {
    /// Task IDs (full IDs or prefixes).
    pub task_ids: Vec<String>,
    /// Skip confirmation for multiple tasks.
    pub force: bool,
}

/// Result of reopening a single task.
#[derive(Debug)]
pub struct ReopenResult {
    /// The task ID.
    pub id: String,
    /// The task content.
    pub content: String,
    /// Whether the operation succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Executes the reopen command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Reopen command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails, task lookup fails, or the API returns an error.
pub async fn execute(ctx: &CommandContext, opts: &ReopenOptions, token: &str) -> Result<()> {
    // Initialize sync manager
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Always sync to ensure we have the latest data
    // This is important because tasks may have been completed recently
    if ctx.verbose {
        eprintln!("Syncing with Todoist...");
    }
    manager.sync().await?;

    let cache = manager.cache();

    // Resolve all task IDs (supporting prefix matching)
    let mut resolved_items: Vec<(&Item, String)> = Vec::new();
    for task_id in &opts.task_ids {
        let item = find_completed_item_by_id_or_prefix(cache, task_id)?;
        resolved_items.push((item, task_id.clone()));
    }

    // Prompt for confirmation if multiple tasks
    let items_for_confirm: Vec<(&str, &str)> = resolved_items
        .iter()
        .map(|(item, _)| {
            let id_prefix = &item.id[..6.min(item.id.len())];
            (id_prefix, item.content.as_str())
        })
        .collect();

    match confirm_bulk_operation("reopen", &items_for_confirm, opts.force, ctx.quiet)? {
        ConfirmResult::Confirmed => {}
        ConfirmResult::Aborted => {
            if !ctx.quiet {
                eprintln!("Aborted.");
            }
            return Ok(());
        }
    }

    // Build commands for all tasks using item_uncomplete
    let commands: Vec<SyncCommand> = resolved_items
        .iter()
        .map(|(item, _)| SyncCommand::new("item_uncomplete", serde_json::json!({ "id": item.id })))
        .collect();

    // Execute the commands
    let api_client = TodoistClient::new(token);
    let request = SyncRequest::with_commands(commands);
    let response = api_client.sync(request).await?;

    // Process results
    let mut results: Vec<ReopenResult> = Vec::new();
    let mut success_count = 0;
    let mut error_count = 0;

    for (item, _original_id) in &resolved_items {
        // Check sync_status for this command
        // Note: We need to match by item ID in the response errors if any
        let has_error = response.errors().iter().any(|(_, err)| {
            // Check if error message contains this item's ID
            err.error.contains(&item.id)
        });

        if has_error {
            let error_msg = response
                .errors()
                .iter()
                .find(|(_, err)| err.error.contains(&item.id))
                .map(|(_, err)| format!("{}: {}", err.error_code, err.error));

            results.push(ReopenResult {
                id: item.id.clone(),
                content: item.content.clone(),
                success: false,
                error: error_msg,
            });
            error_count += 1;
        } else {
            results.push(ReopenResult {
                id: item.id.clone(),
                content: item.content.clone(),
                success: true,
                error: None,
            });
            success_count += 1;
        }
    }

    // Output results
    if ctx.json_output {
        let output = format_reopen_results_json(&results)?;
        println!("{output}");
    } else if !ctx.quiet {
        for result in &results {
            let id_prefix = &result.id[..6.min(result.id.len())];
            if result.success {
                println!("Reopened: {} ({})", result.content, id_prefix);
            } else if let Some(ref err) = result.error {
                eprintln!(
                    "Failed to reopen {} ({}): {}",
                    result.content, id_prefix, err
                );
            }
        }

        if ctx.verbose && results.len() > 1 {
            println!("\n{} reopened, {} failed", success_count, error_count);
        }
    }

    // Return error if any tasks failed
    if error_count > 0 && success_count == 0 {
        return Err(CommandError::Config(format!(
            "Failed to reopen {} task(s)",
            error_count
        )));
    }

    Ok(())
}

/// Finds a completed item by full ID or unique prefix.
fn find_completed_item_by_id_or_prefix<'a>(cache: &'a Cache, id: &str) -> Result<&'a Item> {
    // First try exact match on completed items
    if let Some(item) = cache
        .items
        .iter()
        .find(|i| i.id == id && !i.is_deleted && i.checked)
    {
        return Ok(item);
    }

    // Try prefix match on completed items
    let matches: Vec<&Item> = cache
        .items
        .iter()
        .filter(|i| i.id.starts_with(id) && !i.is_deleted && i.checked)
        .collect();

    match matches.len() {
        0 => Err(CommandError::Config(format!(
            "Completed task not found: {id}"
        ))),
        1 => Ok(matches[0]),
        _ => {
            // Ambiguous prefix - provide helpful error message
            let mut msg =
                format!("Ambiguous task ID \"{id}\"\n\nMultiple completed tasks match this prefix:");
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

/// Formats reopen results as JSON.
fn format_reopen_results_json(results: &[ReopenResult]) -> Result<String> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct ReopenOutput<'a> {
        reopened: Vec<ReopenedTaskOutput<'a>>,
        failed: Vec<FailedTaskOutput<'a>>,
        total_reopened: usize,
        total_failed: usize,
    }

    #[derive(Serialize)]
    struct ReopenedTaskOutput<'a> {
        id: &'a str,
        content: &'a str,
    }

    #[derive(Serialize)]
    struct FailedTaskOutput<'a> {
        id: &'a str,
        content: &'a str,
        error: Option<&'a str>,
    }

    let reopened: Vec<ReopenedTaskOutput> = results
        .iter()
        .filter(|r| r.success)
        .map(|r| ReopenedTaskOutput {
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

    let output = ReopenOutput {
        total_reopened: reopened.len(),
        total_failed: failed.len(),
        reopened,
        failed,
    };

    serde_json::to_string_pretty(&output).map_err(CommandError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reopen_options_single_task() {
        let opts = ReopenOptions {
            task_ids: vec!["abc123".to_string()],
            force: false,
        };

        assert_eq!(opts.task_ids.len(), 1);
        assert!(!opts.force);
    }

    #[test]
    fn test_reopen_options_multiple_tasks() {
        let opts = ReopenOptions {
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
    fn test_reopen_result_success() {
        let result = ReopenResult {
            id: "abc123".to_string(),
            content: "Test task".to_string(),
            success: true,
            error: None,
        };

        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_reopen_result_failure() {
        let result = ReopenResult {
            id: "abc123".to_string(),
            content: "Test task".to_string(),
            success: false,
            error: Some("Task not found".to_string()),
        };

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_find_completed_item_by_id_or_prefix_exact_match() {
        let cache = make_test_cache();
        let result = find_completed_item_by_id_or_prefix(&cache, "item-123-abc");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "item-123-abc");
    }

    #[test]
    fn test_find_completed_item_by_id_or_prefix_unique_prefix() {
        let cache = make_test_cache();
        let result = find_completed_item_by_id_or_prefix(&cache, "item-123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "item-123-abc");
    }

    #[test]
    fn test_find_completed_item_by_id_or_prefix_not_found() {
        let cache = make_test_cache();
        let result = find_completed_item_by_id_or_prefix(&cache, "nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Completed task not found"));
    }

    #[test]
    fn test_find_completed_item_by_id_or_prefix_ambiguous() {
        let cache = make_cache_with_ambiguous_ids();
        let result = find_completed_item_by_id_or_prefix(&cache, "item-");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ambiguous"));
    }

    #[test]
    fn test_find_completed_item_by_id_or_prefix_ignores_deleted() {
        let mut cache = make_test_cache();
        cache.items[0].is_deleted = true;

        let result = find_completed_item_by_id_or_prefix(&cache, "item-123");
        assert!(result.is_err());
    }

    #[test]
    fn test_find_completed_item_by_id_or_prefix_ignores_uncompleted() {
        let mut cache = make_test_cache();
        cache.items[0].checked = false;

        let result = find_completed_item_by_id_or_prefix(&cache, "item-123");
        assert!(result.is_err());
    }

    #[test]
    fn test_format_reopen_results_json() {
        let results = vec![
            ReopenResult {
                id: "abc123".to_string(),
                content: "Task 1".to_string(),
                success: true,
                error: None,
            },
            ReopenResult {
                id: "def456".to_string(),
                content: "Task 2".to_string(),
                success: false,
                error: Some("Not found".to_string()),
            },
        ];

        let json = format_reopen_results_json(&results).unwrap();
        assert!(json.contains("\"total_reopened\": 1"));
        assert!(json.contains("\"total_failed\": 1"));
        assert!(json.contains("Task 1"));
        assert!(json.contains("Task 2"));
        assert!(json.contains("Not found"));
    }

    // Helper function to create a test cache with completed items
    fn make_test_cache() -> Cache {
        Cache {
            sync_token: "test".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: vec![make_test_item("item-123-abc", "Test task", true)],
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
                make_test_item("item-aaa-111", "Task 1", true),
                make_test_item("item-aaa-222", "Task 2", true),
                make_test_item("item-bbb-333", "Task 3", true),
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
