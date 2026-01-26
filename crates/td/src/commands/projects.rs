//! Projects command implementation.
//!
//! Lists and manages projects via the Sync API.

use chrono::Utc;
use todoist_api::client::TodoistClient;
use todoist_api::sync::{Project, SyncCommand, SyncRequest};
use todoist_cache::{Cache, CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};
use crate::output::{format_created_project, format_projects_json, format_projects_table};

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

// ============================================================================
// Projects Add Command
// ============================================================================

/// Options for the projects add command.
#[derive(Debug)]
pub struct ProjectsAddOptions {
    /// Project name.
    pub name: String,
    /// Project color.
    pub color: Option<String>,
    /// Parent project (name or ID).
    pub parent: Option<String>,
    /// Mark as favorite.
    pub favorite: bool,
}

/// Result of a successful project add operation.
#[derive(Debug)]
pub struct ProjectAddResult {
    /// The real ID of the created project.
    pub id: String,
    /// The name of the created project.
    pub name: String,
    /// The color of the created project.
    pub color: Option<String>,
    /// The parent project ID (if any).
    pub parent_id: Option<String>,
    /// The parent project name (if found in cache).
    pub parent_name: Option<String>,
    /// Whether the project is a favorite.
    pub is_favorite: bool,
}

/// Executes the projects add command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Projects add command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails, parent project lookup fails, or the API returns an error.
pub async fn execute_add(ctx: &CommandContext, opts: &ProjectsAddOptions, token: &str) -> Result<()> {
    // Initialize sync manager to resolve parent project name to ID
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Sync to get current state (for parent project lookup)
    manager.sync().await?;
    let cache = manager.cache();

    // Resolve parent project name to ID if provided
    let (parent_id, parent_name) = if let Some(ref parent_ref) = opts.parent {
        let parent_ref_lower = parent_ref.to_lowercase();
        let parent = cache
            .projects
            .iter()
            .find(|p| p.name.to_lowercase() == parent_ref_lower || p.id == *parent_ref)
            .ok_or_else(|| CommandError::Config(format!("Parent project not found: {parent_ref}")))?;
        (Some(parent.id.clone()), Some(parent.name.clone()))
    } else {
        (None, None)
    };

    // Validate color if provided
    if let Some(ref color) = opts.color {
        if !is_valid_color(color) {
            return Err(CommandError::Config(format!(
                "Invalid color: {color}. Valid colors: berry_red, red, orange, yellow, olive_green, lime_green, green, mint_green, teal, sky_blue, light_blue, blue, grape, violet, lavender, magenta, salmon, charcoal, grey, taupe"
            )));
        }
    }

    // Build the project_add command arguments
    let temp_id = uuid::Uuid::new_v4().to_string();
    let mut args = serde_json::json!({
        "name": opts.name,
    });

    // Add optional fields
    if let Some(ref color) = opts.color {
        args["color"] = serde_json::json!(color);
    }

    if let Some(ref pid) = parent_id {
        args["parent_id"] = serde_json::json!(pid);
    }

    if opts.favorite {
        args["is_favorite"] = serde_json::json!(true);
    }

    // Create the command
    let command = SyncCommand::with_temp_id("project_add", &temp_id, args);

    // Execute the command
    let api_client = TodoistClient::new(token);
    let request = SyncRequest::with_commands(vec![command]);
    let response = api_client.sync(request).await?;

    // Check for errors
    if response.has_errors() {
        let errors = response.errors();
        if let Some((_, error)) = errors.first() {
            return Err(CommandError::Api(todoist_api::error::Error::Api(
                todoist_api::error::ApiError::Validation {
                    field: None,
                    message: format!("Error {}: {}", error.error_code, error.error),
                },
            )));
        }
    }

    // Get the real ID from the temp_id_mapping
    let real_id = response
        .real_id(&temp_id)
        .ok_or_else(|| {
            CommandError::Config("Project created but no ID returned in response".to_string())
        })?
        .clone();

    let result = ProjectAddResult {
        id: real_id,
        name: opts.name.clone(),
        color: opts.color.clone(),
        parent_id,
        parent_name,
        is_favorite: opts.favorite,
    };

    // Output
    if ctx.json_output {
        let output = format_created_project(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Created project: {} ({})", result.name, result.id);
            if let Some(ref color) = result.color {
                println!("  Color: {color}");
            }
            if let Some(ref parent_name) = result.parent_name {
                println!("  Parent: {parent_name}");
            }
            if result.is_favorite {
                println!("  Favorite: yes");
            }
        } else {
            println!("Created: {} ({})", result.name, &result.id[..6.min(result.id.len())]);
        }
    }

    Ok(())
}

/// Valid Todoist color names.
const VALID_COLORS: &[&str] = &[
    "berry_red", "red", "orange", "yellow", "olive_green", "lime_green",
    "green", "mint_green", "teal", "sky_blue", "light_blue", "blue",
    "grape", "violet", "lavender", "magenta", "salmon", "charcoal",
    "grey", "taupe",
];

/// Checks if a color name is valid.
fn is_valid_color(color: &str) -> bool {
    VALID_COLORS.contains(&color)
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

    #[test]
    fn test_projects_add_options() {
        let opts = ProjectsAddOptions {
            name: "Test Project".to_string(),
            color: Some("blue".to_string()),
            parent: Some("Parent".to_string()),
            favorite: true,
        };

        assert_eq!(opts.name, "Test Project");
        assert_eq!(opts.color, Some("blue".to_string()));
        assert_eq!(opts.parent, Some("Parent".to_string()));
        assert!(opts.favorite);
    }

    #[test]
    fn test_is_valid_color() {
        assert!(is_valid_color("blue"));
        assert!(is_valid_color("berry_red"));
        assert!(is_valid_color("lime_green"));
        assert!(is_valid_color("charcoal"));
        assert!(!is_valid_color("invalid"));
        assert!(!is_valid_color(""));
        assert!(!is_valid_color("Blue")); // Case-sensitive
    }
}
