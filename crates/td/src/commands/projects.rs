//! Projects command implementation.
//!
//! Lists and manages projects via the Sync API.
//! Uses SyncManager::execute_commands() to automatically update the cache.

use chrono::Utc;
use todoist_api::client::TodoistClient;
use todoist_api::sync::{Project, SyncCommand};
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
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Resolve parent project name to ID if provided (extract owned data before mutation)
    let (parent_id, parent_name) = if let Some(ref parent_ref) = opts.parent {
        let cache = manager.cache();
        let parent_ref_lower = parent_ref.to_lowercase();
        let parent = cache
            .projects
            .iter()
            .find(|p| !p.is_deleted && (p.name.to_lowercase() == parent_ref_lower || p.id == *parent_ref))
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

    // Execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let response = manager.execute_commands(vec![command]).await?;

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

// ============================================================================
// Projects Show Command
// ============================================================================

/// Options for the projects show command.
#[derive(Debug)]
pub struct ProjectsShowOptions {
    /// Project ID (full ID or prefix).
    pub project_id: String,
    /// List sections in this project.
    pub sections: bool,
    /// List tasks in this project.
    pub tasks: bool,
}

/// Result data for the projects show command.
pub struct ProjectsShowResult<'a> {
    /// The project.
    pub project: &'a Project,
    /// Parent project name (if any).
    pub parent_name: Option<String>,
    /// Number of active tasks in this project.
    pub task_count: usize,
    /// Number of sections in this project.
    pub section_count: usize,
    /// Sections in this project (if requested).
    pub sections: Vec<&'a todoist_api::sync::Section>,
    /// Tasks in this project (if requested).
    pub tasks: Vec<&'a todoist_api::sync::Item>,
}

