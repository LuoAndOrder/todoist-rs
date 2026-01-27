//! Filters command implementation.
//!
//! Lists and manages saved filters via the Sync API.
//! Uses SyncManager::execute_commands() to automatically update the cache.

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{Filter, SyncCommand};
use todoist_cache_rs::{Cache, CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};
use crate::output::{
    format_created_filter, format_deleted_filter, format_edited_filter, format_filter_details_json,
    format_filter_details_table, format_filters_json, format_filters_table,
};

/// Options for the filters list command.
#[derive(Debug, Default)]
pub struct FiltersListOptions {
    /// Limit results.
    pub limit: Option<u32>,
}

/// Executes the filters list command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Filters list command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails.
pub async fn execute(ctx: &CommandContext, opts: &FiltersListOptions, token: &str) -> Result<()> {
    // Initialize sync manager
    let client = TodoistClient::new(token);
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

    // Get filters and apply filters
    let filters = filter_filters(cache);

    // Apply limit
    let filters = apply_limit(filters, opts);

    // Output
    if ctx.json_output {
        let output = format_filters_json(&filters)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_filters_table(&filters, ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

/// Filters filters (excludes deleted).
fn filter_filters(cache: &Cache) -> Vec<&Filter> {
    let mut filters: Vec<&Filter> = cache.filters.iter().filter(|f| !f.is_deleted).collect();

    // Sort by item_order for consistent display
    filters.sort_by_key(|f| f.item_order);

    filters
}

/// Applies the limit to the filters.
fn apply_limit<'a>(filters: Vec<&'a Filter>, opts: &FiltersListOptions) -> Vec<&'a Filter> {
    if let Some(limit) = opts.limit {
        filters.into_iter().take(limit as usize).collect()
    } else {
        filters
    }
}

// ============================================================================
// Filters Add Command
// ============================================================================

/// Options for the filters add command.
#[derive(Debug)]
pub struct FiltersAddOptions {
    /// Filter name.
    pub name: String,
    /// Filter query string.
    pub query: String,
    /// Filter color.
    pub color: Option<String>,
    /// Mark as favorite.
    pub favorite: bool,
}

/// Result of a successful filter add operation.
#[derive(Debug)]
pub struct FilterAddResult {
    /// The real ID of the created filter.
    pub id: String,
    /// The name of the created filter.
    pub name: String,
    /// The query of the created filter.
    pub query: String,
    /// The color of the created filter.
    pub color: Option<String>,
    /// Whether the filter is a favorite.
    pub is_favorite: bool,
}

/// Executes the filters add command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Filters add command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if the API returns an error.
pub async fn execute_add(
    ctx: &CommandContext,
    opts: &FiltersAddOptions,
    token: &str,
) -> Result<()> {
    // Validate color if provided
    if let Some(ref color) = opts.color {
        if !is_valid_color(color) {
            return Err(CommandError::Config(format!(
                "Invalid color: {color}. Valid colors: berry_red, red, orange, yellow, olive_green, lime_green, green, mint_green, teal, sky_blue, light_blue, blue, grape, violet, lavender, magenta, salmon, charcoal, grey, taupe"
            )));
        }
    }

    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Build the filter_add command arguments
    let temp_id = uuid::Uuid::new_v4().to_string();
    let mut args = serde_json::json!({
        "name": opts.name,
        "query": opts.query,
    });

    // Add optional fields
    if let Some(ref color) = opts.color {
        args["color"] = serde_json::json!(color);
    }

    if opts.favorite {
        args["is_favorite"] = serde_json::json!(true);
    }

    // Create the command
    let command = SyncCommand::with_temp_id("filter_add", &temp_id, args);

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
            CommandError::Config("Filter created but no ID returned in response".to_string())
        })?
        .clone();

    let result = FilterAddResult {
        id: real_id,
        name: opts.name.clone(),
        query: opts.query.clone(),
        color: opts.color.clone(),
        is_favorite: opts.favorite,
    };

    // Output
    if ctx.json_output {
        let output = format_created_filter(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Created filter: {} ({})", result.name, result.id);
            println!("  Query: {}", result.query);
            if let Some(ref color) = result.color {
                println!("  Color: {color}");
            }
            if result.is_favorite {
                println!("  Favorite: yes");
            }
        } else {
            println!(
                "Created: {} ({})",
                result.name,
                &result.id[..6.min(result.id.len())]
            );
        }
    }

    Ok(())
}

