//! Comments command implementation.
//!
//! Lists and manages comments (notes) via the Sync API.
//! Uses SyncManager::execute_commands() to automatically update the cache.

use todoist_api::client::TodoistClient;
use todoist_api::sync::{Note, ProjectNote, SyncCommand};
use todoist_cache::{Cache, CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};
use crate::output::helpers::ID_DISPLAY_LENGTH;
use crate::output::{format_comments_json, format_comments_table};

/// Maximum length for content preview in compact output.
const CONTENT_PREVIEW_LENGTH: usize = 30;

/// Maximum length for content display in verbose output.
const CONTENT_DISPLAY_LENGTH: usize = 40;

/// Number of task matches to preview in ambiguous ID messages.
const TASK_MATCHES_PREVIEW: usize = 3;

/// Number of project matches to preview in ambiguous ID messages.
const PROJECT_MATCHES_PREVIEW: usize = 3;

/// Maximum total matches displayed (task + project previews).
const MAX_DISPLAYED_MATCHES: usize = TASK_MATCHES_PREVIEW + PROJECT_MATCHES_PREVIEW;

/// Options for the comments list command.
#[derive(Debug, Default)]
pub struct CommentsListOptions {
    /// Filter by task ID.
    pub task: Option<String>,
    /// Filter by project ID.
    pub project: Option<String>,
}

/// A unified comment type that can be either a task note or project note.
#[derive(Debug, Clone)]
pub enum Comment {
    /// A task comment (note).
    Task(Note),
    /// A project comment.
    Project(ProjectNote),
}

impl Comment {
    /// Returns the comment ID.
    pub fn id(&self) -> &str {
        match self {
            Comment::Task(n) => &n.id,
            Comment::Project(n) => &n.id,
        }
    }

    /// Returns the comment content.
    pub fn content(&self) -> &str {
        match self {
            Comment::Task(n) => &n.content,
            Comment::Project(n) => &n.content,
        }
    }

    /// Returns when the comment was posted.
    pub fn posted_at(&self) -> Option<&str> {
        match self {
            Comment::Task(n) => n.posted_at.as_deref(),
            Comment::Project(n) => n.posted_at.as_deref(),
        }
    }

    /// Returns the parent ID (task_id or project_id).
    pub fn parent_id(&self) -> &str {
        match self {
            Comment::Task(n) => &n.item_id,
            Comment::Project(n) => &n.project_id,
        }
    }

    /// Returns whether this is a task comment.
    pub fn is_task_comment(&self) -> bool {
        matches!(self, Comment::Task(_))
    }
}