/// Executes the projects show command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Projects show command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails or if the project is not found.
pub async fn execute_show(ctx: &CommandContext, opts: &ProjectsShowOptions, token: &str) -> Result<()> {
    // Initialize sync manager
    let client = TodoistClient::new(token);
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

    // Find the project by ID or prefix
    let project = find_project_by_id_or_prefix(cache, &opts.project_id)?;

    // Get parent project name
    let parent_name = project.parent_id.as_ref().and_then(|pid| {
        cache
            .projects
            .iter()
            .find(|p| &p.id == pid)
            .map(|p| p.name.clone())
    });

    // Count active tasks in this project
    let task_count = cache
        .items
        .iter()
        .filter(|i| i.project_id == project.id && !i.is_deleted && !i.checked)
        .count();

    // Get sections for this project
    let all_sections: Vec<&todoist_api::sync::Section> = cache
        .sections
        .iter()
        .filter(|s| s.project_id == project.id && !s.is_deleted)
        .collect();
    let section_count = all_sections.len();

    // Only include sections if requested
    let sections = if opts.sections {
        all_sections
    } else {
        vec![]
    };

    // Get tasks for this project if requested
    let tasks: Vec<&todoist_api::sync::Item> = if opts.tasks {
        cache
            .items
            .iter()
            .filter(|i| i.project_id == project.id && !i.is_deleted && !i.checked)
            .collect()
    } else {
        vec![]
    };

    let result = ProjectsShowResult {
        project,
        parent_name,
        task_count,
        section_count,
        sections,
        tasks,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_project_details_json(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = crate::output::format_project_details_table(&result, ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

/// Finds a project by full ID or unique prefix.
fn find_project_by_id_or_prefix<'a>(cache: &'a Cache, id: &str) -> Result<&'a Project> {
    // First try exact match
    if let Some(project) = cache.projects.iter().find(|p| p.id == id && !p.is_deleted) {
        return Ok(project);
    }

    // Try prefix match
    let matches: Vec<&Project> = cache
        .projects
        .iter()
        .filter(|p| p.id.starts_with(id) && !p.is_deleted)
        .collect();

    match matches.len() {
        0 => Err(CommandError::Config(format!("Project not found: {id}"))),
        1 => Ok(matches[0]),
        _ => {
            // Ambiguous prefix - provide helpful error message
            let mut msg = format!("Ambiguous project ID \"{id}\"\n\nMultiple projects match this prefix:");
            for project in matches.iter().take(5) {
                let prefix = &project.id[..6.min(project.id.len())];
                msg.push_str(&format!("\n  {}  {}", prefix, project.name));
            }
            if matches.len() > 5 {
                msg.push_str(&format!("\n  ... and {} more", matches.len() - 5));
            }
            msg.push_str("\n\nPlease use a longer prefix.");
            Err(CommandError::Config(msg))
        }
    }
}

// ============================================================================
// Projects Edit Command
// ============================================================================

/// Options for the projects edit command.
#[derive(Debug)]
pub struct ProjectsEditOptions {
    /// Project ID (full ID or prefix).
    pub project_id: String,
    /// New project name.
    pub name: Option<String>,
    /// New project color.
    pub color: Option<String>,
    /// Set favorite status.
    pub favorite: Option<bool>,
    /// View style (list, board).
    pub view_style: Option<String>,
}

/// Result of a successful project edit operation.
#[derive(Debug)]
pub struct ProjectEditResult {
    /// The ID of the edited project.
    pub id: String,
    /// The name of the project (possibly updated).
    pub name: String,
    /// Fields that were updated.
    pub updated_fields: Vec<String>,
}

/// Executes the projects edit command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Projects edit command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails, project lookup fails, or the API returns an error.
pub async fn execute_edit(ctx: &CommandContext, opts: &ProjectsEditOptions, token: &str) -> Result<()> {
    // Check if any options were provided
    if opts.name.is_none() && opts.color.is_none() && opts.favorite.is_none() && opts.view_style.is_none() {
        return Err(CommandError::Config(
            "No changes specified. Use --name, --color, --favorite, or --view-style.".to_string()
        ));
    }

    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the project by ID or prefix and extract owned data before mutation
    let (project_id, project_name) = {
        let cache = manager.cache();
        let project = find_project_by_id_or_prefix(cache, &opts.project_id)?;
        (project.id.clone(), project.name.clone())
    };

    // Validate color if provided
    if let Some(ref color) = opts.color {
        if !is_valid_color(color) {
            return Err(CommandError::Config(format!(
                "Invalid color: {color}. Valid colors: berry_red, red, orange, yellow, olive_green, lime_green, green, mint_green, teal, sky_blue, light_blue, blue, grape, violet, lavender, magenta, salmon, charcoal, grey, taupe"
            )));
        }
    }

    // Validate view_style if provided
    if let Some(ref view_style) = opts.view_style {
        if view_style != "list" && view_style != "board" {
            return Err(CommandError::Config(format!(
                "Invalid view style: {view_style}. Valid options: list, board"
            )));
        }
    }

    // Build the project_update command arguments
    let mut args = serde_json::json!({
        "id": project_id,
    });

    let mut updated_fields = Vec::new();

    if let Some(ref name) = opts.name {
        args["name"] = serde_json::json!(name);
        updated_fields.push("name".to_string());
    }

    if let Some(ref color) = opts.color {
        args["color"] = serde_json::json!(color);
        updated_fields.push("color".to_string());
    }

    if let Some(favorite) = opts.favorite {
        args["is_favorite"] = serde_json::json!(favorite);
        updated_fields.push("favorite".to_string());
    }

    if let Some(ref view_style) = opts.view_style {
        args["view_style"] = serde_json::json!(view_style);
        updated_fields.push("view_style".to_string());
    }

    // Create the command
    let command = SyncCommand::new("project_update", args);

    // Execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let response = manager.execute_commands(vec![command]).await?;

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

    let result = ProjectEditResult {
        id: project_id,
        name: opts.name.clone().unwrap_or(project_name),
        updated_fields,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_edited_project(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Updated project: {} ({})", result.name, result.id);
            println!("  Changed fields: {}", result.updated_fields.join(", "));
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Updated: {} ({})", result.name, prefix);
        }
    }

    Ok(())
}

// ============================================================================
// Projects Archive Command
// ============================================================================

/// Options for the projects archive command.
#[derive(Debug)]
pub struct ProjectsArchiveOptions {
    /// Project ID (full ID or prefix).
    pub project_id: String,
    /// Skip confirmation.
    pub force: bool,
}

/// Result of a successful project archive operation.
#[derive(Debug)]
pub struct ProjectArchiveResult {
    /// The ID of the archived project.
    pub id: String,
    /// The name of the archived project.
    pub name: String,
}

/// Executes the projects archive command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Projects archive command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails, project lookup fails, or the API returns an error.
pub async fn execute_archive(ctx: &CommandContext, opts: &ProjectsArchiveOptions, token: &str) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the project by ID or prefix and extract owned data before mutation
    let (project_id, project_name, is_archived, is_inbox) = {
        let cache = manager.cache();
        let project = find_project_by_id_or_prefix(cache, &opts.project_id)?;
        (project.id.clone(), project.name.clone(), project.is_archived, project.inbox_project)
    };

    // Check if project is already archived
    if is_archived {
        return Err(CommandError::Config(format!(
            "Project '{}' is already archived",
            project_name
        )));
    }

    // Check if this is the inbox project
    if is_inbox {
        return Err(CommandError::Config(
            "Cannot archive the Inbox project".to_string()
        ));
    }

    // Confirm if not forced
    if !opts.force && !ctx.quiet {
        eprintln!(
            "Archive project '{}' ({})?",
            project_name,
            &project_id[..6.min(project_id.len())]
        );
        eprintln!("This will archive the project and all its descendants.");
        eprintln!("Use --force to skip this confirmation.");
        return Err(CommandError::Config("Operation cancelled. Use --force to confirm.".to_string()));
    }

    // Build the project_archive command arguments
    let args = serde_json::json!({
        "id": project_id,
    });

    // Create the command
    let command = SyncCommand::new("project_archive", args);

    // Execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let response = manager.execute_commands(vec![command]).await?;

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

    let result = ProjectArchiveResult {
        id: project_id,
        name: project_name,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_archived_project(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Archived project: {} ({})", result.name, result.id);
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Archived: {} ({})", result.name, prefix);
        }
    }

    Ok(())
}

// ============================================================================
// Projects Unarchive Command
// ============================================================================

/// Options for the projects unarchive command.
#[derive(Debug)]
pub struct ProjectsUnarchiveOptions {
    /// Project ID (full ID or prefix).
    pub project_id: String,
}

/// Result of a successful project unarchive operation.
#[derive(Debug)]
pub struct ProjectUnarchiveResult {
    /// The ID of the unarchived project.
    pub id: String,
    /// The name of the unarchived project.
    pub name: String,
}

/// Executes the projects unarchive command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Projects unarchive command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails, project lookup fails, or the API returns an error.
pub async fn execute_unarchive(ctx: &CommandContext, opts: &ProjectsUnarchiveOptions, token: &str) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the project by ID or prefix (include archived projects) and extract owned data
    let (project_id, project_name, is_archived) = {
        let cache = manager.cache();
        let project = find_project_by_id_or_prefix_include_archived(cache, &opts.project_id)?;
        (project.id.clone(), project.name.clone(), project.is_archived)
    };

    // Check if project is not archived
    if !is_archived {
        return Err(CommandError::Config(format!(
            "Project '{}' is not archived",
            project_name
        )));
    }

    // Build the project_unarchive command arguments
    let args = serde_json::json!({
        "id": project_id,
    });

    // Create the command
    let command = SyncCommand::new("project_unarchive", args);

    // Execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let response = manager.execute_commands(vec![command]).await?;

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

    let result = ProjectUnarchiveResult {
        id: project_id,
        name: project_name,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_unarchived_project(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Unarchived project: {} ({})", result.name, result.id);
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Unarchived: {} ({})", result.name, prefix);
        }
    }

    Ok(())
}

