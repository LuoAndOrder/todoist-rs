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
pub mod labels;
pub mod list;
pub mod projects;
pub mod quick;
pub mod reminders;
pub mod reopen;
pub mod sections;
pub mod show;
pub mod sync;
pub mod today;

use crate::cli::Cli;

/// Error type for command execution.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// Cache/sync error.
    #[error("sync error: {0}")]
    Sync(#[from] todoist_cache::SyncError),

    /// Cache store error.
    #[error("cache error: {0}")]
    CacheStore(#[from] todoist_cache::CacheStoreError),

    /// Filter parsing error.
    #[error("filter error: {0}")]
    Filter(#[from] todoist_cache::filter::FilterError),

    /// API error.
    #[error("API error: {0}")]
    Api(#[from] todoist_api::error::Error),

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
}

impl CommandContext {
    /// Creates a new command context from CLI arguments.
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            json_output: cli.json,
            use_colors: !cli.no_color,
            quiet: cli.quiet,
            verbose: cli.verbose,
        }
    }
}
