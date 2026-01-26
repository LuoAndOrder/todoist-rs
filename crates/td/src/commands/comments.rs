//! Comments command implementation.
//!
//! Lists and manages comments (notes) via the Sync API.

use chrono::Utc;
use todoist_api::client::TodoistClient;
use todoist_api::sync::{Note, ProjectNote};
use todoist_cache::{Cache, CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};
use crate::output::{format_comments_json, format_comments_table};

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

    // Sync if needed
    let now = Utc::now();
    if manager.needs_sync(now) {
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
}
