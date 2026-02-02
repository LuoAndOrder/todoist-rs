//! Edit command implementation.
//!
//! Updates a task via the Sync API's `item_update` and/or `item_move` commands.
//! Uses SyncManager::execute_commands() to automatically update the cache.
//! Uses resolve_item_by_prefix(), resolve_project(), and resolve_section()
//! for smart lookups with auto-sync fallback.

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{SyncCommand, SyncCommandType};
use todoist_cache_rs::{CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};

/// Options for the edit command.
#[derive(Debug)]
pub struct EditOptions {
    /// Task ID (full ID or prefix).
    pub task_id: String,
    /// New content/title.
    pub content: Option<String>,
    /// Move to project (name or ID).
    pub project: Option<String>,
    /// New priority level (1=highest, 4=lowest).
    pub priority: Option<u8>,
    /// New due date (natural language or ISO).
    pub due: Option<String>,
    /// Remove due date.
    pub no_due: bool,
    /// Set labels (replaces existing).
    pub labels: Vec<String>,
    /// Add a single label.
    pub add_label: Option<String>,
    /// Remove a single label.
    pub remove_label: Option<String>,
    /// Move to section within project.
    pub section: Option<String>,
    /// New description.
    pub description: Option<String>,
}

/// Result of a successful edit operation.
#[derive(Debug)]
pub struct EditResult {
    /// The task ID.
    pub id: String,
    /// The updated content (if changed).
    pub content: Option<String>,
    /// Fields that were updated.
    pub updated_fields: Vec<String>,
}

