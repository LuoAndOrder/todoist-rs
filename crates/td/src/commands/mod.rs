//! Command implementations for the td CLI.
//!
//! This module contains the actual command handlers that are invoked by the CLI.

pub mod add;
pub mod comments;
pub mod completions;
pub mod config;
pub mod delete;
pub mod done;
pub mod edit;
pub mod filters;
pub mod keyring;
pub mod labels;
pub mod list;
pub mod projects;
pub mod quick;
pub mod reminders;
pub mod reopen;
pub mod sections;
pub mod setup;
pub mod show;
pub mod sync;
pub mod today;

use std::io::IsTerminal;

use crate::cli::Cli;

/// Confirmation result for bulk operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmResult {
    /// User confirmed the operation.
    Confirmed,
    /// Operation was aborted by user.
    Aborted,
}

/// Prompts for confirmation on bulk destructive operations.
///
/// Returns `Ok(ConfirmResult::Confirmed)` if:
/// - Only 1 item (no confirmation needed)
/// - `force` is true (skip confirmation)
/// - `quiet` is true (skip confirmation)
/// - User confirms with y/Y
/// - stdin is not a TTY (proceeds with warning)
///
/// Returns `Ok(ConfirmResult::Aborted)` if user declines.
///
/// # Arguments
///
/// * `action` - The action being performed (e.g., "delete", "complete", "reopen")
/// * `items` - List of (id_prefix, content) tuples to display
/// * `force` - If true, skip confirmation
/// * `quiet` - If true, skip confirmation
pub fn confirm_bulk_operation(
    action: &str,
    items: &[(&str, &str)],
    force: bool,
    quiet: bool,
) -> Result<ConfirmResult> {
    // No confirmation needed for single item
    if items.len() <= 1 {
        return Ok(ConfirmResult::Confirmed);
    }

    // Skip if forced or quiet
    if force || quiet {
        return Ok(ConfirmResult::Confirmed);
    }

    // Check if stdin is a TTY
    let stdin_is_tty = std::io::stdin().is_terminal();

    if !stdin_is_tty {
        // Not a TTY - proceed with warning to stderr
        eprintln!(
            "Warning: About to {} {} tasks (non-interactive mode, proceeding automatically)",
            action,
            items.len()
        );
        return Ok(ConfirmResult::Confirmed);
    }

    // Display items to be affected
    eprintln!("About to {} {} tasks:", action, items.len());
    for (id_prefix, content) in items {
        eprintln!("  {}  {}", id_prefix, content);
    }
    eprintln!();

    // Use dialoguer for interactive confirmation
    let confirmed = dialoguer::Confirm::new()
        .with_prompt(format!("Continue with {} {} tasks?", action, items.len()))
        .default(false)
        .interact()
        .map_err(|e| CommandError::Io(std::io::Error::other(format!("Failed to read input: {}", e))))?;

    if confirmed {
        Ok(ConfirmResult::Confirmed)
    } else {
        Ok(ConfirmResult::Aborted)
    }
}

/// Error type for command execution.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// Cache/sync error.
    #[error("sync error: {0}")]
    Sync(#[from] todoist_cache_rs::SyncError),

    /// Cache store error.
    #[error("cache error: {0}")]
    CacheStore(#[from] todoist_cache_rs::CacheStoreError),

    /// Filter parsing error.
    #[error("filter error: {0}")]
    Filter(#[from] todoist_cache_rs::filter::FilterError),

    /// API error.
    #[error("API error: {0}")]
    Api(#[from] todoist_api_rs::error::Error),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result type for command execution.
pub type Result<T> = std::result::Result<T, CommandError>;

/// Context for command execution, containing common dependencies.
pub struct CommandContext {
    /// Whether to output JSON.
    pub json_output: bool,
    /// Whether to use colors.
    pub use_colors: bool,
    /// Whether to be quiet (errors only).
    pub quiet: bool,
    /// Whether to be verbose.
    pub verbose: bool,
    /// Whether to sync before executing the command.
    /// Used by read commands with the --sync flag.
    pub sync_first: bool,
}

impl CommandContext {
    /// Creates a new command context from CLI arguments.
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            json_output: cli.json,
            use_colors: !cli.no_color,
            quiet: cli.quiet,
            verbose: cli.verbose,
            sync_first: cli.sync,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_bulk_single_item_no_confirmation() {
        let items = vec![("abc123", "Task 1")];
        let result = confirm_bulk_operation("delete", &items, false, false).unwrap();
        assert_eq!(result, ConfirmResult::Confirmed);
    }

    #[test]
    fn test_confirm_bulk_empty_items_no_confirmation() {
        let items: Vec<(&str, &str)> = vec![];
        let result = confirm_bulk_operation("delete", &items, false, false).unwrap();
        assert_eq!(result, ConfirmResult::Confirmed);
    }

    #[test]
    fn test_confirm_bulk_force_skips_confirmation() {
        let items = vec![("abc123", "Task 1"), ("def456", "Task 2")];
        let result = confirm_bulk_operation("delete", &items, true, false).unwrap();
        assert_eq!(result, ConfirmResult::Confirmed);
    }

    #[test]
    fn test_confirm_bulk_quiet_skips_confirmation() {
        let items = vec![("abc123", "Task 1"), ("def456", "Task 2")];
        let result = confirm_bulk_operation("delete", &items, false, true).unwrap();
        assert_eq!(result, ConfirmResult::Confirmed);
    }

    #[test]
    fn test_confirm_bulk_force_and_quiet_skips_confirmation() {
        let items = vec![("abc123", "Task 1"), ("def456", "Task 2"), ("ghi789", "Task 3")];
        let result = confirm_bulk_operation("complete", &items, true, true).unwrap();
        assert_eq!(result, ConfirmResult::Confirmed);
    }

    // Note: Testing the interactive prompt path would require mocking stdin,
    // which is complex. The non-TTY path auto-confirms, so in test contexts
    // (where stdin is typically not a TTY), it will proceed with a warning.
    #[test]
    fn test_confirm_bulk_non_tty_proceeds() {
        // In tests, stdin is typically not a TTY, so this should auto-confirm
        let items = vec![("abc123", "Task 1"), ("def456", "Task 2")];
        let result = confirm_bulk_operation("reopen", &items, false, false).unwrap();
        // Non-TTY stdin should auto-confirm
        assert_eq!(result, ConfirmResult::Confirmed);
    }
}
