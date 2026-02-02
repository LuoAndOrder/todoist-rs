//! Add command implementation.
//!
//! Creates a new task via the Sync API's `item_add` command.
//! Uses SyncManager::execute_commands() to automatically update the cache.

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::SyncCommand;
use todoist_cache_rs::{CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};
use crate::output::format_created_item;

/// Options for the add command.
#[derive(Debug)]
pub struct AddOptions {
    /// Task content/title.
    pub content: String,
    /// Target project (name or ID).
    pub project: Option<String>,
    /// Priority level (1=highest, 4=lowest).
    pub priority: Option<u8>,
    /// Due date (natural language or ISO).
    pub due: Option<String>,
    /// Labels to attach.
    pub labels: Vec<String>,
    /// Target section within project.
    pub section: Option<String>,
    /// Parent task ID (creates subtask).
    pub parent: Option<String>,
    /// Task description/notes.
    pub description: Option<String>,
}

/// Result of a successful add operation.
#[derive(Debug)]
pub struct AddResult {
    /// The real ID of the created task.
    pub id: String,
    /// The content of the created task.
    pub content: String,
    /// The project ID.
    pub project_id: String,
    /// The project name (if found in cache).
    pub project_name: Option<String>,
}

/// Executes the add command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Add command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if project/section lookup fails or the API returns an error.
pub async fn execute(ctx: &CommandContext, opts: &AddOptions, token: &str) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Resolve project name to ID using smart lookup (cache-first with auto-sync fallback)
    let project_id = if let Some(ref project_name) = opts.project {
        manager.resolve_project(project_name).await?.id.clone()
    } else {
        // Use inbox project if no project specified
        manager
            .cache()
            .projects
            .iter()
            .find(|p| p.inbox_project && !p.is_deleted)
            .map(|p| p.id.clone())
            .ok_or_else(|| CommandError::Config("Inbox project not found".to_string()))?
    };

    // Resolve section name to ID using smart lookup (cache-first with auto-sync fallback)
    let section_id = if let Some(ref section_name) = opts.section {
        Some(
            manager
                .resolve_section(section_name, Some(&project_id))
                .await?
                .id
                .clone(),
        )
    } else {
        None
    };

    // Build the item_add command arguments
    let temp_id = uuid::Uuid::new_v4().to_string();
    let mut args = serde_json::json!({
        "content": opts.content,
        "project_id": project_id,
    });

    // Add optional fields
    if let Some(ref description) = opts.description {
        args["description"] = serde_json::json!(description);
    }

    if let Some(priority) = opts.priority {
        // Convert user priority (1=highest) to API priority (4=highest)
        let api_priority = 5 - priority as i32;
        args["priority"] = serde_json::json!(api_priority);
    }

    if let Some(ref due) = opts.due {
        // Use the "string" field to let Todoist parse natural language dates
        args["due"] = serde_json::json!({"string": due});
    }

    if !opts.labels.is_empty() {
        args["labels"] = serde_json::json!(opts.labels);
    }

    if let Some(ref section_id) = section_id {
        args["section_id"] = serde_json::json!(section_id);
    }

    if let Some(ref parent_id) = opts.parent {
        args["parent_id"] = serde_json::json!(parent_id);
    }

    // Create and execute the command via SyncManager
    // This sends the command, applies the response to cache, and saves to disk
    let command = SyncCommand::with_temp_id("item_add", &temp_id, args);
    let response = manager.execute_commands(vec![command]).await?;

    // Check for command errors in the response
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
            CommandError::Config("Task created but no ID returned in response".to_string())
        })?
        .clone();

    // Get project name for output from the updated cache
    let project_name = manager
        .cache()
        .projects
        .iter()
        .find(|p| p.id == project_id)
        .map(|p| p.name.clone());

    let result = AddResult {
        id: real_id,
        content: opts.content.clone(),
        project_id,
        project_name,
    };

    // Output
    if ctx.json_output {
        let output = format_created_item(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Created task: {} ({})", result.content, result.id);
            if let Some(ref project_name) = result.project_name {
                println!("  Project: {project_name}");
            }
            if let Some(ref due) = opts.due {
                println!("  Due: {due}");
            }
            if !opts.labels.is_empty() {
                println!("  Labels: {}", opts.labels.join(", "));
            }
        } else {
            println!(
                "Created: {} ({})",
                result.content,
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
    fn test_add_options_defaults() {
        let opts = AddOptions {
            content: "Test task".to_string(),
            project: None,
            priority: None,
            due: None,
            labels: vec![],
            section: None,
            parent: None,
            description: None,
        };

        assert_eq!(opts.content, "Test task");
        assert!(opts.project.is_none());
        assert!(opts.labels.is_empty());
    }

    #[test]
    fn test_add_options_with_all_fields() {
        let opts = AddOptions {
            content: "Test task".to_string(),
            project: Some("Work".to_string()),
            priority: Some(1),
            due: Some("tomorrow".to_string()),
            labels: vec!["urgent".to_string(), "important".to_string()],
            section: Some("In Progress".to_string()),
            parent: Some("parent-123".to_string()),
            description: Some("Task description".to_string()),
        };

        assert_eq!(opts.content, "Test task");
        assert_eq!(opts.project, Some("Work".to_string()));
        assert_eq!(opts.priority, Some(1));
        assert_eq!(opts.due, Some("tomorrow".to_string()));
        assert_eq!(opts.labels.len(), 2);
        assert_eq!(opts.section, Some("In Progress".to_string()));
        assert_eq!(opts.parent, Some("parent-123".to_string()));
        assert_eq!(opts.description, Some("Task description".to_string()));
    }

    #[test]
    fn test_priority_conversion() {
        // User priority 1 (highest) -> API priority 4
        assert_eq!(5 - 1, 4);
        // User priority 4 (lowest) -> API priority 1
        assert_eq!(5 - 4, 1);
    }
}
