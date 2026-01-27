//! Quick add command implementation.
//!
//! Creates a new task using the Quick Add REST API with server-side NLP parsing.

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::quick_add::{QuickAddRequest, QuickAddResponse};
use todoist_cache_rs::{CacheStore, SyncManager};

use super::{CommandContext, Result};

/// Options for the quick add command.
#[derive(Debug)]
pub struct QuickOptions {
    /// Natural language text to parse.
    pub text: String,
    /// Whether to add default reminder when task has due time.
    pub auto_reminder: bool,
    /// Optional note/comment to attach to the task.
    pub note: Option<String>,
}

/// Result of a successful quick add operation.
#[derive(Debug)]
pub struct QuickResult {
    /// The ID of the created task.
    pub id: String,
    /// The parsed content of the task.
    pub content: String,
    /// The project ID.
    pub project_id: String,
    /// The project name (if resolved).
    pub project_name: Option<String>,
    /// The due date string (if parsed).
    pub due_string: Option<String>,
    /// Priority (user-facing: 1=highest, 4=lowest).
    pub priority: u8,
    /// Labels parsed from input.
    pub labels: Vec<String>,
}

impl QuickResult {
    /// Creates a QuickResult from a QuickAddResponse.
    pub fn from_response(response: QuickAddResponse, project_name: Option<String>) -> Self {
        Self {
            id: response.v2_id.unwrap_or(response.id),
            content: response.content,
            project_id: response.v2_project_id.unwrap_or(response.project_id),
            project_name,
            due_string: response.due.as_ref().and_then(|d| d.string.clone()),
            // Convert API priority (4=highest) to user priority (1=highest)
            priority: (5 - response.priority) as u8,
            labels: response.labels,
        }
    }
}

