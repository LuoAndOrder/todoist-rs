//! Comment output formatting.

use owo_colors::OwoColorize;
use serde::Serialize;
use todoist_cache_rs::Cache;

use crate::commands::comments::{Comment, CommentAddResult, CommentDeleteResult, CommentEditResult};

use super::helpers::{format_datetime, truncate_id, truncate_str};

/// JSON output structure for comments list command.
#[derive(Serialize)]
pub struct CommentsListOutput<'a> {
    pub comments: Vec<CommentListOutput<'a>>,
}

/// JSON output structure for a single comment in list.
#[derive(Serialize)]
pub struct CommentListOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<&'a str>,
    pub parent_id: &'a str,
    pub parent_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<&'a str>,
}

/// Formats comments as JSON.
pub fn format_comments_json(
    comments: &[Comment],
    cache: &Cache,
) -> Result<String, serde_json::Error> {
    let comments_output: Vec<CommentListOutput> = comments
        .iter()
        .map(|c| {
            let (parent_type, parent_name) = if c.is_task_comment() {
                let task_name = cache
                    .items
                    .iter()
                    .find(|i| i.id == c.parent_id())
                    .map(|i| i.content.as_str());
                ("task", task_name)
            } else {
                let project_name = cache
                    .projects
                    .iter()
                    .find(|p| p.id == c.parent_id())
                    .map(|p| p.name.as_str());
                ("project", project_name)
            };

            CommentListOutput {
                id: c.id(),
                content: c.content(),
                posted_at: c.posted_at(),
                parent_id: c.parent_id(),
                parent_type,
                parent_name,
            }
        })
        .collect();

    let output = CommentsListOutput {
        comments: comments_output,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a created comment.
#[derive(Serialize)]
pub struct CreatedCommentOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub parent_id: &'a str,
    pub parent_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<&'a str>,
}

/// Formats a created comment as JSON.
pub fn format_created_comment(result: &CommentAddResult) -> Result<String, serde_json::Error> {
    let parent_type = if result.is_task_comment {
        "task"
    } else {
        "project"
    };
    let output = CreatedCommentOutput {
        id: &result.id,
        content: &result.content,
        parent_id: &result.parent_id,
        parent_type,
        parent_name: result.parent_name.as_deref(),
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an edited comment.
#[derive(Serialize)]
pub struct EditedCommentOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub parent_id: &'a str,
    pub parent_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<&'a str>,
}

/// Formats an edited comment as JSON.
pub fn format_edited_comment(result: &CommentEditResult) -> Result<String, serde_json::Error> {
    let parent_type = if result.is_task_comment {
        "task"
    } else {
        "project"
    };
    let output = EditedCommentOutput {
        id: &result.id,
        content: &result.content,
        parent_id: &result.parent_id,
        parent_type,
        parent_name: result.parent_name.as_deref(),
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a deleted comment.
#[derive(Serialize)]
pub struct DeletedCommentOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub parent_id: &'a str,
    pub parent_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<&'a str>,
    pub status: &'static str,
}

/// Formats a deleted comment as JSON.
pub fn format_deleted_comment(result: &CommentDeleteResult) -> Result<String, serde_json::Error> {
    let parent_type = if result.is_task_comment {
        "task"
    } else {
        "project"
    };
    let output = DeletedCommentOutput {
        id: &result.id,
        content: &result.content,
        parent_id: &result.parent_id,
        parent_type,
        parent_name: result.parent_name.as_deref(),
        status: "deleted",
    };

    serde_json::to_string_pretty(&output)
}

/// Formats comments as a table.
pub fn format_comments_table(
    comments: &[Comment],
    parent_name: Option<&str>,
    use_colors: bool,
) -> String {
    if comments.is_empty() {
        return "No comments found.\n".to_string();
    }

    let mut output = String::new();

    // Header with parent context
    if let Some(name) = parent_name {
        let header = format!("Comments for: {}", name);
        if use_colors {
            output.push_str(&format!("{}\n\n", header.bold()));
        } else {
            output.push_str(&header);
            output.push_str("\n\n");
        }
    }

    // Column header
    let header = format!("{:<8} {:<20} {}", "ID", "Posted", "Content");
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Comments
    for comment in comments {
        let id_prefix = truncate_id(comment.id());
        let posted = comment
            .posted_at()
            .map(format_datetime)
            .unwrap_or_default();
        let posted_display = truncate_str(&posted, 20);

        // Truncate content to first line and max 50 chars for table view
        let content_first_line = comment.content().lines().next().unwrap_or("");
        let content_display = truncate_str(content_first_line, 50);

        let line = format!(
            "{:<8} {:<20} {}",
            id_prefix, posted_display, content_display
        );
        output.push_str(&line);
        output.push('\n');
    }

    output
}
