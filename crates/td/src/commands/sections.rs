//! Sections command implementation.
//!
//! Lists and manages sections via the Sync API.
//! Uses SyncManager::execute_commands() to automatically update the cache.

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{Section, SyncCommand, SyncCommandType};
use todoist_cache_rs::{Cache, CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};
use crate::output::{format_sections_json, format_sections_table};

/// Options for the sections list command.
#[derive(Debug, Default)]
pub struct SectionsListOptions {
    /// Filter by project.
    pub project: Option<String>,
    /// Limit results.
    pub limit: Option<u32>,
}

/// Executes the sections list command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Sections list command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails.
pub async fn execute(ctx: &CommandContext, opts: &SectionsListOptions, token: &str) -> Result<()> {
    // Initialize sync manager
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Only sync if explicitly requested with --sync flag
    if ctx.sync_first {
        if ctx.verbose {
            eprintln!("Syncing with Todoist...");
        }
        manager.sync().await?;
    }

    let cache = manager.cache();

    // Resolve project filter if provided
    let project_id = if let Some(ref project) = opts.project {
        Some(resolve_project_id(cache, project)?)
    } else {
        None
    };

    // Get sections and apply filters
    let sections = filter_sections(cache, project_id.as_deref());

    // Apply limit
    let sections = apply_limit(sections, opts);

    // Output
    if ctx.json_output {
        let output = format_sections_json(&sections, cache)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_sections_table(&sections, cache, ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

/// Resolves a project name or ID to a project ID.
fn resolve_project_id(cache: &Cache, project: &str) -> Result<String> {
    // First try exact ID match
    if let Some(p) = cache
        .projects
        .iter()
        .find(|p| p.id == project && !p.is_deleted)
    {
        return Ok(p.id.clone());
    }

    // Try ID prefix match
    let prefix_matches: Vec<_> = cache
        .projects
        .iter()
        .filter(|p| p.id.starts_with(project) && !p.is_deleted)
        .collect();

    if prefix_matches.len() == 1 {
        return Ok(prefix_matches[0].id.clone());
    }

    // Try name match (case-insensitive)
    let name_matches: Vec<_> = cache
        .projects
        .iter()
        .filter(|p| p.name.eq_ignore_ascii_case(project) && !p.is_deleted)
        .collect();

    if name_matches.len() == 1 {
        return Ok(name_matches[0].id.clone());
    }

    if name_matches.len() > 1 || prefix_matches.len() > 1 {
        Err(CommandError::Config(format!(
            "Ambiguous project: '{project}'. Multiple projects match."
        )))
    } else {
        Err(CommandError::Config(format!(
            "Project not found: {project}"
        )))
    }
}

/// Filters sections (excludes deleted, optionally by project).
fn filter_sections<'a>(cache: &'a Cache, project_id: Option<&str>) -> Vec<&'a Section> {
    let mut sections: Vec<&Section> = cache
        .sections
        .iter()
        .filter(|s| !s.is_deleted)
        .filter(|s| {
            if let Some(pid) = project_id {
                s.project_id == pid
            } else {
                true
            }
        })
        .collect();

    // Sort by section_order for consistent display
    sections.sort_by_key(|s| s.section_order);

    sections
}

/// Applies the limit to the sections.
fn apply_limit<'a>(sections: Vec<&'a Section>, opts: &SectionsListOptions) -> Vec<&'a Section> {
    if let Some(limit) = opts.limit {
        sections.into_iter().take(limit as usize).collect()
    } else {
        sections
    }
}

// ============================================================================
// Sections Add Command
// ============================================================================

/// Options for the sections add command.
#[derive(Debug)]
pub struct SectionsAddOptions {
    /// Section name.
    pub name: String,
    /// Project ID or name (required).
    pub project: String,
}

/// Result of a successful section add operation.
#[derive(Debug)]
pub struct SectionAddResult {
    /// The real ID of the created section.
    pub id: String,
    /// The name of the created section.
    pub name: String,
    /// The project ID.
    pub project_id: String,
    /// The project name.
    pub project_name: Option<String>,
}