/// Executes the quick add command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Quick add options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if the API call fails.
pub async fn execute(ctx: &CommandContext, opts: &QuickOptions, token: &str) -> Result<()> {
    let client = TodoistClient::new(token);

    // Build the quick add request
    let mut request =
        QuickAddRequest::new(&opts.text).map_err(todoist_api_rs::error::Error::from)?;

    if opts.auto_reminder {
        request = request.with_auto_reminder(true);
    }

    if let Some(ref note) = opts.note {
        request = request.with_note(note);
    }

    // Execute the quick add
    let response = client.quick_add(request).await?;

    // Try to resolve project name from cache for better output
    let project_name = resolve_project_name(token, &response).await;

    let result = QuickResult::from_response(response, project_name);

    // Output
    if ctx.json_output {
        let output = crate::output::format_quick_add_result(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        if ctx.verbose {
            println!("Created task: {} ({})", result.content, result.id);
            if let Some(ref project_name) = result.project_name {
                println!("  Project: {project_name}");
            }
            if let Some(ref due) = result.due_string {
                println!("  Due: {due}");
            }
            if !result.labels.is_empty() {
                let labels: Vec<String> = result.labels.iter().map(|l| format!("@{l}")).collect();
                println!("  Labels: {}", labels.join(", "));
            }
            if result.priority < 4 {
                println!("  Priority: p{}", result.priority);
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

/// Attempts to resolve the project name from cache.
async fn resolve_project_name(token: &str, response: &QuickAddResponse) -> Option<String> {
    // Try to get from resolved_project_name first
    if let Some(ref name) = response.resolved_project_name {
        return Some(name.clone());
    }

    // Fall back to looking up in cache
    let client = TodoistClient::new(token);
    let store = CacheStore::new().ok()?;
    let manager = SyncManager::new(client, store).ok()?;
    let cache = manager.cache();

    let project_id = response.api_project_id();
    cache
        .projects
        .iter()
        .find(|p| p.id == project_id)
        .map(|p| p.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use todoist_api_rs::models::Due;

    #[test]
    fn test_quick_options_defaults() {
        let opts = QuickOptions {
            text: "Buy milk tomorrow".to_string(),
            auto_reminder: false,
            note: None,
        };

        assert_eq!(opts.text, "Buy milk tomorrow");
        assert!(!opts.auto_reminder);
        assert!(opts.note.is_none());
    }

    #[test]
    fn test_quick_options_with_all_fields() {
        let opts = QuickOptions {
            text: "Meeting at 3pm #Work".to_string(),
            auto_reminder: true,
            note: Some("Bring laptop".to_string()),
        };

        assert_eq!(opts.text, "Meeting at 3pm #Work");
        assert!(opts.auto_reminder);
        assert_eq!(opts.note, Some("Bring laptop".to_string()));
    }

    #[test]
    fn test_quick_result_from_response_minimal() {
        let response = QuickAddResponse {
            id: "old-id".to_string(),
            v2_id: Some("v2-task-123".to_string()),
            project_id: "old-proj".to_string(),
            v2_project_id: Some("v2-proj-456".to_string()),
            content: "Buy groceries".to_string(),
            description: String::new(),
            priority: 1, // API priority (lowest)
            due: None,
            section_id: None,
            parent_id: None,
            child_order: 0,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            resolved_project_name: None,
            resolved_assignee_name: None,
        };

        let result = QuickResult::from_response(response, None);

        assert_eq!(result.id, "v2-task-123");
        assert_eq!(result.content, "Buy groceries");
        assert_eq!(result.project_id, "v2-proj-456");
        assert!(result.project_name.is_none());
        assert!(result.due_string.is_none());
        assert_eq!(result.priority, 4); // User priority (lowest)
        assert!(result.labels.is_empty());
    }

    #[test]
    fn test_quick_result_from_response_full() {
        let response = QuickAddResponse {
            id: "old-id".to_string(),
            v2_id: Some("v2-task-123".to_string()),
            project_id: "old-proj".to_string(),
            v2_project_id: Some("v2-proj-456".to_string()),
            content: "Buy groceries".to_string(),
            description: String::new(),
            priority: 4, // API priority (highest)
            due: Some(Due {
                date: "2026-01-27".to_string(),
                datetime: None,
                string: Some("tomorrow".to_string()),
                timezone: None,
                is_recurring: false,
                lang: None,
            }),
            section_id: None,
            parent_id: None,
            child_order: 0,
            labels: vec!["errands".to_string(), "shopping".to_string()],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            resolved_project_name: Some("Shopping".to_string()),
            resolved_assignee_name: None,
        };

        let result = QuickResult::from_response(response, Some("Shopping".to_string()));

        assert_eq!(result.id, "v2-task-123");
        assert_eq!(result.content, "Buy groceries");
        assert_eq!(result.project_id, "v2-proj-456");
        assert_eq!(result.project_name, Some("Shopping".to_string()));
        assert_eq!(result.due_string, Some("tomorrow".to_string()));
        assert_eq!(result.priority, 1); // User priority (highest)
        assert_eq!(result.labels, vec!["errands", "shopping"]);
    }

    #[test]
    fn test_quick_result_priority_conversion() {
        // API priority 4 (highest) -> User priority 1
        let response = create_test_response(4);
        let result = QuickResult::from_response(response, None);
        assert_eq!(result.priority, 1);

        // API priority 3 -> User priority 2
        let response = create_test_response(3);
        let result = QuickResult::from_response(response, None);
        assert_eq!(result.priority, 2);

        // API priority 2 -> User priority 3
        let response = create_test_response(2);
        let result = QuickResult::from_response(response, None);
        assert_eq!(result.priority, 3);

        // API priority 1 (lowest) -> User priority 4
        let response = create_test_response(1);
        let result = QuickResult::from_response(response, None);
        assert_eq!(result.priority, 4);
    }

    fn create_test_response(priority: i32) -> QuickAddResponse {
        QuickAddResponse {
            id: "test-id".to_string(),
            v2_id: None,
            project_id: "proj-id".to_string(),
            v2_project_id: None,
            content: "Test".to_string(),
            description: String::new(),
            priority,
            due: None,
            section_id: None,
            parent_id: None,
            child_order: 0,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            resolved_project_name: None,
            resolved_assignee_name: None,
        }
    }

    #[test]
    fn test_quick_result_uses_v2_ids() {
        // When v2_id is present, should use it
        let response = QuickAddResponse {
            id: "legacy-id".to_string(),
            v2_id: Some("v2-id".to_string()),
            project_id: "legacy-proj".to_string(),
            v2_project_id: Some("v2-proj".to_string()),
            content: "Test".to_string(),
            description: String::new(),
            priority: 1,
            due: None,
            section_id: None,
            parent_id: None,
            child_order: 0,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            resolved_project_name: None,
            resolved_assignee_name: None,
        };

        let result = QuickResult::from_response(response, None);
        assert_eq!(result.id, "v2-id");
        assert_eq!(result.project_id, "v2-proj");
    }

    #[test]
    fn test_quick_result_falls_back_to_legacy_ids() {
        // When v2_id is absent, should use legacy id
        let response = QuickAddResponse {
            id: "legacy-id".to_string(),
            v2_id: None,
            project_id: "legacy-proj".to_string(),
            v2_project_id: None,
            content: "Test".to_string(),
            description: String::new(),
            priority: 1,
            due: None,
            section_id: None,
            parent_id: None,
            child_order: 0,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            resolved_project_name: None,
            resolved_assignee_name: None,
        };

        let result = QuickResult::from_response(response, None);
        assert_eq!(result.id, "legacy-id");
        assert_eq!(result.project_id, "legacy-proj");
    }
}
