//! Output formatting utilities for the td CLI.
//!
//! This module provides functions for formatting data as tables or JSON.
//! It is organized into submodules by entity type:
//!
//! - [`tasks`] - Task output formatting (list, show, add, quick add)
//! - [`projects`] - Project output formatting (list, show, add, edit, archive, delete)
//! - [`labels`] - Label output formatting (list, add, edit, delete)
//! - [`sections`] - Section output formatting (list, add, edit, delete)
//! - [`comments`] - Comment output formatting (list, add, edit, delete)
//! - [`reminders`] - Reminder output formatting (list, add, delete)
//! - [`filters`] - Filter output formatting (list, show, add, edit, delete)
//! - [`helpers`] - Common formatting utilities (truncation, priority, due dates)

mod comments;
mod filters;
pub mod helpers;
mod labels;
mod projects;
mod reminders;
mod sections;
mod tasks;

// Re-export all public functions from submodules

// Tasks
pub use tasks::{
    format_created_item, format_item_details_json, format_item_details_table, format_items_json,
    format_items_table, format_quick_add_result,
};

// Projects
pub use projects::{
    format_archived_project, format_created_project, format_deleted_project,
    format_edited_project, format_project_details_json, format_project_details_table,
    format_projects_json, format_projects_table, format_unarchived_project,
};

// Labels
pub use labels::{
    format_created_label, format_deleted_label, format_edited_label, format_labels_json,
    format_labels_table,
};

// Sections
pub use sections::{
    format_created_section, format_deleted_section, format_edited_section, format_sections_json,
    format_sections_table,
};

// Comments
pub use comments::{
    format_comments_json, format_comments_table, format_created_comment, format_deleted_comment,
    format_edited_comment,
};

// Reminders
pub use reminders::{
    format_created_reminder, format_deleted_reminder, format_reminders_json,
    format_reminders_table,
};

// Filters
pub use filters::{
    format_created_filter, format_deleted_filter, format_edited_filter,
    format_filter_details_json, format_filter_details_table, format_filters_json,
    format_filters_table,
};
