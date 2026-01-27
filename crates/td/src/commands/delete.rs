//! Delete command implementation.
//!
//! Deletes task(s) via the Sync API's `item_delete` command.
//! Uses SyncManager::execute_commands() to automatically update the cache.
//! Uses resolve_item_by_prefix() for smart lookups with auto-sync fallback.

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::SyncCommand;
use todoist_cache_rs::{CacheStore, SyncManager};

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

    // Resolve all task IDs using smart lookup (cache-first with auto-sync fallback)
    // require_checked=None to match any task (delete works on completed and uncompleted)
    let mut resolved_items: Vec<(String, String)> = Vec::new();
    for task_id in &opts.task_ids {
        let item = manager
            .resolve_item_by_prefix(task_id, None)
            .await
            .map_err(|e| CommandError::Config(e.to_string()))?;
        resolved_items.push((item.id.clone(), item.content.clone()));
    }

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

    // Note: Tests for item lookup by prefix are now in SyncManager tests
    // (resolve_item_by_prefix covers exact match, prefix match, not found,
    // ambiguous, deleted items, and completion status filtering)

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
}