/// Executes the sections add command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Sections add command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if the API returns an error.
pub async fn execute_add(
    ctx: &CommandContext,
    opts: &SectionsAddOptions,
    token: &str,
) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Resolve project name to ID and extract owned data before mutation
    let (project_id, project_name) = {
        let cache = manager.cache();
        let pid = resolve_project_id(cache, &opts.project)?;
        let pname = cache
            .projects
            .iter()
            .find(|p| p.id == pid)
            .map(|p| p.name.clone());
        (pid, pname)
    };

    // Build the section_add command arguments
    let temp_id = uuid::Uuid::new_v4().to_string();
    let args = serde_json::json!({
        "name": opts.name,
        "project_id": project_id,
    });

    // Create the command
    let command = SyncCommand::with_temp_id(SyncCommandType::SectionAdd, &temp_id, args);

    // Execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let response = manager.execute_commands(vec![command]).await?;

    // Check for errors
    if response.has_errors() {
        let errors = response.errors();
        if let Some((_, error)) = errors.first() {
            return Err(CommandError::Api(todoist_api_rs::error::Error::Api(
                todoist_api_rs::error::ApiError::Validation {
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
            CommandError::Config("Section created but no ID returned in response".to_string())
        })?
        .clone();

    let result = SectionAddResult {
        id: real_id,
        name: opts.name.clone(),
        project_id,
        project_name,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_created_section(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Created section: {} ({})", result.name, result.id);
            if let Some(ref pname) = result.project_name {
                println!("  Project: {pname}");
            }
        } else {
            println!(
                "Created: {} ({}) in {}",
                result.name,
                &result.id[..6.min(result.id.len())],
                result.project_name.as_deref().unwrap_or(&result.project_id)
            );
        }
    }

    Ok(())
}

// ============================================================================
// Sections Edit Command
// ============================================================================

/// Options for the sections edit command.
#[derive(Debug)]
pub struct SectionsEditOptions {
    /// Section ID (full ID or prefix).
    pub section_id: String,
    /// New section name.
    pub name: Option<String>,
}

/// Result of a successful section edit operation.
#[derive(Debug)]
pub struct SectionEditResult {
    /// The ID of the edited section.
    pub id: String,
    /// The name of the section (possibly updated).
    pub name: String,
    /// Fields that were updated.
    pub updated_fields: Vec<String>,
}

/// Executes the sections edit command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Sections edit command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if section lookup fails or the API returns an error.
pub async fn execute_edit(
    ctx: &CommandContext,
    opts: &SectionsEditOptions,
    token: &str,
) -> Result<()> {
    // Check if any options were provided
    if opts.name.is_none() {
        return Err(CommandError::Config(
            "No changes specified. Use --name to change the section name.".to_string(),
        ));
    }

    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the section by ID or prefix and extract owned data before mutation
    let (section_id, section_name) = {
        let cache = manager.cache();
        let section = find_section_by_id_or_prefix(cache, &opts.section_id)?;
        (section.id.clone(), section.name.clone())
    };

    // Build the section_update command arguments
    let mut args = serde_json::json!({
        "id": section_id,
    });

    let mut updated_fields = Vec::new();

    if let Some(ref name) = opts.name {
        args["name"] = serde_json::json!(name);
        updated_fields.push("name".to_string());
    }

    // Create the command
    let command = SyncCommand::new(SyncCommandType::SectionUpdate, args);

    // Execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let response = manager.execute_commands(vec![command]).await?;

    // Check for errors
    if response.has_errors() {
        let errors = response.errors();
        if let Some((_, error)) = errors.first() {
            return Err(CommandError::Api(todoist_api_rs::error::Error::Api(
                todoist_api_rs::error::ApiError::Validation {
                    field: None,
                    message: format!("Error {}: {}", error.error_code, error.error),
                },
            )));
        }
    }

    let result = SectionEditResult {
        id: section_id,
        name: opts.name.clone().unwrap_or(section_name),
        updated_fields,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_edited_section(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Updated section: {} ({})", result.name, result.id);
            println!("  Changed fields: {}", result.updated_fields.join(", "));
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Updated: {} ({})", result.name, prefix);
        }
    }

    Ok(())
}

/// Finds a section by full ID or unique prefix.
fn find_section_by_id_or_prefix<'a>(cache: &'a Cache, id: &str) -> Result<&'a Section> {
    // First try exact match
    if let Some(section) = cache.sections.iter().find(|s| s.id == id && !s.is_deleted) {
        return Ok(section);
    }

    // Try prefix match
    let matches: Vec<&Section> = cache
        .sections
        .iter()
        .filter(|s| s.id.starts_with(id) && !s.is_deleted)
        .collect();

    match matches.len() {
        0 => Err(CommandError::Config(format!("Section not found: {id}"))),
        1 => Ok(matches[0]),
        _ => {
            // Ambiguous prefix - provide helpful error message
            let mut msg =
                format!("Ambiguous section ID \"{id}\"\n\nMultiple sections match this prefix:");
            for section in matches.iter().take(5) {
                let prefix = &section.id[..6.min(section.id.len())];
                msg.push_str(&format!("\n  {}  {}", prefix, section.name));
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
// Sections Delete Command
// ============================================================================

/// Options for the sections delete command.
#[derive(Debug)]
pub struct SectionsDeleteOptions {
    /// Section ID (full ID or prefix).
    pub section_id: String,
    /// Skip confirmation.
    pub force: bool,
}

/// Result of a successful section delete operation.
#[derive(Debug)]
pub struct SectionDeleteResult {
    /// The ID of the deleted section.
    pub id: String,
    /// The name of the deleted section.
    pub name: String,
}

/// Executes the sections delete command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Sections delete command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if section lookup fails or the API returns an error.
pub async fn execute_delete(
    ctx: &CommandContext,
    opts: &SectionsDeleteOptions,
    token: &str,
) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the section by ID or prefix and extract owned data before mutation
    let (section_id, section_name) = {
        let cache = manager.cache();
        let section = find_section_by_id_or_prefix(cache, &opts.section_id)?;
        (section.id.clone(), section.name.clone())
    };

    // Confirm if not forced
    if !opts.force && !ctx.quiet {
        eprintln!(
            "Delete section '{}' ({})?",
            section_name,
            &section_id[..6.min(section_id.len())]
        );
        eprintln!("This will also delete all tasks in this section.");
        eprintln!("Use --force to skip this confirmation.");
        return Err(CommandError::Config(
            "Operation cancelled. Use --force to confirm.".to_string(),
        ));
    }

    // Build the section_delete command arguments
    let args = serde_json::json!({
        "id": section_id,
    });

    // Create the command
    let command = SyncCommand::new(SyncCommandType::SectionDelete, args);

    // Execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let response = manager.execute_commands(vec![command]).await?;

    // Check for errors
    if response.has_errors() {
        let errors = response.errors();
        if let Some((_, error)) = errors.first() {
            return Err(CommandError::Api(todoist_api_rs::error::Error::Api(
                todoist_api_rs::error::ApiError::Validation {
                    field: None,
                    message: format!("Error {}: {}", error.error_code, error.error),
                },
            )));
        }
    }

    let result = SectionDeleteResult {
        id: section_id,
        name: section_name,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_deleted_section(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Deleted section: {} ({})", result.name, result.id);
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Deleted: {} ({})", result.name, prefix);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sections_list_options_defaults() {
        let opts = SectionsListOptions::default();

        assert!(opts.project.is_none());
        assert!(opts.limit.is_none());
    }

    #[test]
    fn test_sections_list_options_with_values() {
        let opts = SectionsListOptions {
            project: Some("project-123".to_string()),
            limit: Some(10),
        };

        assert_eq!(opts.project, Some("project-123".to_string()));
        assert_eq!(opts.limit, Some(10));
    }

    #[test]
    fn test_sections_add_options() {
        let opts = SectionsAddOptions {
            name: "Groceries".to_string(),
            project: "Shopping".to_string(),
        };

        assert_eq!(opts.name, "Groceries");
        assert_eq!(opts.project, "Shopping");
    }

    #[test]
    fn test_sections_edit_options() {
        let opts = SectionsEditOptions {
            section_id: "section-123".to_string(),
            name: Some("new-name".to_string()),
        };

        assert_eq!(opts.section_id, "section-123");
        assert_eq!(opts.name, Some("new-name".to_string()));
    }

    #[test]
    fn test_sections_edit_options_minimal() {
        let opts = SectionsEditOptions {
            section_id: "section-456".to_string(),
            name: None,
        };

        assert_eq!(opts.section_id, "section-456");
        assert!(opts.name.is_none());
    }

    #[test]
    fn test_section_edit_result() {
        let result = SectionEditResult {
            id: "section-123".to_string(),
            name: "updated".to_string(),
            updated_fields: vec!["name".to_string()],
        };

        assert_eq!(result.id, "section-123");
        assert_eq!(result.name, "updated");
        assert_eq!(result.updated_fields, vec!["name"]);
    }

    #[test]
    fn test_sections_delete_options() {
        let opts = SectionsDeleteOptions {
            section_id: "section-123".to_string(),
            force: false,
        };

        assert_eq!(opts.section_id, "section-123");
        assert!(!opts.force);
    }

    #[test]
    fn test_sections_delete_options_with_force() {
        let opts = SectionsDeleteOptions {
            section_id: "section-456".to_string(),
            force: true,
        };

        assert_eq!(opts.section_id, "section-456");
        assert!(opts.force);
    }

    #[test]
    fn test_section_delete_result() {
        let result = SectionDeleteResult {
            id: "section-789".to_string(),
            name: "deleted-section".to_string(),
        };

        assert_eq!(result.id, "section-789");
        assert_eq!(result.name, "deleted-section");
    }

    #[test]
    fn test_find_section_by_id_or_prefix_exact_match() {
        let cache = make_test_cache_with_sections();
        let result = find_section_by_id_or_prefix(&cache, "section-123-abc");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "section-123-abc");
    }

    #[test]
    fn test_find_section_by_id_or_prefix_unique_prefix() {
        let cache = make_test_cache_with_sections();
        let result = find_section_by_id_or_prefix(&cache, "section-123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "section-123-abc");
    }

    #[test]
    fn test_find_section_by_id_or_prefix_not_found() {
        let cache = make_test_cache_with_sections();
        let result = find_section_by_id_or_prefix(&cache, "nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Section not found"));
    }

    #[test]
    fn test_find_section_by_id_or_prefix_ambiguous() {
        let cache = make_cache_with_ambiguous_section_ids();
        let result = find_section_by_id_or_prefix(&cache, "section-");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ambiguous"));
    }

    #[test]
    fn test_find_section_by_id_or_prefix_ignores_deleted() {
        let mut cache = make_test_cache_with_sections();
        // Mark the section as deleted
        cache.sections[0].is_deleted = true;

        let result = find_section_by_id_or_prefix(&cache, "section-123");
        assert!(result.is_err());
    }

    // Helper function to create a test cache with sections
    fn make_test_cache_with_sections() -> Cache {
        Cache::with_data(
            "test".to_string(),
            None,
            None,
            vec![],
            vec![make_test_project("project-1", "Test Project")],
            vec![],
            vec![make_test_section(
                "section-123-abc",
                "Groceries",
                "project-1",
            )],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
    }

    fn make_cache_with_ambiguous_section_ids() -> Cache {
        Cache::with_data(
            "test".to_string(),
            None,
            None,
            vec![],
            vec![make_test_project("project-1", "Test Project")],
            vec![],
            vec![
                make_test_section("section-aaa-111", "section1", "project-1"),
                make_test_section("section-aaa-222", "section2", "project-1"),
                make_test_section("section-bbb-333", "section3", "project-1"),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
    }

    fn make_test_section(id: &str, name: &str, project_id: &str) -> Section {
        Section {
            id: id.to_string(),
            name: name.to_string(),
            project_id: project_id.to_string(),
            section_order: 0,
            is_collapsed: false,
            is_deleted: false,
            is_archived: false,
            archived_at: None,
            added_at: None,
            updated_at: None,
        }
    }

    fn make_test_project(id: &str, name: &str) -> todoist_api_rs::sync::Project {
        todoist_api_rs::sync::Project {
            id: id.to_string(),
            name: name.to_string(),
            color: None,
            parent_id: None,
            child_order: 0,
            is_collapsed: false,
            shared: false,
            can_assign_tasks: false,
            is_deleted: false,
            is_archived: false,
            is_favorite: false,
            view_style: None,
            inbox_project: false,
            folder_id: None,
            created_at: None,
            updated_at: None,
        }
    }
}