/// Executes the comments list command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Comments list command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails or if neither --task nor --project is specified.
pub async fn execute(ctx: &CommandContext, opts: &CommentsListOptions, token: &str) -> Result<()> {
    // Require at least one of --task or --project
    if opts.task.is_none() && opts.project.is_none() {
        return Err(CommandError::Config(
            "Either --task or --project is required to list comments.".to_string(),
        ));
    }

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

    // Resolve task filter if provided
    let task_id = if let Some(ref task) = opts.task {
        Some(resolve_task_id(cache, task)?)
    } else {
        None
    };

    // Resolve project filter if provided
    let project_id = if let Some(ref project) = opts.project {
        Some(resolve_project_id(cache, project)?)
    } else {
        None
    };

    // Get comments
    let comments = filter_comments(cache, task_id.as_deref(), project_id.as_deref());

    // Get parent name for display
    let parent_name = if let Some(ref tid) = task_id {
        cache
            .items
            .iter()
            .find(|i| i.id == *tid)
            .map(|i| i.content.clone())
    } else if let Some(ref pid) = project_id {
        cache
            .projects
            .iter()
            .find(|p| p.id == *pid)
            .map(|p| p.name.clone())
    } else {
        None
    };

    // Output
    if ctx.json_output {
        let output = format_comments_json(&comments, cache)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_comments_table(&comments, parent_name.as_deref(), ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

/// Resolves a task name/ID to a task ID.
fn resolve_task_id(cache: &Cache, task: &str) -> Result<String> {
    // First try exact ID match
    if let Some(i) = cache.items.iter().find(|i| i.id == task && !i.is_deleted) {
        return Ok(i.id.clone());
    }

    // Try ID prefix match (6+ chars)
    if task.len() >= 6 {
        let prefix_matches: Vec<_> = cache
            .items
            .iter()
            .filter(|i| i.id.starts_with(task) && !i.is_deleted)
            .collect();

        if prefix_matches.len() == 1 {
            return Ok(prefix_matches[0].id.clone());
        }

        if prefix_matches.len() > 1 {
            let mut msg = format!(
                "Ambiguous task ID \"{task}\"\n\nMultiple tasks match this prefix:"
            );
            for item in prefix_matches.iter().take(5) {
                let prefix = &item.id[..6.min(item.id.len())];
                msg.push_str(&format!("\n  {}  {}", prefix, item.content));
            }
            if prefix_matches.len() > 5 {
                msg.push_str(&format!("\n  ... and {} more", prefix_matches.len() - 5));
            }
            msg.push_str("\n\nPlease use a longer prefix.");
            return Err(CommandError::Config(msg));
        }
    }

    Err(CommandError::Config(format!("Task not found: {task}")))
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
        Err(CommandError::Config(format!("Project not found: {project}")))
    }
}

// ============================================================================
// Comments Add Command
// ============================================================================

/// Options for the comments add command.
#[derive(Debug)]
pub struct CommentsAddOptions {
    /// Comment text content.
    pub content: String,
    /// Task ID (for task comment).
    pub task: Option<String>,
    /// Project ID (for project comment).
    pub project: Option<String>,
}

/// Result of a successful comment add operation.
#[derive(Debug)]
pub struct CommentAddResult {
    /// The real ID of the created comment.
    pub id: String,
    /// The content of the comment.
    pub content: String,
    /// Whether this is a task comment (vs project comment).
    pub is_task_comment: bool,
    /// The parent ID (task_id or project_id).
    pub parent_id: String,
    /// The parent name (task content or project name).
    pub parent_name: Option<String>,
}

/// Executes the comments add command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Comments add command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if neither --task nor --project is specified,
/// or if the API returns an error.
pub async fn execute_add(
    ctx: &CommandContext,
    opts: &CommentsAddOptions,
    token: &str,
) -> Result<()> {
    // Require exactly one of --task or --project
    if opts.task.is_none() && opts.project.is_none() {
        return Err(CommandError::Config(
            "Either --task or --project is required to add a comment.".to_string(),
        ));
    }
    if opts.task.is_some() && opts.project.is_some() {
        return Err(CommandError::Config(
            "Cannot specify both --task and --project. Choose one.".to_string(),
        ));
    }

    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Resolve task/project ID and get parent name before mutation
    let (is_task_comment, parent_id, parent_name) = {
        let cache = manager.cache();
        if let Some(ref task) = opts.task {
            let task_id = resolve_task_id(cache, task)?;
            let task_name = cache
                .items
                .iter()
                .find(|i| i.id == task_id)
                .map(|i| i.content.clone());
            (true, task_id, task_name)
        } else if let Some(ref project) = opts.project {
            let project_id = resolve_project_id(cache, project)?;
            let project_name = cache
                .projects
                .iter()
                .find(|p| p.id == project_id)
                .map(|p| p.name.clone());
            (false, project_id, project_name)
        } else {
            unreachable!("Already validated that one of task or project is provided");
        }
    };

    // Build the note_add command arguments
    // Both task and project comments use note_add, but with different parent ID field
    let temp_id = uuid::Uuid::new_v4().to_string();
    let args = if is_task_comment {
        serde_json::json!({
            "item_id": parent_id,
            "content": opts.content,
        })
    } else {
        serde_json::json!({
            "project_id": parent_id,
            "content": opts.content,
        })
    };

    // Create the command
    let command = SyncCommand::with_temp_id("note_add", &temp_id, args);

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
            CommandError::Config("Comment created but no ID returned in response".to_string())
        })?
        .clone();

    let result = CommentAddResult {
        id: real_id,
        content: opts.content.clone(),
        is_task_comment,
        parent_id,
        parent_name,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_created_comment(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        let parent_type = if result.is_task_comment { "task" } else { "project" };
        let parent_display = result
            .parent_name
            .as_deref()
            .unwrap_or(&result.parent_id);
        if ctx.verbose {
            println!("Created comment: {}", result.id);
            println!("  On {}: {}", parent_type, parent_display);
            println!("  Content: {}", result.content);
        } else {
            let prefix = &result.id[..ID_DISPLAY_LENGTH.min(result.id.len())];
            // Truncate content for display
            let content_display = if result.content.len() > CONTENT_DISPLAY_LENGTH {
                format!("{}...", &result.content[..CONTENT_DISPLAY_LENGTH - 3])
            } else {
                result.content.clone()
            };
            println!(
                "Added comment ({}) to {}: {}",
                prefix, parent_display, content_display
            );
        }
    }

    Ok(())
}

// ============================================================================
// Comments Edit Command
// ============================================================================

/// Options for the comments edit command.
#[derive(Debug)]
pub struct CommentsEditOptions {
    /// Comment ID to edit.
    pub comment_id: String,
    /// New content for the comment.
    pub content: String,
}

/// Result of a successful comment edit operation.
#[derive(Debug)]
pub struct CommentEditResult {
    /// The ID of the edited comment.
    pub id: String,
    /// The new content of the comment.
    pub content: String,
    /// Whether this is a task comment (vs project comment).
    pub is_task_comment: bool,
    /// The parent ID (task_id or project_id).
    pub parent_id: String,
    /// The parent name (task content or project name).
    pub parent_name: Option<String>,
}

/// Executes the comments edit command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Comments edit command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if the comment is not found or the API returns an error.
pub async fn execute_edit(
    ctx: &CommandContext,
    opts: &CommentsEditOptions,
    token: &str,
) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the comment by ID and extract owned data before mutation
    let (comment_id, is_task_comment, parent_id, parent_name) = {
        let cache = manager.cache();
        resolve_comment(cache, &opts.comment_id)?
    };

    // Build the note_update command
    let args = serde_json::json!({
        "id": comment_id,
        "content": opts.content,
    });

    let command = SyncCommand::new("note_update", args);

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

    let result = CommentEditResult {
        id: comment_id,
        content: opts.content.clone(),
        is_task_comment,
        parent_id,
        parent_name,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_edited_comment(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        let parent_type = if result.is_task_comment {
            "task"
        } else {
            "project"
        };
        let parent_display = result.parent_name.as_deref().unwrap_or(&result.parent_id);
        if ctx.verbose {
            println!("Updated comment: {}", result.id);
            println!("  On {}: {}", parent_type, parent_display);
            println!("  Content: {}", result.content);
        } else {
            let prefix = &result.id[..ID_DISPLAY_LENGTH.min(result.id.len())];
            // Truncate content for display
            let content_display = if result.content.len() > CONTENT_DISPLAY_LENGTH {
                format!("{}...", &result.content[..CONTENT_DISPLAY_LENGTH - 3])
            } else {
                result.content.clone()
            };
            println!("Updated comment ({}) on {}: {}", prefix, parent_display, content_display);
        }
    }

    Ok(())
}

// ============================================================================
// Comments Delete Command
// ============================================================================

/// Options for the comments delete command.
#[derive(Debug)]
pub struct CommentsDeleteOptions {
    /// Comment ID to delete.
    pub comment_id: String,
    /// Skip confirmation.
    pub force: bool,
}

/// Result of a successful comment delete operation.
#[derive(Debug)]
pub struct CommentDeleteResult {
    /// The ID of the deleted comment.
    pub id: String,
    /// The content of the deleted comment (truncated for display).
    pub content: String,
    /// Whether this was a task comment (vs project comment).
    pub is_task_comment: bool,
    /// The parent ID (task_id or project_id).
    pub parent_id: String,
    /// The parent name (task content or project name).
    pub parent_name: Option<String>,
}

/// Executes the comments delete command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Comments delete command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if the comment is not found or the API returns an error.
pub async fn execute_delete(
    ctx: &CommandContext,
    opts: &CommentsDeleteOptions,
    token: &str,
) -> Result<()> {
    // Initialize sync manager (loads cache from disk)
    let client = TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Find the comment by ID and extract owned data before mutation
    let (comment_id, is_task_comment, parent_id, parent_name, content_preview) = {
        let cache = manager.cache();
        let (c_id, is_task, p_id, p_name) = resolve_comment(cache, &opts.comment_id)?;
        let comment_content = get_comment_content(cache, &c_id);
        let preview = if comment_content.len() > CONTENT_DISPLAY_LENGTH {
            format!("{}...", &comment_content[..CONTENT_DISPLAY_LENGTH - 3])
        } else {
            comment_content
        };
        (c_id, is_task, p_id, p_name, preview)
    };

    // Confirm if not forced
    if !opts.force && !ctx.quiet {
        let parent_type = if is_task_comment { "task" } else { "project" };
        let parent_display = parent_name.as_deref().unwrap_or(&parent_id);
        eprintln!(
            "Delete comment ({}) on {} '{}'?",
            &comment_id[..ID_DISPLAY_LENGTH.min(comment_id.len())],
            parent_type,
            parent_display
        );
        eprintln!("  Content: {}", content_preview);
        eprintln!("Use --force to skip this confirmation.");
        return Err(CommandError::Config(
            "Operation cancelled. Use --force to confirm.".to_string(),
        ));
    }

    // Build the note_delete command
    let args = serde_json::json!({
        "id": comment_id,
    });

    let command = SyncCommand::new("note_delete", args);

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

    let result = CommentDeleteResult {
        id: comment_id,
        content: content_preview,
        is_task_comment,
        parent_id,
        parent_name,
    };

    // Output
    if ctx.json_output {
        let output = crate::output::format_deleted_comment(&result)?;
        println!("{output}");
    } else if !ctx.quiet {
        let parent_type = if result.is_task_comment {
            "task"
        } else {
            "project"
        };
        let parent_display = result.parent_name.as_deref().unwrap_or(&result.parent_id);
        if ctx.verbose {
            println!("Deleted comment: {}", result.id);
            println!("  On {}: {}", parent_type, parent_display);
            println!("  Content: {}", result.content);
        } else {
            let prefix = &result.id[..6.min(result.id.len())];
            println!("Deleted comment ({}) from {}", prefix, parent_display);
        }
    }

    Ok(())
}

/// Gets the content of a comment by ID.
fn get_comment_content(cache: &Cache, comment_id: &str) -> String {
    // Check task notes
    if let Some(note) = cache.notes.iter().find(|n| n.id == comment_id) {
        return note.content.clone();
    }

    // Check project notes
    if let Some(note) = cache.project_notes.iter().find(|n| n.id == comment_id) {
        return note.content.clone();
    }

    String::new()
}

/// Resolves a comment ID from either task notes or project notes.
/// Returns (full_id, is_task_comment, parent_id, parent_name).
fn resolve_comment(
    cache: &Cache,
    comment_id: &str,
) -> Result<(String, bool, String, Option<String>)> {
    // First try exact ID match in task notes
    for note in &cache.notes {
        if note.id == comment_id && !note.is_deleted {
            let parent_name = cache
                .items
                .iter()
                .find(|i| i.id == note.item_id && !i.is_deleted)
                .map(|i| i.content.clone());
            return Ok((note.id.clone(), true, note.item_id.clone(), parent_name));
        }
    }

    // Try exact ID match in project notes
    for note in &cache.project_notes {
        if note.id == comment_id && !note.is_deleted {
            let parent_name = cache
                .projects
                .iter()
                .find(|p| p.id == note.project_id && !p.is_deleted)
                .map(|p| p.name.clone());
            return Ok((note.id.clone(), false, note.project_id.clone(), parent_name));
        }
    }

    // Try ID prefix match (6+ chars) in task notes
    if comment_id.len() >= 6 {
        let task_note_matches: Vec<_> = cache
            .notes
            .iter()
            .filter(|n| n.id.starts_with(comment_id) && !n.is_deleted)
            .collect();

        let project_note_matches: Vec<_> = cache
            .project_notes
            .iter()
            .filter(|n| n.id.starts_with(comment_id) && !n.is_deleted)
            .collect();

        let total_matches = task_note_matches.len() + project_note_matches.len();

        if total_matches == 1 {
            if let Some(note) = task_note_matches.first() {
                let parent_name = cache
                    .items
                    .iter()
                    .find(|i| i.id == note.item_id && !i.is_deleted)
                    .map(|i| i.content.clone());
                return Ok((note.id.clone(), true, note.item_id.clone(), parent_name));
            }
            if let Some(note) = project_note_matches.first() {
                let parent_name = cache
                    .projects
                    .iter()
                    .find(|p| p.id == note.project_id && !p.is_deleted)
                    .map(|p| p.name.clone());
                return Ok((note.id.clone(), false, note.project_id.clone(), parent_name));
            }
        }

        if total_matches > 1 {
            let mut msg =
                format!("Ambiguous comment ID \"{comment_id}\"\n\nMultiple comments match this prefix:");
            for note in task_note_matches.iter().take(TASK_MATCHES_PREVIEW) {
                let prefix = &note.id[..ID_DISPLAY_LENGTH.min(note.id.len())];
                let content_preview = if note.content.len() > CONTENT_PREVIEW_LENGTH {
                    format!("{}...", &note.content[..CONTENT_PREVIEW_LENGTH - 3])
                } else {
                    note.content.clone()
                };
                msg.push_str(&format!("\n  {} (task): {}", prefix, content_preview));
            }
            for note in project_note_matches.iter().take(PROJECT_MATCHES_PREVIEW) {
                let prefix = &note.id[..ID_DISPLAY_LENGTH.min(note.id.len())];
                let content_preview = if note.content.len() > CONTENT_PREVIEW_LENGTH {
                    format!("{}...", &note.content[..CONTENT_PREVIEW_LENGTH - 3])
                } else {
                    note.content.clone()
                };
                msg.push_str(&format!("\n  {} (project): {}", prefix, content_preview));
            }
            if total_matches > MAX_DISPLAYED_MATCHES {
                msg.push_str(&format!(
                    "\n  ... and {} more",
                    total_matches - MAX_DISPLAYED_MATCHES
                ));
            }
            msg.push_str("\n\nPlease use a longer prefix.");
            return Err(CommandError::Config(msg));
        }
    }

    Err(CommandError::Config(format!(
        "Comment not found: {comment_id}"
    )))
}

/// Filters comments based on task_id or project_id.
fn filter_comments(
    cache: &Cache,
    task_id: Option<&str>,
    project_id: Option<&str>,
) -> Vec<Comment> {
    let mut comments = Vec::new();

    // Get task comments if filtering by task
    if let Some(tid) = task_id {
        for note in &cache.notes {
            if !note.is_deleted && note.item_id == tid {
                comments.push(Comment::Task(note.clone()));
            }
        }
    }

    // Get project comments if filtering by project
    if let Some(pid) = project_id {
        for note in &cache.project_notes {
            if !note.is_deleted && note.project_id == pid {
                comments.push(Comment::Project(note.clone()));
            }
        }
    }

    // Sort by posted_at (newest first)
    comments.sort_by(|a, b| {
        let a_time = a.posted_at().unwrap_or("");
        let b_time = b.posted_at().unwrap_or("");
        b_time.cmp(a_time)
    });

    comments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comments_list_options_defaults() {
        let opts = CommentsListOptions::default();

        assert!(opts.task.is_none());
        assert!(opts.project.is_none());
    }

    #[test]
    fn test_comments_add_options_with_task() {
        let opts = CommentsAddOptions {
            content: "My comment".to_string(),
            task: Some("task-123".to_string()),
            project: None,
        };

        assert_eq!(opts.content, "My comment");
        assert_eq!(opts.task, Some("task-123".to_string()));
        assert!(opts.project.is_none());
    }

    #[test]
    fn test_comments_add_options_with_project() {
        let opts = CommentsAddOptions {
            content: "Project comment".to_string(),
            task: None,
            project: Some("project-456".to_string()),
        };

        assert_eq!(opts.content, "Project comment");
        assert!(opts.task.is_none());
        assert_eq!(opts.project, Some("project-456".to_string()));
    }

    #[test]
    fn test_comment_add_result_task() {
        let result = CommentAddResult {
            id: "note-123".to_string(),
            content: "Test comment".to_string(),
            is_task_comment: true,
            parent_id: "task-1".to_string(),
            parent_name: Some("My Task".to_string()),
        };

        assert_eq!(result.id, "note-123");
        assert_eq!(result.content, "Test comment");
        assert!(result.is_task_comment);
        assert_eq!(result.parent_id, "task-1");
        assert_eq!(result.parent_name, Some("My Task".to_string()));
    }

    #[test]
    fn test_comment_add_result_project() {
        let result = CommentAddResult {
            id: "pnote-456".to_string(),
            content: "Project comment".to_string(),
            is_task_comment: false,
            parent_id: "project-1".to_string(),
            parent_name: Some("My Project".to_string()),
        };

        assert_eq!(result.id, "pnote-456");
        assert_eq!(result.content, "Project comment");
        assert!(!result.is_task_comment);
        assert_eq!(result.parent_id, "project-1");
        assert_eq!(result.parent_name, Some("My Project".to_string()));
    }

    #[test]
    fn test_comments_list_options_with_task() {
        let opts = CommentsListOptions {
            task: Some("task-123".to_string()),
            project: None,
        };

        assert_eq!(opts.task, Some("task-123".to_string()));
        assert!(opts.project.is_none());
    }

    #[test]
    fn test_comments_list_options_with_project() {
        let opts = CommentsListOptions {
            task: None,
            project: Some("project-456".to_string()),
        };

        assert!(opts.task.is_none());
        assert_eq!(opts.project, Some("project-456".to_string()));
    }

    #[test]
    fn test_comment_task_variant() {
        let note = Note {
            id: "note-1".to_string(),
            item_id: "task-1".to_string(),
            content: "Test comment".to_string(),
            posted_at: Some("2025-01-26T10:00:00Z".to_string()),
            is_deleted: false,
            posted_uid: None,
            file_attachment: None,
        };
        let comment = Comment::Task(note);

        assert_eq!(comment.id(), "note-1");
        assert_eq!(comment.content(), "Test comment");
        assert_eq!(comment.posted_at(), Some("2025-01-26T10:00:00Z"));
        assert_eq!(comment.parent_id(), "task-1");
        assert!(comment.is_task_comment());
    }

    #[test]
    fn test_comment_project_variant() {
        let note = ProjectNote {
            id: "pnote-1".to_string(),
            project_id: "project-1".to_string(),
            content: "Project comment".to_string(),
            posted_at: Some("2025-01-26T11:00:00Z".to_string()),
            is_deleted: false,
            posted_uid: None,
            file_attachment: None,
        };
        let comment = Comment::Project(note);

        assert_eq!(comment.id(), "pnote-1");
        assert_eq!(comment.content(), "Project comment");
        assert_eq!(comment.posted_at(), Some("2025-01-26T11:00:00Z"));
        assert_eq!(comment.parent_id(), "project-1");
        assert!(!comment.is_task_comment());
    }

    #[test]
    fn test_filter_comments_by_task() {
        let cache = make_test_cache();
        let comments = filter_comments(&cache, Some("task-1"), None);

        assert_eq!(comments.len(), 2);
        // Should be sorted newest first
        assert_eq!(comments[0].content(), "Second comment");
        assert_eq!(comments[1].content(), "First comment");
    }

    #[test]
    fn test_filter_comments_by_project() {
        let cache = make_test_cache();
        let comments = filter_comments(&cache, None, Some("project-1"));

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].content(), "Project comment");
    }

    #[test]
    fn test_filter_comments_excludes_deleted() {
        let mut cache = make_test_cache();
        cache.notes[0].is_deleted = true;

        let comments = filter_comments(&cache, Some("task-1"), None);

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].content(), "Second comment");
    }

    #[test]
    fn test_resolve_task_id_exact_match() {
        let cache = make_test_cache();
        let result = resolve_task_id(&cache, "task-1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "task-1");
    }

    #[test]
    fn test_resolve_task_id_not_found() {
        let cache = make_test_cache();
        let result = resolve_task_id(&cache, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Task not found"));
    }

    #[test]
    fn test_resolve_project_id_exact_match() {
        let cache = make_test_cache();
        let result = resolve_project_id(&cache, "project-1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "project-1");
    }

    #[test]
    fn test_resolve_project_id_by_name() {
        let cache = make_test_cache();
        let result = resolve_project_id(&cache, "Test Project");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "project-1");
    }

    #[test]
    fn test_resolve_project_id_not_found() {
        let cache = make_test_cache();
        let result = resolve_project_id(&cache, "nonexistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Project not found"));
    }

    // Helper function to create a test cache
    fn make_test_cache() -> Cache {
        Cache {
            sync_token: "test".to_string(),
            full_sync_date_utc: None,
            last_sync: None,
            items: vec![make_test_item("task-1", "Test Task", "project-1")],
            projects: vec![make_test_project("project-1", "Test Project")],
            labels: vec![],
            sections: vec![],
            notes: vec![
                Note {
                    id: "note-1".to_string(),
                    item_id: "task-1".to_string(),
                    content: "First comment".to_string(),
                    posted_at: Some("2025-01-26T10:00:00Z".to_string()),
                    is_deleted: false,
                    posted_uid: None,
                    file_attachment: None,
                },
                Note {
                    id: "note-2".to_string(),
                    item_id: "task-1".to_string(),
                    content: "Second comment".to_string(),
                    posted_at: Some("2025-01-26T11:00:00Z".to_string()),
                    is_deleted: false,
                    posted_uid: None,
                    file_attachment: None,
                },
            ],
            project_notes: vec![ProjectNote {
                id: "pnote-1".to_string(),
                project_id: "project-1".to_string(),
                content: "Project comment".to_string(),
                posted_at: Some("2025-01-26T12:00:00Z".to_string()),
                is_deleted: false,
                posted_uid: None,
                file_attachment: None,
            }],
            reminders: vec![],
            filters: vec![],
            user: None,
        }
    }

    fn make_test_item(id: &str, content: &str, project_id: &str) -> todoist_api::sync::Item {
        todoist_api::sync::Item {
            id: id.to_string(),
            user_id: None,
            project_id: project_id.to_string(),
            content: content.to_string(),
            description: String::new(),
            priority: 1,
            due: None,
            deadline: None,
            parent_id: None,
            child_order: 0,
            section_id: None,
            day_order: 0,
            is_collapsed: false,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            updated_at: None,
            completed_at: None,
            duration: None,
        }
    }

    fn make_test_project(id: &str, name: &str) -> todoist_api::sync::Project {
        todoist_api::sync::Project {
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

    // ========================================================================
    // Comments Edit Tests
    // ========================================================================

    #[test]
    fn test_comments_edit_options() {
        let opts = CommentsEditOptions {
            comment_id: "note-123".to_string(),
            content: "Updated content".to_string(),
        };

        assert_eq!(opts.comment_id, "note-123");
        assert_eq!(opts.content, "Updated content");
    }

    #[test]
    fn test_comment_edit_result_task() {
        let result = CommentEditResult {
            id: "note-123".to_string(),
            content: "Updated comment".to_string(),
            is_task_comment: true,
            parent_id: "task-1".to_string(),
            parent_name: Some("My Task".to_string()),
        };

        assert_eq!(result.id, "note-123");
        assert_eq!(result.content, "Updated comment");
        assert!(result.is_task_comment);
        assert_eq!(result.parent_id, "task-1");
        assert_eq!(result.parent_name, Some("My Task".to_string()));
    }

    #[test]
    fn test_comment_edit_result_project() {
        let result = CommentEditResult {
            id: "pnote-456".to_string(),
            content: "Updated project comment".to_string(),
            is_task_comment: false,
            parent_id: "project-1".to_string(),
            parent_name: Some("My Project".to_string()),
        };

        assert_eq!(result.id, "pnote-456");
        assert_eq!(result.content, "Updated project comment");
        assert!(!result.is_task_comment);
        assert_eq!(result.parent_id, "project-1");
        assert_eq!(result.parent_name, Some("My Project".to_string()));
    }

    #[test]
    fn test_resolve_comment_exact_task_note() {
        let cache = make_test_cache();
        let result = resolve_comment(&cache, "note-1");
        assert!(result.is_ok());
        let (id, is_task, parent_id, parent_name) = result.unwrap();
        assert_eq!(id, "note-1");
        assert!(is_task);
        assert_eq!(parent_id, "task-1");
        assert_eq!(parent_name, Some("Test Task".to_string()));
    }

    #[test]
    fn test_resolve_comment_exact_project_note() {
        let cache = make_test_cache();
        let result = resolve_comment(&cache, "pnote-1");
        assert!(result.is_ok());
        let (id, is_task, parent_id, parent_name) = result.unwrap();
        assert_eq!(id, "pnote-1");
        assert!(!is_task);
        assert_eq!(parent_id, "project-1");
        assert_eq!(parent_name, Some("Test Project".to_string()));
    }

    #[test]
    fn test_resolve_comment_not_found() {
        let cache = make_test_cache();
        let result = resolve_comment(&cache, "nonexistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Comment not found"));
    }

    #[test]
    fn test_resolve_comment_excludes_deleted() {
        let mut cache = make_test_cache();
        cache.notes[0].is_deleted = true;

        let result = resolve_comment(&cache, "note-1");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Comment not found"));
    }

    #[test]
    fn test_resolve_comment_prefix_match() {
        let mut cache = make_test_cache();
        // Add a note with a longer ID for prefix matching
        cache.notes.push(Note {
            id: "note-abc123def456".to_string(),
            item_id: "task-1".to_string(),
            content: "Prefixed comment".to_string(),
            posted_at: Some("2025-01-26T13:00:00Z".to_string()),
            is_deleted: false,
            posted_uid: None,
            file_attachment: None,
        });

        // Should match with 6+ char prefix
        let result = resolve_comment(&cache, "note-a");
        assert!(result.is_ok());
        let (id, is_task, _, _) = result.unwrap();
        assert_eq!(id, "note-abc123def456");
        assert!(is_task);
    }

    // ========================================================================
    // Comments Delete Tests
    // ========================================================================

    #[test]
    fn test_comments_delete_options() {
        let opts = CommentsDeleteOptions {
            comment_id: "note-123".to_string(),
            force: false,
        };

        assert_eq!(opts.comment_id, "note-123");
        assert!(!opts.force);
    }

    #[test]
    fn test_comments_delete_options_with_force() {
        let opts = CommentsDeleteOptions {
            comment_id: "note-456".to_string(),
            force: true,
        };

        assert_eq!(opts.comment_id, "note-456");
        assert!(opts.force);
    }

    #[test]
    fn test_comment_delete_result_task() {
        let result = CommentDeleteResult {
            id: "note-123".to_string(),
            content: "Deleted comment".to_string(),
            is_task_comment: true,
            parent_id: "task-1".to_string(),
            parent_name: Some("My Task".to_string()),
        };

        assert_eq!(result.id, "note-123");
        assert_eq!(result.content, "Deleted comment");
        assert!(result.is_task_comment);
        assert_eq!(result.parent_id, "task-1");
        assert_eq!(result.parent_name, Some("My Task".to_string()));
    }

    #[test]
    fn test_comment_delete_result_project() {
        let result = CommentDeleteResult {
            id: "pnote-456".to_string(),
            content: "Deleted project comment".to_string(),
            is_task_comment: false,
            parent_id: "project-1".to_string(),
            parent_name: Some("My Project".to_string()),
        };

        assert_eq!(result.id, "pnote-456");
        assert_eq!(result.content, "Deleted project comment");
        assert!(!result.is_task_comment);
        assert_eq!(result.parent_id, "project-1");
        assert_eq!(result.parent_name, Some("My Project".to_string()));
    }

    #[test]
    fn test_get_comment_content_task_note() {
        let cache = make_test_cache();
        let content = get_comment_content(&cache, "note-1");
        assert_eq!(content, "First comment");
    }

    #[test]
    fn test_get_comment_content_project_note() {
        let cache = make_test_cache();
        let content = get_comment_content(&cache, "pnote-1");
        assert_eq!(content, "Project comment");
    }

    #[test]
    fn test_get_comment_content_not_found() {
        let cache = make_test_cache();
        let content = get_comment_content(&cache, "nonexistent");
        assert_eq!(content, "");
    }
}
