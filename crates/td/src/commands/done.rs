//! Done command implementation.
//!
//! Completes task(s) via the Sync API's `item_close` command.
//! Uses SyncManager::execute_commands() to automatically update the cache.
//! Uses resolve_item_by_prefix() for smart lookups with auto-sync fallback.

use todoist_api::client::TodoistClient;
use todoist_api::sync::SyncCommand;
use todoist_cache::{CacheStore, SyncManager};

use super::{confirm_bulk_operation, CommandContext, CommandError, ConfirmResult, Result};

/// Options for the done command.
#[derive(Debug)]
pub struct DoneOptions {
    /// Task IDs (full IDs or prefixes).
    pub task_ids: Vec<String>,
    /// Complete all future occurrences (for recurring tasks).
    /// When false (default), uses `item_close` which schedules recurring tasks to next occurrence.
    /// When true, uses `item_complete` which fully completes the task including all future occurrences.
    pub all_occurrences: bool,
    /// Skip confirmation for multiple tasks.
    pub force: bool,
}

/// Result of completing a single task.
#[derive(Debug)]
pub struct DoneResult {
    /// The task ID.
    pub id: String,
    /// The task content.
    pub content: String,
    /// Whether the operation succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Executes the done command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Done command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails, task lookup fails, or the API returns an error.
pub async fn execute(ctx: &CommandContext, opts: &DoneOptions, token: &str) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Resolve all task IDs using smart lookup (cache-first with auto-sync fallback)
    // require_checked=Some(false) to only find uncompleted tasks
    let mut resolved_items: Vec<(String, String)> = Vec::new();
    for task_id in &opts.task_ids {
        let item = manager
            .resolve_item_by_prefix(task_id, Some(false))
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

    match confirm_bulk_operation("complete", &items_for_confirm, opts.force, ctx.quiet)? {
        ConfirmResult::Confirmed => {}
        ConfirmResult::Aborted => {
            if !ctx.quiet {
                eprintln!("Aborted.");
            }
            return Ok(());
        }
    }

    // Build commands for all tasks
    // Use item_close by default (schedules recurring tasks to next occurrence)
    // Use item_complete when --all-occurrences is set (fully completes including recurring)
    let command_type = if opts.all_occurrences {
        "item_complete"
    } else {
        "item_close"
    };

    let commands: Vec<SyncCommand> = resolved_items
        .iter()
        .map(|(id, _)| {
            SyncCommand::new(
                command_type,
                serde_json::json!({ "id": id }),
            )
        })
        .collect();

    // Execute the commands via SyncManager
    // This sends the commands, applies the response to cache, and saves to disk
    let response = manager.execute_commands(commands).await?;

    // Process results
    let mut results: Vec<DoneResult> = Vec::new();
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

            results.push(DoneResult {
                id: id.clone(),
                content: content.clone(),
                success: false,
                error: error_msg,
            });
            error_count += 1;
        } else {
            results.push(DoneResult {
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
        let output = format_done_results_json(&results)?;
        println!("{output}");
    } else if !ctx.quiet {
        for result in &results {
            let id_prefix = &result.id[..6.min(result.id.len())];
            if result.success {
                println!("Completed: {} ({})", result.content, id_prefix);
            } else if let Some(ref err) = result.error {
                eprintln!("Failed to complete {} ({}): {}", result.content, id_prefix, err);
            }
        }

        if ctx.verbose && results.len() > 1 {
            println!("\n{} completed, {} failed", success_count, error_count);
        }
    }

    // Return error if any tasks failed
    if error_count > 0 && success_count == 0 {
        return Err(CommandError::Config(format!(
            "Failed to complete {} task(s)",
            error_count
        )));
    }

    Ok(())
}


/// Formats done results as JSON.
fn format_done_results_json(results: &[DoneResult]) -> Result<String> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct DoneOutput<'a> {
        completed: Vec<CompletedTaskOutput<'a>>,
        failed: Vec<FailedTaskOutput<'a>>,
        total_completed: usize,
        total_failed: usize,
    }

    #[derive(Serialize)]
    struct CompletedTaskOutput<'a> {
        id: &'a str,
        content: &'a str,
    }

    #[derive(Serialize)]
    struct FailedTaskOutput<'a> {
        id: &'a str,
        content: &'a str,
        error: Option<&'a str>,
    }

    let completed: Vec<CompletedTaskOutput> = results
        .iter()
        .filter(|r| r.success)
        .map(|r| CompletedTaskOutput {
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

    let output = DoneOutput {
        total_completed: completed.len(),
        total_failed: failed.len(),
        completed,
        failed,
    };

    serde_json::to_string_pretty(&output).map_err(CommandError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_done_options_single_task() {
        let opts = DoneOptions {
            task_ids: vec!["abc123".to_string()],
            all_occurrences: false,
            force: false,
        };

        assert_eq!(opts.task_ids.len(), 1);
        assert!(!opts.all_occurrences);
        assert!(!opts.force);
    }

    #[test]
    fn test_done_options_multiple_tasks() {
        let opts = DoneOptions {
            task_ids: vec![
                "abc123".to_string(),
                "def456".to_string(),
                "ghi789".to_string(),
            ],
            all_occurrences: false,
            force: true,
        };

        assert_eq!(opts.task_ids.len(), 3);
        assert!(opts.force);
    }

    #[test]
    fn test_done_result_success() {
        let result = DoneResult {
            id: "abc123".to_string(),
            content: "Test task".to_string(),
            success: true,
            error: None,
        };

        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_done_result_failure() {
        let result = DoneResult {
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
    fn test_format_done_results_json() {
        let results = vec![
            DoneResult {
                id: "abc123".to_string(),
                content: "Task 1".to_string(),
                success: true,
                error: None,
            },
            DoneResult {
                id: "def456".to_string(),
                content: "Task 2".to_string(),
                success: false,
                error: Some("Not found".to_string()),
            },
        ];

        let json = format_done_results_json(&results).unwrap();
        assert!(json.contains("\"total_completed\": 1"));
        assert!(json.contains("\"total_failed\": 1"));
        assert!(json.contains("Task 1"));
        assert!(json.contains("Task 2"));
        assert!(json.contains("Not found"));
    }
}