// ============================================================================
// Projects Delete Command
// ============================================================================

/// Options for the projects delete command.
#[derive(Debug)]
pub struct ProjectsDeleteOptions {
    /// Project ID (full ID or prefix).
    pub project_id: String,
    /// Skip confirmation.
    pub force: bool,
}

/// Result of a successful project delete operation.
#[derive(Debug)]
pub struct ProjectDeleteResult {
    /// The ID of the deleted project.
    pub id: String,
    /// The name of the deleted project.
    pub name: String,
}

/// Executes the projects delete command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Projects delete command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails, project lookup fails, or the API returns an error.
pub async fn execute_delete(ctx: &CommandContext, opts: &ProjectsDeleteOptions, token: &str) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the project by ID or prefix (include archived projects since they can be deleted)
    // Extract owned data before mutation
    let (project_id, project_name, is_inbox) = {
        let cache = manager.cache();
        let project = find_project_by_id_or_prefix_include_archived(cache, &opts.project_id)?;
        (project.id.clone(), project.name.clone(), project.inbox_project)
    };

    // Check if this is the inbox project
    if is_inbox {
        return Err(CommandError::Config(
            "Cannot delete the Inbox project".to_string()
        ));
    }

    // Confirm if not forced
    if !opts.force && !ctx.quiet {
        eprintln!(
            "Delete project '{}' ({})?",
            project_name,
            &project_id[..6.min(project_id.len())]
        );
        eprintln!("This will permanently delete the project and all its tasks.");
        eprintln!("Use --force to skip this confirmation.");
        return Err(CommandError::Config("Operation cancelled. Use --force to confirm.".to_string()));
    }

    // Build the project_delete command arguments
    let args = serde_json::json!({
        "id": project_id,
    });

    // Create the command
    let command = SyncCommand::new("project_delete", args);

    // Execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let response = manager.execute_commands(vec![command]).await?;

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

    let result = ProjectDeleteResult {
        id: project_id,
        name: project_name,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_deleted_project(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Deleted project: {} ({})", result.name, result.id);
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Deleted: {} ({})", result.name, prefix);
        }
    }

    Ok(())
}

