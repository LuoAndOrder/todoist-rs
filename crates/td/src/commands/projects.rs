//! Projects command implementation.
//!
//! Lists projects from the local cache with options for hierarchy display,
//! archived projects, and output limiting.

use chrono::Utc;
use todoist_api::sync::Project;
use todoist_cache::{Cache, CacheStore, SyncManager};

use super::{CommandContext, Result};
use crate::output::{format_projects_json, format_projects_table};

/// Options for the projects list command.
#[derive(Debug, Default)]
pub struct ProjectsListOptions {
    /// Show as tree hierarchy.
    pub tree: bool,
    /// Include archived projects.
    pub archived: bool,
    /// Limit results.
    pub limit: Option<u32>,
}

/// Executes the projects list command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Projects list command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails.
pub async fn execute(ctx: &CommandContext, opts: &ProjectsListOptions, token: &str) -> Result<()> {
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

    // Get projects and apply filters
    let projects = filter_projects(cache, opts);

    // Apply limit
    let projects = apply_limit(projects, opts);

    // Output
    if ctx.json_output {
        let output = format_projects_json(&projects)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_projects_table(&projects, cache, ctx.use_colors, opts.tree);
        print!("{output}");
    }

    Ok(())
}

/// Filters projects based on the provided options.
fn filter_projects<'a>(cache: &'a Cache, opts: &ProjectsListOptions) -> Vec<&'a Project> {
    let mut projects: Vec<&Project> = cache
        .projects
        .iter()
        .filter(|p| {
            // Always exclude deleted projects
            if p.is_deleted {
                return false;
            }
            // Include archived only if requested
            if !opts.archived && p.is_archived {
                return false;
            }
            true
        })
        .collect();

    // Sort by child_order for consistent display
    projects.sort_by_key(|p| p.child_order);

    projects
}

/// Applies the limit to the projects.
fn apply_limit<'a>(projects: Vec<&'a Project>, opts: &ProjectsListOptions) -> Vec<&'a Project> {
    if let Some(limit) = opts.limit {
        projects.into_iter().take(limit as usize).collect()
    } else {
        projects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projects_list_options_defaults() {
        let opts = ProjectsListOptions::default();

        assert!(!opts.tree);
        assert!(!opts.archived);
        assert!(opts.limit.is_none());
    }

    #[test]
    fn test_projects_list_options_with_values() {
        let opts = ProjectsListOptions {
            tree: true,
            archived: true,
            limit: Some(10),
        };

        assert!(opts.tree);
        assert!(opts.archived);
        assert_eq!(opts.limit, Some(10));
    }
}
