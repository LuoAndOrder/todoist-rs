//! Reopen command implementation.
//!
//! Reopens completed task(s) via the Sync API's `item_uncomplete` command.
//! Uses SyncManager::execute_commands() to automatically update the cache.
//! Uses resolve_item_by_prefix() for smart lookups with auto-sync fallback.

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::SyncCommand;
use todoist_cache_rs::{CacheStore, SyncManager};

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
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Resolve all task IDs using smart lookup (cache-first with auto-sync fallback)
    // require_checked=Some(true) to only find completed tasks (reopen only makes sense for completed tasks)
    let mut resolved_items: Vec<(String, String)> = Vec::new();
    for task_id in &opts.task_ids {
        let item = manager
            .resolve_item_by_prefix(task_id, Some(true))
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
        .map(|(id, _)| SyncCommand::new("item_uncomplete", serde_json::json!({ "id": id })))
        .collect();

    // Execute the commands via SyncManager
    // This sends the commands, applies the response to cache, and saves to disk
    let response = manager.execute_commands(commands).await?;

    // Process results
    let mut results: Vec<ReopenResult> = Vec::new();
    let mut success_count = 0;
    let mut error_count = 0;

    for (id, content) in &resolved_items {
        // Check sync_status for this command
        // Note: We need to match by item ID in the response errors if any
        let has_error = response.errors().iter().any(|(_, err)| {
            // Check if error message contains this item's ID
            err.error.contains(id)
        });

        if has_error {
            let error_msg = response
                .errors()
                .iter()
                .find(|(_, err)| err.error.contains(id))
                .map(|(_, err)| format!("{}: {}", err.error_code, err.error));

            results.push(ReopenResult {
                id: id.clone(),
                content: content.clone(),
                success: false,
                error: error_msg,
            });
            error_count += 1;
        } else {
            results.push(ReopenResult {
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

    // Note: Tests for item lookup by prefix are now in SyncManager tests
    // (resolve_item_by_prefix covers exact match, prefix match, not found,
    // ambiguous, deleted items, and completion status filtering)

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
}
