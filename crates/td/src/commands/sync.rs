//! Sync command implementation.
//!
//! Force sync local cache with Todoist. Supports full sync with --full flag.

use chrono::Utc;
use todoist_cache::{CacheStore, SyncManager};

use super::{CommandContext, Result};

/// Options for the sync command.
#[derive(Debug)]
pub struct SyncOptions {
    /// Force full sync (ignore cache).
    pub full: bool,
}

/// Summary of a sync operation.
pub struct SyncSummary {
    /// Whether this was a full sync.
    pub full_sync: bool,
    /// Number of tasks in cache after sync.
    pub tasks: usize,
    /// Number of projects in cache after sync.
    pub projects: usize,
    /// Number of labels in cache after sync.
    pub labels: usize,
    /// Number of sections in cache after sync.
    pub sections: usize,
    /// Number of comments in cache after sync.
    pub comments: usize,
    /// Number of reminders in cache after sync.
    pub reminders: usize,
    /// Number of filters in cache after sync.
    pub filters: usize,
}

/// Executes the sync command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Sync command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails.
pub async fn execute(ctx: &CommandContext, opts: &SyncOptions, token: &str) -> Result<()> {
    // Initialize sync manager
    let client = todoist_api::client::TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Show what we're doing
    if ctx.verbose {
        if opts.full {
            eprintln!("Performing full sync...");
        } else {
            eprintln!("Performing incremental sync...");
        }
    }

    // Perform sync
    let cache = if opts.full {
        manager.full_sync().await?
    } else {
        manager.sync().await?
    };

    // Build summary
    let summary = SyncSummary {
        full_sync: opts.full || cache.full_sync_date_utc.is_some_and(|d| {
            // Check if this sync updated the full_sync_date to "now"
            let now = Utc::now();
            (now - d).num_seconds().abs() < 5
        }),
        tasks: cache.items.iter().filter(|i| !i.is_deleted && !i.checked).count(),
        projects: cache.projects.iter().filter(|p| !p.is_deleted).count(),
        labels: cache.labels.iter().filter(|l| !l.is_deleted).count(),
        sections: cache.sections.iter().filter(|s| !s.is_deleted).count(),
        comments: cache.notes.iter().filter(|n| !n.is_deleted).count()
            + cache.project_notes.iter().filter(|n| !n.is_deleted).count(),
        reminders: cache.reminders.iter().filter(|r| !r.is_deleted).count(),
        filters: cache.filters.iter().filter(|f| !f.is_deleted).count(),
    };

    // Output
    if ctx.json_output {
        let output = format_sync_json(&summary)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_sync_table(&summary, ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

/// Formats the sync summary as JSON.
fn format_sync_json(summary: &SyncSummary) -> std::result::Result<String, serde_json::Error> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct SyncOutput {
        status: &'static str,
        sync_type: &'static str,
        summary: SummaryOutput,
    }

    #[derive(Serialize)]
    struct SummaryOutput {
        tasks: usize,
        projects: usize,
        labels: usize,
        sections: usize,
        comments: usize,
        reminders: usize,
        filters: usize,
    }

    let output = SyncOutput {
        status: "success",
        sync_type: if summary.full_sync { "full" } else { "incremental" },
        summary: SummaryOutput {
            tasks: summary.tasks,
            projects: summary.projects,
            labels: summary.labels,
            sections: summary.sections,
            comments: summary.comments,
            reminders: summary.reminders,
            filters: summary.filters,
        },
    };

    serde_json::to_string_pretty(&output)
}

/// Formats the sync summary as a human-readable table.
fn format_sync_table(summary: &SyncSummary, use_colors: bool) -> String {
    use owo_colors::OwoColorize;

    let mut output = String::new();

    // Header
    let sync_type = if summary.full_sync { "Full" } else { "Incremental" };
    let header = format!("{} sync completed", sync_type);
    if use_colors {
        output.push_str(&format!("{}\n\n", header.green().bold()));
    } else {
        output.push_str(&format!("{}\n\n", header));
    }

    // Summary
    output.push_str("Cache summary:\n");
    output.push_str(&format!("  Tasks:     {}\n", summary.tasks));
    output.push_str(&format!("  Projects:  {}\n", summary.projects));
    output.push_str(&format!("  Labels:    {}\n", summary.labels));
    output.push_str(&format!("  Sections:  {}\n", summary.sections));
    output.push_str(&format!("  Comments:  {}\n", summary.comments));
    output.push_str(&format!("  Reminders: {}\n", summary.reminders));
    output.push_str(&format!("  Filters:   {}\n", summary.filters));

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_options_defaults() {
        let opts = SyncOptions { full: false };
        assert!(!opts.full);
    }

    #[test]
    fn test_sync_options_full() {
        let opts = SyncOptions { full: true };
        assert!(opts.full);
    }

    #[test]
    fn test_format_sync_json_incremental() {
        let summary = SyncSummary {
            full_sync: false,
            tasks: 10,
            projects: 3,
            labels: 5,
            sections: 2,
            comments: 1,
            reminders: 0,
            filters: 2,
        };

        let json = format_sync_json(&summary).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["status"], "success");
        assert_eq!(parsed["sync_type"], "incremental");
        assert_eq!(parsed["summary"]["tasks"], 10);
        assert_eq!(parsed["summary"]["projects"], 3);
    }

    #[test]
    fn test_format_sync_json_full() {
        let summary = SyncSummary {
            full_sync: true,
            tasks: 25,
            projects: 5,
            labels: 8,
            sections: 4,
            comments: 3,
            reminders: 2,
            filters: 1,
        };

        let json = format_sync_json(&summary).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["status"], "success");
        assert_eq!(parsed["sync_type"], "full");
        assert_eq!(parsed["summary"]["tasks"], 25);
    }

    #[test]
    fn test_format_sync_table_incremental() {
        let summary = SyncSummary {
            full_sync: false,
            tasks: 10,
            projects: 3,
            labels: 5,
            sections: 2,
            comments: 1,
            reminders: 0,
            filters: 2,
        };

        let output = format_sync_table(&summary, false);
        assert!(output.contains("Incremental sync completed"));
        assert!(output.contains("Tasks:     10"));
        assert!(output.contains("Projects:  3"));
    }

    #[test]
    fn test_format_sync_table_full() {
        let summary = SyncSummary {
            full_sync: true,
            tasks: 25,
            projects: 5,
            labels: 8,
            sections: 4,
            comments: 3,
            reminders: 2,
            filters: 1,
        };

        let output = format_sync_table(&summary, false);
        assert!(output.contains("Full sync completed"));
        assert!(output.contains("Tasks:     25"));
    }
}