/// Executes the edit command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Edit command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails, task lookup fails, or the API returns an error.
pub async fn execute(ctx: &CommandContext, opts: &EditOptions, token: &str) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Resolve task using smart lookup (cache-first with auto-sync fallback)
    // require_checked=None to match any task (edit works on completed and uncompleted)
    let item = manager
        .resolve_item_by_prefix(&opts.task_id, None)
        .await
        .map_err(|e| CommandError::Config(e.to_string()))?;
    let task_id = item.id.clone();
    let current_content = item.content.clone();
    let current_labels = item.labels.clone();
    let current_project_id = item.project_id.clone();
    let current_section_id = item.section_id.clone();

    // Track what we're updating
    let mut updated_fields = Vec::new();
    let mut commands = Vec::new();

    // Build item_update command if any update fields are specified
    let has_updates = opts.content.is_some()
        || opts.priority.is_some()
        || opts.due.is_some()
        || opts.no_due
        || !opts.labels.is_empty()
        || opts.add_label.is_some()
        || opts.remove_label.is_some()
        || opts.description.is_some();

    if has_updates {
        let mut args = serde_json::json!({
            "id": task_id,
        });

        if let Some(ref content) = opts.content {
            args["content"] = serde_json::json!(content);
            updated_fields.push("content".to_string());
        }

        if let Some(priority) = opts.priority {
            // Convert user priority (1=highest) to API priority (4=highest)
            let api_priority = 5 - priority as i32;
            args["priority"] = serde_json::json!(api_priority);
            updated_fields.push("priority".to_string());
        }

        if opts.no_due {
            // Remove due date by setting to null
            args["due"] = serde_json::Value::Null;
            updated_fields.push("due (removed)".to_string());
        } else if let Some(ref due) = opts.due {
            // Use the "string" field to let Todoist parse natural language dates
            args["due"] = serde_json::json!({"string": due});
            updated_fields.push("due".to_string());
        }

        // Handle labels
        if !opts.labels.is_empty() {
            // Replace all labels
            args["labels"] = serde_json::json!(opts.labels);
            updated_fields.push("labels".to_string());
        } else if opts.add_label.is_some() || opts.remove_label.is_some() {
            // Modify existing labels
            let mut new_labels = current_labels.clone();

            if let Some(ref add_label) = opts.add_label {
                if !new_labels.contains(add_label) {
                    new_labels.push(add_label.clone());
                    updated_fields.push(format!("label +{}", add_label));
                }
            }

            if let Some(ref remove_label) = opts.remove_label {
                if let Some(pos) = new_labels.iter().position(|l| l == remove_label) {
                    new_labels.remove(pos);
                    updated_fields.push(format!("label -{}", remove_label));
                }
            }

            args["labels"] = serde_json::json!(new_labels);
        }

        if let Some(ref description) = opts.description {
            args["description"] = serde_json::json!(description);
            updated_fields.push("description".to_string());
        }

        let update_command = SyncCommand::new(SyncCommandType::ItemUpdate, args);
        commands.push(update_command);
    }

    // Build item_move command if moving to different project or section
    // Note: item_move only allows one of project_id, section_id, or parent_id
    if opts.project.is_some() || opts.section.is_some() {
        let mut move_args = serde_json::json!({
            "id": task_id,
        });

        // Resolve project name to ID using smart lookup (cache-first with auto-sync fallback)
        let resolved_project_id = if let Some(ref project_name) = opts.project {
            let project = manager
                .resolve_project(project_name)
                .await
                .map_err(|e| CommandError::Config(e.to_string()))?;

            // Only move if project is different
            if project.id != current_project_id {
                move_args["project_id"] = serde_json::json!(project.id);
                updated_fields.push("project".to_string());
            }
            project.id.clone()
        } else {
            current_project_id.clone()
        };

        // Resolve section name to ID using smart lookup (cache-first with auto-sync fallback)
        if let Some(ref section_name) = opts.section {
            let section = manager
                .resolve_section(section_name, Some(&resolved_project_id))
                .await
                .map_err(|e| CommandError::Config(e.to_string()))?;

            // Only move if section is different
            if current_section_id.as_ref() != Some(&section.id) {
                // Note: When moving to a section, we only set section_id
                // The project will be implicitly set to the section's project
                move_args["section_id"] = serde_json::json!(section.id);
                if !updated_fields.contains(&"project".to_string()) {
                    updated_fields.push("section".to_string());
                } else {
                    // Project already being updated, section is in that project
                    updated_fields.push("section".to_string());
                }
            }
        }

        // Only add move command if we're actually moving somewhere
        if move_args.get("project_id").is_some() || move_args.get("section_id").is_some() {
            let move_command = SyncCommand::new(SyncCommandType::ItemMove, move_args);
            commands.push(move_command);
        }
    }

    // Check if we have any changes to make
    if commands.is_empty() {
        if !ctx.quiet {
            if ctx.json_output {
                let output = serde_json::json!({
                    "status": "no_changes",
                    "id": task_id,
                    "message": "No changes specified"
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!(
                    "No changes specified for task {}",
                    &task_id[..6.min(task_id.len())]
                );
            }
        }
        return Ok(());
    }

    // Execute the commands via SyncManager
    // This sends the commands, applies the response to cache, and saves to disk
    let response = manager.execute_commands(commands).await?;

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

    let result = EditResult {
        id: task_id,
        content: opts.content.clone().or(Some(current_content)),
        updated_fields,
    };

    // Output
    if ctx.json_output {
        let output = serde_json::json!({
            "status": "updated",
            "id": result.id,
            "content": result.content,
            "updated_fields": result.updated_fields
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !ctx.quiet {
        let content_display = result.content.as_deref().unwrap_or("(unknown)");
        if ctx.verbose {
            println!("Updated task: {} ({})", content_display, result.id);
            println!("  Changed: {}", result.updated_fields.join(", "));
        } else {
            println!(
                "Updated: {} ({})",
                content_display,
                &result.id[..6.min(result.id.len())]
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_options_defaults() {
        let opts = EditOptions {
            task_id: "abc123".to_string(),
            content: None,
            project: None,
            priority: None,
            due: None,
            no_due: false,
            labels: vec![],
            add_label: None,
            remove_label: None,
            section: None,
            description: None,
        };

        assert_eq!(opts.task_id, "abc123");
        assert!(opts.content.is_none());
        assert!(!opts.no_due);
        assert!(opts.labels.is_empty());
    }

    #[test]
    fn test_edit_options_with_all_fields() {
        let opts = EditOptions {
            task_id: "abc123def456".to_string(),
            content: Some("Updated content".to_string()),
            project: Some("Work".to_string()),
            priority: Some(1),
            due: Some("tomorrow".to_string()),
            no_due: false,
            labels: vec!["urgent".to_string(), "important".to_string()],
            add_label: None,
            remove_label: None,
            section: Some("In Progress".to_string()),
            description: Some("New description".to_string()),
        };

        assert_eq!(opts.task_id, "abc123def456");
        assert_eq!(opts.content, Some("Updated content".to_string()));
        assert_eq!(opts.project, Some("Work".to_string()));
        assert_eq!(opts.priority, Some(1));
        assert_eq!(opts.due, Some("tomorrow".to_string()));
        assert!(!opts.no_due);
        assert_eq!(opts.labels.len(), 2);
        assert_eq!(opts.section, Some("In Progress".to_string()));
        assert_eq!(opts.description, Some("New description".to_string()));
    }

    #[test]
    fn test_edit_options_no_due() {
        let opts = EditOptions {
            task_id: "abc123".to_string(),
            content: None,
            project: None,
            priority: None,
            due: None,
            no_due: true,
            labels: vec![],
            add_label: None,
            remove_label: None,
            section: None,
            description: None,
        };

        assert!(opts.no_due);
        assert!(opts.due.is_none());
    }

    #[test]
    fn test_edit_options_label_operations() {
        let opts = EditOptions {
            task_id: "abc123".to_string(),
            content: None,
            project: None,
            priority: None,
            due: None,
            no_due: false,
            labels: vec![],
            add_label: Some("new-label".to_string()),
            remove_label: Some("old-label".to_string()),
            section: None,
            description: None,
        };

        assert!(opts.labels.is_empty());
        assert_eq!(opts.add_label, Some("new-label".to_string()));
        assert_eq!(opts.remove_label, Some("old-label".to_string()));
    }

    #[test]
    fn test_priority_conversion() {
        // User priority 1 (highest) -> API priority 4
        assert_eq!(5 - 1, 4);
        // User priority 4 (lowest) -> API priority 1
        assert_eq!(5 - 4, 1);
    }

    // Note: Tests for item lookup by prefix are now in SyncManager tests
    // (resolve_item_by_prefix covers exact match, prefix match, not found,
    // ambiguous, deleted items, and completion status filtering)
}