/// Finds a project by full ID or unique prefix, including archived projects.
fn find_project_by_id_or_prefix_include_archived<'a>(cache: &'a Cache, id: &str) -> Result<&'a Project> {
    // First try exact match
    if let Some(project) = cache.projects.iter().find(|p| p.id == id && !p.is_deleted) {
        return Ok(project);
    }

    // Try prefix match
    let matches: Vec<&Project> = cache
        .projects
        .iter()
        .filter(|p| p.id.starts_with(id) && !p.is_deleted)
        .collect();

    match matches.len() {
        0 => Err(CommandError::Config(format!("Project not found: {id}"))),
        1 => Ok(matches[0]),
        _ => {
            // Ambiguous prefix - provide helpful error message
            let mut msg = format!("Ambiguous project ID \"{id}\"\n\nMultiple projects match this prefix:");
            for project in matches.iter().take(5) {
                let prefix = &project.id[..6.min(project.id.len())];
                let archived_indicator = if project.is_archived { " [archived]" } else { "" };
                msg.push_str(&format!("\n  {}  {}{}", prefix, project.name, archived_indicator));
            }
            if matches.len() > 5 {
                msg.push_str(&format!("\n  ... and {} more", matches.len() - 5));
            }
            msg.push_str("\n\nPlease use a longer prefix.");
            Err(CommandError::Config(msg))
        }
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

    #[test]
    fn test_projects_show_options() {
        let opts = ProjectsShowOptions {
            project_id: "abc123".to_string(),
            sections: false,
            tasks: false,
        };

        assert_eq!(opts.project_id, "abc123");
        assert!(!opts.sections);
        assert!(!opts.tasks);
    }

    #[test]
    fn test_projects_show_options_with_flags() {
        let opts = ProjectsShowOptions {
            project_id: "project-123-abc".to_string(),
            sections: true,
            tasks: true,
        };

        assert_eq!(opts.project_id, "project-123-abc");
        assert!(opts.sections);
        assert!(opts.tasks);
    }

    #[test]
    fn test_find_project_by_id_or_prefix_exact_match() {
        let cache = make_test_cache_with_projects();
        let result = find_project_by_id_or_prefix(&cache, "proj-123-abc");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "proj-123-abc");
    }

    #[test]
    fn test_find_project_by_id_or_prefix_unique_prefix() {
        let cache = make_test_cache_with_projects();
        let result = find_project_by_id_or_prefix(&cache, "proj-123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "proj-123-abc");
    }

    #[test]
    fn test_find_project_by_id_or_prefix_not_found() {
        let cache = make_test_cache_with_projects();
        let result = find_project_by_id_or_prefix(&cache, "nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Project not found"));
    }

    #[test]
    fn test_find_project_by_id_or_prefix_ambiguous() {
        let cache = make_cache_with_ambiguous_project_ids();
        let result = find_project_by_id_or_prefix(&cache, "proj-");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ambiguous"));
    }

    #[test]
    fn test_find_project_by_id_or_prefix_ignores_deleted() {
        let mut cache = make_test_cache_with_projects();
        // Mark the project as deleted
        cache.projects[0].is_deleted = true;

        let result = find_project_by_id_or_prefix(&cache, "proj-123");
        assert!(result.is_err());
    }

    // Helper function to create a test cache with projects
    fn make_test_cache_with_projects() -> Cache {
        Cache {
            sync_token: "test".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: vec![],
            projects: vec![make_test_project("proj-123-abc", "Test Project")],
            labels: vec![],
            sections: vec![],
            notes: vec![],
            project_notes: vec![],
            reminders: vec![],
            filters: vec![],
            user: None,
        }
    }

    fn make_cache_with_ambiguous_project_ids() -> Cache {
        Cache {
            sync_token: "test".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: vec![],
            projects: vec![
                make_test_project("proj-aaa-111", "Project 1"),
                make_test_project("proj-aaa-222", "Project 2"),
                make_test_project("proj-bbb-333", "Project 3"),
            ],
            labels: vec![],
            sections: vec![],
            notes: vec![],
            project_notes: vec![],
            reminders: vec![],
            filters: vec![],
            user: None,
        }
    }

    fn make_test_project(id: &str, name: &str) -> Project {
        Project {
            id: id.to_string(),
            name: name.to_string(),
            color: None,
            parent_id: None,
            child_order: 0,
            is_collapsed: false,
            is_favorite: false,
            is_deleted: false,
            is_archived: false,
            inbox_project: false,
            view_style: None,
            shared: false,
            can_assign_tasks: false,
            folder_id: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_projects_edit_options() {
        let opts = ProjectsEditOptions {
            project_id: "proj-123".to_string(),
            name: Some("New Name".to_string()),
            color: Some("blue".to_string()),
            favorite: Some(true),
            view_style: Some("board".to_string()),
        };

        assert_eq!(opts.project_id, "proj-123");
        assert_eq!(opts.name, Some("New Name".to_string()));
        assert_eq!(opts.color, Some("blue".to_string()));
        assert_eq!(opts.favorite, Some(true));
        assert_eq!(opts.view_style, Some("board".to_string()));
    }

    #[test]
    fn test_projects_edit_options_minimal() {
        let opts = ProjectsEditOptions {
            project_id: "proj-456".to_string(),
            name: None,
            color: None,
            favorite: None,
            view_style: Some("list".to_string()),
        };

        assert_eq!(opts.project_id, "proj-456");
        assert!(opts.name.is_none());
        assert!(opts.color.is_none());
        assert!(opts.favorite.is_none());
        assert_eq!(opts.view_style, Some("list".to_string()));
    }

    #[test]
    fn test_project_edit_result() {
        let result = ProjectEditResult {
            id: "proj-123".to_string(),
            name: "Updated Project".to_string(),
            updated_fields: vec!["name".to_string(), "color".to_string()],
        };

        assert_eq!(result.id, "proj-123");
        assert_eq!(result.name, "Updated Project");
        assert_eq!(result.updated_fields, vec!["name", "color"]);
    }

    #[test]
    fn test_projects_archive_options() {
        let opts = ProjectsArchiveOptions {
            project_id: "proj-123".to_string(),
            force: false,
        };

        assert_eq!(opts.project_id, "proj-123");
        assert!(!opts.force);
    }

    #[test]
    fn test_projects_archive_options_with_force() {
        let opts = ProjectsArchiveOptions {
            project_id: "proj-456".to_string(),
            force: true,
        };

        assert_eq!(opts.project_id, "proj-456");
        assert!(opts.force);
    }

    #[test]
    fn test_project_archive_result() {
        let result = ProjectArchiveResult {
            id: "proj-789".to_string(),
            name: "Archived Project".to_string(),
        };

        assert_eq!(result.id, "proj-789");
        assert_eq!(result.name, "Archived Project");
    }

    #[test]
    fn test_projects_unarchive_options() {
        let opts = ProjectsUnarchiveOptions {
            project_id: "proj-123".to_string(),
        };

        assert_eq!(opts.project_id, "proj-123");
    }

    #[test]
    fn test_project_unarchive_result() {
        let result = ProjectUnarchiveResult {
            id: "proj-456".to_string(),
            name: "Unarchived Project".to_string(),
        };

        assert_eq!(result.id, "proj-456");
        assert_eq!(result.name, "Unarchived Project");
    }

    #[test]
    fn test_projects_delete_options() {
        let opts = ProjectsDeleteOptions {
            project_id: "proj-123".to_string(),
            force: false,
        };

        assert_eq!(opts.project_id, "proj-123");
        assert!(!opts.force);
    }

    #[test]
    fn test_projects_delete_options_with_force() {
        let opts = ProjectsDeleteOptions {
            project_id: "proj-456".to_string(),
            force: true,
        };

        assert_eq!(opts.project_id, "proj-456");
        assert!(opts.force);
    }

    #[test]
    fn test_project_delete_result() {
        let result = ProjectDeleteResult {
            id: "proj-789".to_string(),
            name: "Deleted Project".to_string(),
        };

        assert_eq!(result.id, "proj-789");
        assert_eq!(result.name, "Deleted Project");
    }
}