/// Valid Todoist color names.
const VALID_COLORS: &[&str] = &[
    "berry_red",
    "red",
    "orange",
    "yellow",
    "olive_green",
    "lime_green",
    "green",
    "mint_green",
    "teal",
    "sky_blue",
    "light_blue",
    "blue",
    "grape",
    "violet",
    "lavender",
    "magenta",
    "salmon",
    "charcoal",
    "grey",
    "taupe",
];

/// Checks if a color name is valid.
fn is_valid_color(color: &str) -> bool {
    VALID_COLORS.contains(&color)
}

// ============================================================================
// Filters Show Command
// ============================================================================

/// Options for the filters show command.
#[derive(Debug)]
pub struct FiltersShowOptions {
    /// Filter ID (full ID or prefix).
    pub filter_id: String,
}

/// Result of a successful filter show operation.
#[derive(Debug)]
pub struct FilterShowResult {
    /// The filter.
    pub filter: Filter,
}

/// Executes the filters show command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Filters show command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails or filter lookup fails.
pub async fn execute_show(
    ctx: &CommandContext,
    opts: &FiltersShowOptions,
    token: &str,
) -> Result<()> {
    // Initialize sync manager to resolve filter ID
    let client = TodoistClient::new(token);
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

    // Find the filter by ID or prefix
    let filter = find_filter_by_id_or_prefix(cache, &opts.filter_id)?;

    let result = FilterShowResult {
        filter: filter.clone(),
    };

    // Output
    if ctx.json_output {
        let output = format_filter_details_json(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_filter_details_table(&result, ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

// ============================================================================
// Filters Edit Command
// ============================================================================

/// Options for the filters edit command.
#[derive(Debug)]
pub struct FiltersEditOptions {
    /// Filter ID (full ID or prefix).
    pub filter_id: String,
    /// New filter name.
    pub name: Option<String>,
    /// New filter query.
    pub query: Option<String>,
    /// New filter color.
    pub color: Option<String>,
    /// Set favorite status.
    pub favorite: Option<bool>,
}

/// Result of a successful filter edit operation.
#[derive(Debug)]
pub struct FilterEditResult {
    /// The ID of the edited filter.
    pub id: String,
    /// The name of the filter (possibly updated).
    pub name: String,
    /// Fields that were updated.
    pub updated_fields: Vec<String>,
}

/// Executes the filters edit command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Filters edit command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if filter lookup fails or the API returns an error.
pub async fn execute_edit(
    ctx: &CommandContext,
    opts: &FiltersEditOptions,
    token: &str,
) -> Result<()> {
    // Check if any options were provided
    if opts.name.is_none()
        && opts.query.is_none()
        && opts.color.is_none()
        && opts.favorite.is_none()
    {
        return Err(CommandError::Config(
            "No changes specified. Use --name, --query, --color, or --favorite.".to_string(),
        ));
    }

    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the filter by ID or prefix and extract owned data before mutation
    let (filter_id, filter_name) = {
        let cache = manager.cache();
        let filter = find_filter_by_id_or_prefix(cache, &opts.filter_id)?;
        (filter.id.clone(), filter.name.clone())
    };

    // Validate color if provided
    if let Some(ref color) = opts.color {
        if !is_valid_color(color) {
            return Err(CommandError::Config(format!(
                "Invalid color: {color}. Valid colors: berry_red, red, orange, yellow, olive_green, lime_green, green, mint_green, teal, sky_blue, light_blue, blue, grape, violet, lavender, magenta, salmon, charcoal, grey, taupe"
            )));
        }
    }

    // Build the filter_update command arguments
    let mut args = serde_json::json!({
        "id": filter_id,
    });

    let mut updated_fields = Vec::new();

    if let Some(ref name) = opts.name {
        args["name"] = serde_json::json!(name);
        updated_fields.push("name".to_string());
    }

    if let Some(ref query) = opts.query {
        args["query"] = serde_json::json!(query);
        updated_fields.push("query".to_string());
    }

    if let Some(ref color) = opts.color {
        args["color"] = serde_json::json!(color);
        updated_fields.push("color".to_string());
    }

    if let Some(favorite) = opts.favorite {
        args["is_favorite"] = serde_json::json!(favorite);
        updated_fields.push("favorite".to_string());
    }

    // Create the command
    let command = SyncCommand::new("filter_update", args);

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

    let result = FilterEditResult {
        id: filter_id,
        name: opts.name.clone().unwrap_or(filter_name),
        updated_fields,
    };

    // Output
    if ctx.json_output {
        let output = format_edited_filter(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Updated filter: {} ({})", result.name, result.id);
            println!("  Changed fields: {}", result.updated_fields.join(", "));
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Updated: {} ({})", result.name, prefix);
        }
    }

    Ok(())
}

/// Finds a filter by full ID or unique prefix.
fn find_filter_by_id_or_prefix<'a>(cache: &'a Cache, id: &str) -> Result<&'a Filter> {
    // First try exact match
    if let Some(filter) = cache.filters.iter().find(|f| f.id == id && !f.is_deleted) {
        return Ok(filter);
    }

    // Try prefix match
    let matches: Vec<&Filter> = cache
        .filters
        .iter()
        .filter(|f| f.id.starts_with(id) && !f.is_deleted)
        .collect();

    match matches.len() {
        0 => Err(CommandError::Config(format!("Filter not found: {id}"))),
        1 => Ok(matches[0]),
        _ => {
            // Ambiguous prefix - provide helpful error message
            let mut msg =
                format!("Ambiguous filter ID \"{id}\"\n\nMultiple filters match this prefix:");
            for filter in matches.iter().take(5) {
                let prefix = &filter.id[..6.min(filter.id.len())];
                msg.push_str(&format!("\n  {}  {}", prefix, filter.name));
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
// Filters Delete Command
// ============================================================================

/// Options for the filters delete command.
#[derive(Debug)]
pub struct FiltersDeleteOptions {
    /// Filter ID (full ID or prefix).
    pub filter_id: String,
    /// Skip confirmation.
    pub force: bool,
}

/// Result of a successful filter delete operation.
#[derive(Debug)]
pub struct FilterDeleteResult {
    /// The ID of the deleted filter.
    pub id: String,
    /// The name of the deleted filter.
    pub name: String,
}

/// Executes the filters delete command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Filters delete command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if filter lookup fails or the API returns an error.
pub async fn execute_delete(
    ctx: &CommandContext,
    opts: &FiltersDeleteOptions,
    token: &str,
) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the filter by ID or prefix and extract owned data before mutation
    let (filter_id, filter_name) = {
        let cache = manager.cache();
        let filter = find_filter_by_id_or_prefix(cache, &opts.filter_id)?;
        (filter.id.clone(), filter.name.clone())
    };

    // Confirm if not forced
    if !opts.force && !ctx.quiet {
        eprintln!(
            "Delete filter '{}' ({})?",
            filter_name,
            &filter_id[..6.min(filter_id.len())]
        );
        eprintln!("Use --force to skip this confirmation.");
        return Err(CommandError::Config(
            "Operation cancelled. Use --force to confirm.".to_string(),
        ));
    }

    // Build the filter_delete command arguments
    let args = serde_json::json!({
        "id": filter_id,
    });

    // Create the command
    let command = SyncCommand::new("filter_delete", args);

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

    let result = FilterDeleteResult {
        id: filter_id,
        name: filter_name,
    };

    // Output
    if ctx.json_output {
        let output = format_deleted_filter(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Deleted filter: {} ({})", result.name, result.id);
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
    fn test_filters_list_options_defaults() {
        let opts = FiltersListOptions::default();

        assert!(opts.limit.is_none());
    }

    #[test]
    fn test_filters_list_options_with_values() {
        let opts = FiltersListOptions { limit: Some(10) };

        assert_eq!(opts.limit, Some(10));
    }

    #[test]
    fn test_filters_add_options() {
        let opts = FiltersAddOptions {
            name: "Today & High Priority".to_string(),
            query: "today & p1".to_string(),
            color: Some("red".to_string()),
            favorite: true,
        };

        assert_eq!(opts.name, "Today & High Priority");
        assert_eq!(opts.query, "today & p1");
        assert_eq!(opts.color, Some("red".to_string()));
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
    fn test_filters_show_options() {
        let opts = FiltersShowOptions {
            filter_id: "filter-123".to_string(),
        };

        assert_eq!(opts.filter_id, "filter-123");
    }

    #[test]
    fn test_filters_edit_options() {
        let opts = FiltersEditOptions {
            filter_id: "filter-123".to_string(),
            name: Some("new-name".to_string()),
            query: Some("today".to_string()),
            color: Some("blue".to_string()),
            favorite: Some(true),
        };

        assert_eq!(opts.filter_id, "filter-123");
        assert_eq!(opts.name, Some("new-name".to_string()));
        assert_eq!(opts.query, Some("today".to_string()));
        assert_eq!(opts.color, Some("blue".to_string()));
        assert_eq!(opts.favorite, Some(true));
    }

    #[test]
    fn test_filters_edit_options_minimal() {
        let opts = FiltersEditOptions {
            filter_id: "filter-456".to_string(),
            name: None,
            query: None,
            color: None,
            favorite: Some(false),
        };

        assert_eq!(opts.filter_id, "filter-456");
        assert!(opts.name.is_none());
        assert!(opts.query.is_none());
        assert!(opts.color.is_none());
        assert_eq!(opts.favorite, Some(false));
    }

    #[test]
    fn test_filter_edit_result() {
        let result = FilterEditResult {
            id: "filter-123".to_string(),
            name: "updated".to_string(),
            updated_fields: vec!["name".to_string(), "query".to_string()],
        };

        assert_eq!(result.id, "filter-123");
        assert_eq!(result.name, "updated");
        assert_eq!(result.updated_fields, vec!["name", "query"]);
    }

    #[test]
    fn test_filters_delete_options() {
        let opts = FiltersDeleteOptions {
            filter_id: "filter-123".to_string(),
            force: false,
        };

        assert_eq!(opts.filter_id, "filter-123");
        assert!(!opts.force);
    }

    #[test]
    fn test_filters_delete_options_with_force() {
        let opts = FiltersDeleteOptions {
            filter_id: "filter-456".to_string(),
            force: true,
        };

        assert_eq!(opts.filter_id, "filter-456");
        assert!(opts.force);
    }

    #[test]
    fn test_filter_delete_result() {
        let result = FilterDeleteResult {
            id: "filter-789".to_string(),
            name: "deleted-filter".to_string(),
        };

        assert_eq!(result.id, "filter-789");
        assert_eq!(result.name, "deleted-filter");
    }

    #[test]
    fn test_find_filter_by_id_or_prefix_exact_match() {
        let cache = make_test_cache_with_filters();
        let result = find_filter_by_id_or_prefix(&cache, "filter-123-abc");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "filter-123-abc");
    }

    #[test]
    fn test_find_filter_by_id_or_prefix_unique_prefix() {
        let cache = make_test_cache_with_filters();
        let result = find_filter_by_id_or_prefix(&cache, "filter-123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "filter-123-abc");
    }

    #[test]
    fn test_find_filter_by_id_or_prefix_not_found() {
        let cache = make_test_cache_with_filters();
        let result = find_filter_by_id_or_prefix(&cache, "nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Filter not found"));
    }

    #[test]
    fn test_find_filter_by_id_or_prefix_ambiguous() {
        let cache = make_cache_with_ambiguous_filter_ids();
        let result = find_filter_by_id_or_prefix(&cache, "filter-");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ambiguous"));
    }

    #[test]
    fn test_find_filter_by_id_or_prefix_ignores_deleted() {
        let mut cache = make_test_cache_with_filters();
        // Mark the filter as deleted
        cache.filters[0].is_deleted = true;

        let result = find_filter_by_id_or_prefix(&cache, "filter-123");
        assert!(result.is_err());
    }

    // Helper function to create a test cache with filters
    fn make_test_cache_with_filters() -> Cache {
        Cache {
            sync_token: "test".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: vec![],
            projects: vec![],
            labels: vec![],
            sections: vec![],
            notes: vec![],
            project_notes: vec![],
            reminders: vec![],
            filters: vec![make_test_filter("filter-123-abc", "Today", "today")],
            user: None,
        }
    }

    fn make_cache_with_ambiguous_filter_ids() -> Cache {
        Cache {
            sync_token: "test".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: vec![],
            projects: vec![],
            labels: vec![],
            sections: vec![],
            notes: vec![],
            project_notes: vec![],
            reminders: vec![],
            filters: vec![
                make_test_filter("filter-aaa-111", "Filter 1", "today"),
                make_test_filter("filter-aaa-222", "Filter 2", "tomorrow"),
                make_test_filter("filter-bbb-333", "Filter 3", "overdue"),
            ],
            user: None,
        }
    }

    fn make_test_filter(id: &str, name: &str, query: &str) -> Filter {
        Filter {
            id: id.to_string(),
            name: name.to_string(),
            query: query.to_string(),
            color: None,
            item_order: 0,
            is_deleted: false,
            is_favorite: false,
        }
    }
}
