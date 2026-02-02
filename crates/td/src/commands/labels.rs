//! Labels command implementation.
//!
//! Lists and manages labels via the Sync API.
//! Uses SyncManager::execute_commands() to automatically update the cache.

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{Label, SyncCommand, SyncCommandType};
use todoist_cache_rs::{Cache, CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};
use crate::output::{format_labels_json, format_labels_table};

/// Options for the labels list command.
#[derive(Debug, Default)]
pub struct LabelsListOptions {
    /// Limit results.
    pub limit: Option<u32>,
}

/// Executes the labels list command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Labels list command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails.
pub async fn execute(ctx: &CommandContext, opts: &LabelsListOptions, token: &str) -> Result<()> {
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

    // Get labels and apply filters
    let labels = filter_labels(cache);

    // Apply limit
    let labels = apply_limit(labels, opts);

    // Output
    if ctx.json_output {
        let output = format_labels_json(&labels)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_labels_table(&labels, ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

/// Filters labels (excludes deleted).
fn filter_labels(cache: &Cache) -> Vec<&Label> {
    let mut labels: Vec<&Label> = cache.labels.iter().filter(|l| !l.is_deleted).collect();

    // Sort by item_order for consistent display
    labels.sort_by_key(|l| l.item_order);

    labels
}

/// Applies the limit to the labels.
fn apply_limit<'a>(labels: Vec<&'a Label>, opts: &LabelsListOptions) -> Vec<&'a Label> {
    if let Some(limit) = opts.limit {
        labels.into_iter().take(limit as usize).collect()
    } else {
        labels
    }
}

// ============================================================================
// Labels Add Command
// ============================================================================

/// Options for the labels add command.
#[derive(Debug)]
pub struct LabelsAddOptions {
    /// Label name.
    pub name: String,
    /// Label color.
    pub color: Option<String>,
    /// Mark as favorite.
    pub favorite: bool,
}

/// Result of a successful label add operation.
#[derive(Debug)]
pub struct LabelAddResult {
    /// The real ID of the created label.
    pub id: String,
    /// The name of the created label.
    pub name: String,
    /// The color of the created label.
    pub color: Option<String>,
    /// Whether the label is a favorite.
    pub is_favorite: bool,
}

/// Executes the labels add command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Labels add command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if the API returns an error.
pub async fn execute_add(ctx: &CommandContext, opts: &LabelsAddOptions, token: &str) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Validate color if provided
    if let Some(ref color) = opts.color {
        if !is_valid_color(color) {
            return Err(CommandError::Config(format!(
                "Invalid color: {color}. Valid colors: berry_red, red, orange, yellow, olive_green, lime_green, green, mint_green, teal, sky_blue, light_blue, blue, grape, violet, lavender, magenta, salmon, charcoal, grey, taupe"
            )));
        }
    }

    // Build the label_add command arguments
    let temp_id = uuid::Uuid::new_v4().to_string();
    let mut args = serde_json::json!({
        "name": opts.name,
    });

    // Add optional fields
    if let Some(ref color) = opts.color {
        args["color"] = serde_json::json!(color);
    }

    if opts.favorite {
        args["is_favorite"] = serde_json::json!(true);
    }

    // Create the command
    let command = SyncCommand::with_temp_id(SyncCommandType::LabelAdd, &temp_id, args);

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
            CommandError::Config("Label created but no ID returned in response".to_string())
        })?
        .clone();

    let result = LabelAddResult {
        id: real_id,
        name: opts.name.clone(),
        color: opts.color.clone(),
        is_favorite: opts.favorite,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_created_label(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Created label: {} ({})", result.name, result.id);
            if let Some(ref color) = result.color {
                println!("  Color: {color}");
            }
            if result.is_favorite {
                println!("  Favorite: yes");
            }
        } else {
            println!(
                "Created: @{} ({})",
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
// Labels Edit Command
// ============================================================================

/// Options for the labels edit command.
#[derive(Debug)]
pub struct LabelsEditOptions {
    /// Label ID (full ID or prefix).
    pub label_id: String,
    /// New label name.
    pub name: Option<String>,
    /// New label color.
    pub color: Option<String>,
    /// Set favorite status.
    pub favorite: Option<bool>,
}

/// Result of a successful label edit operation.
#[derive(Debug)]
pub struct LabelEditResult {
    /// The ID of the edited label.
    pub id: String,
    /// The name of the label (possibly updated).
    pub name: String,
    /// Fields that were updated.
    pub updated_fields: Vec<String>,
}

/// Executes the labels edit command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Labels edit command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if label lookup fails or the API returns an error.
pub async fn execute_edit(
    ctx: &CommandContext,
    opts: &LabelsEditOptions,
    token: &str,
) -> Result<()> {
    // Check if any options were provided
    if opts.name.is_none() && opts.color.is_none() && opts.favorite.is_none() {
        return Err(CommandError::Config(
            "No changes specified. Use --name, --color, or --favorite.".to_string(),
        ));
    }

    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the label by ID or prefix and extract owned data before mutation
    let (label_id, label_name) = {
        let cache = manager.cache();
        let label = find_label_by_id_or_prefix(cache, &opts.label_id)?;
        (label.id.clone(), label.name.clone())
    };

    // Validate color if provided
    if let Some(ref color) = opts.color {
        if !is_valid_color(color) {
            return Err(CommandError::Config(format!(
                "Invalid color: {color}. Valid colors: berry_red, red, orange, yellow, olive_green, lime_green, green, mint_green, teal, sky_blue, light_blue, blue, grape, violet, lavender, magenta, salmon, charcoal, grey, taupe"
            )));
        }
    }

    // Build the label_update command arguments
    let mut args = serde_json::json!({
        "id": label_id,
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

    // Create the command
    let command = SyncCommand::new(SyncCommandType::LabelUpdate, args);

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

    let result = LabelEditResult {
        id: label_id,
        name: opts.name.clone().unwrap_or(label_name),
        updated_fields,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_edited_label(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Updated label: @{} ({})", result.name, result.id);
            println!("  Changed fields: {}", result.updated_fields.join(", "));
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Updated: @{} ({})", result.name, prefix);
        }
    }

    Ok(())
}

/// Finds a label by full ID or unique prefix.
fn find_label_by_id_or_prefix<'a>(cache: &'a Cache, id: &str) -> Result<&'a Label> {
    // First try exact match
    if let Some(label) = cache.labels.iter().find(|l| l.id == id && !l.is_deleted) {
        return Ok(label);
    }

    // Try prefix match
    let matches: Vec<&Label> = cache
        .labels
        .iter()
        .filter(|l| l.id.starts_with(id) && !l.is_deleted)
        .collect();

    match matches.len() {
        0 => Err(CommandError::Config(format!("Label not found: {id}"))),
        1 => Ok(matches[0]),
        _ => {
            // Ambiguous prefix - provide helpful error message
            let mut msg =
                format!("Ambiguous label ID \"{id}\"\n\nMultiple labels match this prefix:");
            for label in matches.iter().take(5) {
                let prefix = &label.id[..6.min(label.id.len())];
                msg.push_str(&format!("\n  {}  @{}", prefix, label.name));
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
// Labels Delete Command
// ============================================================================

/// Options for the labels delete command.
#[derive(Debug)]
pub struct LabelsDeleteOptions {
    /// Label ID (full ID or prefix).
    pub label_id: String,
    /// Skip confirmation.
    pub force: bool,
}

/// Result of a successful label delete operation.
#[derive(Debug)]
pub struct LabelDeleteResult {
    /// The ID of the deleted label.
    pub id: String,
    /// The name of the deleted label.
    pub name: String,
}

/// Executes the labels delete command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Labels delete command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if label lookup fails or the API returns an error.
pub async fn execute_delete(
    ctx: &CommandContext,
    opts: &LabelsDeleteOptions,
    token: &str,
) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the label by ID or prefix and extract owned data before mutation
    let (label_id, label_name) = {
        let cache = manager.cache();
        let label = find_label_by_id_or_prefix(cache, &opts.label_id)?;
        (label.id.clone(), label.name.clone())
    };

    // Confirm if not forced
    if !opts.force && !ctx.quiet {
        eprintln!(
            "Delete label '@{}' ({})?",
            label_name,
            &label_id[..6.min(label_id.len())]
        );
        eprintln!("This will remove the label from all tasks.");
        eprintln!("Use --force to skip this confirmation.");
        return Err(CommandError::Config(
            "Operation cancelled. Use --force to confirm.".to_string(),
        ));
    }

    // Build the label_delete command arguments
    let args = serde_json::json!({
        "id": label_id,
    });

    // Create the command
    let command = SyncCommand::new(SyncCommandType::LabelDelete, args);

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

    let result = LabelDeleteResult {
        id: label_id,
        name: label_name,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_deleted_label(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Deleted label: @{} ({})", result.name, result.id);
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Deleted: @{} ({})", result.name, prefix);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_labels_list_options_defaults() {
        let opts = LabelsListOptions::default();

        assert!(opts.limit.is_none());
    }

    #[test]
    fn test_labels_list_options_with_values() {
        let opts = LabelsListOptions { limit: Some(10) };

        assert_eq!(opts.limit, Some(10));
    }

    #[test]
    fn test_labels_add_options() {
        let opts = LabelsAddOptions {
            name: "urgent".to_string(),
            color: Some("red".to_string()),
            favorite: true,
        };

        assert_eq!(opts.name, "urgent");
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
    fn test_labels_edit_options() {
        let opts = LabelsEditOptions {
            label_id: "label-123".to_string(),
            name: Some("new-name".to_string()),
            color: Some("blue".to_string()),
            favorite: Some(true),
        };

        assert_eq!(opts.label_id, "label-123");
        assert_eq!(opts.name, Some("new-name".to_string()));
        assert_eq!(opts.color, Some("blue".to_string()));
        assert_eq!(opts.favorite, Some(true));
    }

    #[test]
    fn test_labels_edit_options_minimal() {
        let opts = LabelsEditOptions {
            label_id: "label-456".to_string(),
            name: None,
            color: None,
            favorite: Some(false),
        };

        assert_eq!(opts.label_id, "label-456");
        assert!(opts.name.is_none());
        assert!(opts.color.is_none());
        assert_eq!(opts.favorite, Some(false));
    }

    #[test]
    fn test_label_edit_result() {
        let result = LabelEditResult {
            id: "label-123".to_string(),
            name: "updated".to_string(),
            updated_fields: vec!["name".to_string(), "color".to_string()],
        };

        assert_eq!(result.id, "label-123");
        assert_eq!(result.name, "updated");
        assert_eq!(result.updated_fields, vec!["name", "color"]);
    }

    #[test]
    fn test_labels_delete_options() {
        let opts = LabelsDeleteOptions {
            label_id: "label-123".to_string(),
            force: false,
        };

        assert_eq!(opts.label_id, "label-123");
        assert!(!opts.force);
    }

    #[test]
    fn test_labels_delete_options_with_force() {
        let opts = LabelsDeleteOptions {
            label_id: "label-456".to_string(),
            force: true,
        };

        assert_eq!(opts.label_id, "label-456");
        assert!(opts.force);
    }

    #[test]
    fn test_label_delete_result() {
        let result = LabelDeleteResult {
            id: "label-789".to_string(),
            name: "deleted-label".to_string(),
        };

        assert_eq!(result.id, "label-789");
        assert_eq!(result.name, "deleted-label");
    }

    #[test]
    fn test_find_label_by_id_or_prefix_exact_match() {
        let cache = make_test_cache_with_labels();
        let result = find_label_by_id_or_prefix(&cache, "label-123-abc");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "label-123-abc");
    }

    #[test]
    fn test_find_label_by_id_or_prefix_unique_prefix() {
        let cache = make_test_cache_with_labels();
        let result = find_label_by_id_or_prefix(&cache, "label-123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "label-123-abc");
    }

    #[test]
    fn test_find_label_by_id_or_prefix_not_found() {
        let cache = make_test_cache_with_labels();
        let result = find_label_by_id_or_prefix(&cache, "nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Label not found"));
    }

    #[test]
    fn test_find_label_by_id_or_prefix_ambiguous() {
        let cache = make_cache_with_ambiguous_label_ids();
        let result = find_label_by_id_or_prefix(&cache, "label-");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ambiguous"));
    }

    #[test]
    fn test_find_label_by_id_or_prefix_ignores_deleted() {
        let mut cache = make_test_cache_with_labels();
        // Mark the label as deleted
        cache.labels[0].is_deleted = true;

        let result = find_label_by_id_or_prefix(&cache, "label-123");
        assert!(result.is_err());
    }

    // Helper function to create a test cache with labels
    fn make_test_cache_with_labels() -> Cache {
        Cache::with_data(
            "test".to_string(),
            None,
            None,
            vec![],
            vec![],
            vec![make_test_label("label-123-abc", "urgent")],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
    }

    fn make_cache_with_ambiguous_label_ids() -> Cache {
        Cache::with_data(
            "test".to_string(),
            None,
            None,
            vec![],
            vec![],
            vec![
                make_test_label("label-aaa-111", "label1"),
                make_test_label("label-aaa-222", "label2"),
                make_test_label("label-bbb-333", "label3"),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
    }

    fn make_test_label(id: &str, name: &str) -> Label {
        Label {
            id: id.to_string(),
            name: name.to_string(),
            color: None,
            item_order: 0,
            is_deleted: false,
            is_favorite: false,
        }
    }
}
