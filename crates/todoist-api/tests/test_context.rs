//! Test context for E2E tests with rate-limit-aware sync state management.
//!
//! This module provides a `TestContext` struct that minimizes API calls by:
//! - Performing ONE full sync at initialization
//! - Using partial (incremental) syncs for all subsequent operations
//! - Caching state locally to avoid re-syncing for verification
//!
//! ## Rate Limits
//!
//! The Todoist API has strict rate limits:
//! - Full sync: 100 requests / 15 minutes
//! - Partial sync: 1000 requests / 15 minutes
//! - Commands per request: 100 max
//!
//! By using `TestContext`, tests use partial syncs instead of full syncs,
//! allowing ~10x more API calls before hitting rate limits.

#![cfg(feature = "extended-e2e")]
#![allow(dead_code)]

use std::fs;
use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{
    Filter, Item, Label, Note, Project, ProjectNote, Reminder, Section, SyncCommand,
    SyncCommandType, SyncRequest, SyncResponse,
};

/// Reads the API token from .env.local or environment variable.
pub fn get_test_token() -> Option<String> {
    // Try to read from .env.local at workspace root
    for path in &[".env.local", "../../.env.local", "../../../.env.local"] {
        if let Ok(contents) = fs::read_to_string(path) {
            for line in contents.lines() {
                if let Some(token) = line
                    .strip_prefix("TODOIST_TEST_API_TOKEN=")
                    .or_else(|| line.strip_prefix("todoist_test_api_key="))
                {
                    return Some(token.trim().to_string());
                }
            }
        }
    }

    std::env::var("TODOIST_TEST_API_TOKEN")
        .or_else(|_| std::env::var("TODOIST_TEST_API_KEY"))
        .ok()
}

/// Shared context for E2E tests that minimizes API calls.
///
/// `TestContext` performs ONE full sync at initialization and uses partial syncs
/// for all subsequent operations. This dramatically reduces API calls and helps
/// stay within Todoist's rate limits.
///
/// ## Usage
///
/// ```rust,ignore
/// #[tokio::test]
/// async fn test_example() {
///     let mut ctx = TestContext::new().await.expect("Failed to create context");
///
///     // Create a task (partial sync with command)
///     let temp_id = uuid::Uuid::new_v4().to_string();
///     let response = ctx.execute(vec![
///         SyncCommand::with_temp_id("item_add", &temp_id, json!({
///             "content": "Test task",
///             "project_id": ctx.inbox_id()
///         }))
///     ]).await.unwrap();
///
///     // Get the real ID from temp_id mapping
///     let task_id = response.real_id(&temp_id).unwrap();
///
///     // Verify from cache (no API call)
///     let task = ctx.find_item(task_id).expect("Task should exist in cache");
///     assert_eq!(task.content, "Test task");
///
///     // Cleanup
///     ctx.execute(vec![
///         SyncCommand::new("item_delete", json!({"id": task_id}))
///     ]).await.unwrap();
/// }
/// ```
#[derive(Debug)]
pub struct TestContext {
    client: TodoistClient,
    sync_token: String,
    inbox_id: String,
    user_timezone: String,
    // Cached state from syncs
    items: Vec<Item>,
    projects: Vec<Project>,
    sections: Vec<Section>,
    labels: Vec<Label>,
    notes: Vec<Note>,
    project_notes: Vec<ProjectNote>,
    reminders: Vec<Reminder>,
    filters: Vec<Filter>,
}

impl TestContext {
    /// Creates a new TestContext with ONE full sync.
    ///
    /// Returns `None` if the API token is not available.
    /// Returns `Err` if the full sync fails.
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let token = get_test_token().ok_or("TODOIST_TEST_API_TOKEN not found")?;
        let client = TodoistClient::new(token)?;

        // ONE full sync at initialization
        let response = client.sync(SyncRequest::full_sync()).await?;

        let inbox_id = response
            .projects
            .iter()
            .find(|p| p.inbox_project && !p.is_deleted)
            .ok_or("Should have inbox project")?
            .id
            .clone();

        // Extract user timezone, default to UTC if not available
        let user_timezone = response
            .user
            .as_ref()
            .and_then(|u| u.timezone().map(|s| s.to_string()))
            .unwrap_or_else(|| "UTC".to_string());

        Ok(Self {
            client,
            sync_token: response.sync_token,
            inbox_id,
            user_timezone,
            items: response.items,
            projects: response.projects,
            sections: response.sections,
            labels: response.labels,
            notes: response.notes,
            project_notes: response.project_notes,
            reminders: response.reminders,
            filters: response.filters,
        })
    }

    /// Returns the inbox project ID.
    pub fn inbox_id(&self) -> &str {
        &self.inbox_id
    }

    /// Returns the user's timezone (e.g., "America/New_York").
    pub fn user_timezone(&self) -> &str {
        &self.user_timezone
    }

    /// Returns a reference to the API client.
    pub fn client(&self) -> &TodoistClient {
        &self.client
    }

    /// Returns the current sync token.
    pub fn sync_token(&self) -> &str {
        &self.sync_token
    }

    /// Executes commands and updates cached state (partial sync).
    ///
    /// This method:
    /// 1. Sends the commands with the current sync token
    /// 2. Requests all resource types to get updated state
    /// 3. Merges the response into the cached state
    /// 4. Updates the sync token for the next call
    ///
    /// Returns the full `SyncResponse` for access to `temp_id_mapping` and `sync_status`.
    pub async fn execute(
        &mut self,
        commands: Vec<SyncCommand>,
    ) -> Result<SyncResponse, todoist_api_rs::error::Error> {
        let request = SyncRequest::incremental(&self.sync_token)
            .with_resource_types(vec!["all".to_string()])
            .add_commands(commands);

        let response = self.client.sync(request).await?;

        // Update sync token
        self.sync_token = response.sync_token.clone();

        // Merge response data into cached state
        self.merge_response(&response);

        Ok(response)
    }

    /// Performs a partial sync to refresh state without executing commands.
    ///
    /// Use this to get the latest state from the server when you need to
    /// verify changes made outside of the test context.
    pub async fn refresh(&mut self) -> Result<SyncResponse, todoist_api_rs::error::Error> {
        let request =
            SyncRequest::incremental(&self.sync_token).with_resource_types(vec!["all".to_string()]);

        let response = self.client.sync(request).await?;

        // Update sync token
        self.sync_token = response.sync_token.clone();

        // Merge response data into cached state
        self.merge_response(&response);

        Ok(response)
    }

    /// Finds an item (task) in the cached state by ID.
    ///
    /// Returns `None` if the item is not found or is deleted.
    /// This does NOT make an API call.
    pub fn find_item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id && !i.is_deleted)
    }

    /// Finds a project in the cached state by ID.
    ///
    /// Returns `None` if the project is not found or is deleted.
    /// This does NOT make an API call.
    pub fn find_project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id && !p.is_deleted)
    }

    /// Finds a project by name in the cached state.
    ///
    /// Returns `None` if not found or deleted. Case-insensitive.
    /// This does NOT make an API call.
    pub fn find_project_by_name(&self, name: &str) -> Option<&Project> {
        self.projects
            .iter()
            .find(|p| !p.is_deleted && p.name.eq_ignore_ascii_case(name))
    }

    /// Finds a section in the cached state by ID.
    ///
    /// Returns `None` if the section is not found or is deleted.
    /// This does NOT make an API call.
    pub fn find_section(&self, id: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.id == id && !s.is_deleted)
    }

    /// Finds a label in the cached state by ID.
    ///
    /// Returns `None` if the label is not found or is deleted.
    /// This does NOT make an API call.
    pub fn find_label(&self, id: &str) -> Option<&Label> {
        self.labels.iter().find(|l| l.id == id && !l.is_deleted)
    }

    /// Finds a label by name in the cached state.
    ///
    /// Returns `None` if not found or deleted. Case-insensitive.
    /// This does NOT make an API call.
    pub fn find_label_by_name(&self, name: &str) -> Option<&Label> {
        self.labels
            .iter()
            .find(|l| !l.is_deleted && l.name.eq_ignore_ascii_case(name))
    }

    /// Finds a reminder in the cached state by ID.
    ///
    /// Returns `None` if the reminder is not found or is deleted.
    /// This does NOT make an API call.
    pub fn find_reminder(&self, id: &str) -> Option<&Reminder> {
        self.reminders.iter().find(|r| r.id == id && !r.is_deleted)
    }

    /// Finds all reminders for a specific task.
    ///
    /// Returns a vector of non-deleted reminders for the given item_id.
    /// This does NOT make an API call.
    pub fn find_reminders_for_task(&self, item_id: &str) -> Vec<&Reminder> {
        self.reminders
            .iter()
            .filter(|r| !r.is_deleted && r.item_id == item_id)
            .collect()
    }

    /// Returns all non-deleted items in the cache.
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|i| !i.is_deleted)
    }

    /// Returns all non-deleted projects in the cache.
    pub fn projects(&self) -> impl Iterator<Item = &Project> {
        self.projects.iter().filter(|p| !p.is_deleted)
    }

    /// Returns all non-deleted sections in the cache.
    pub fn sections(&self) -> impl Iterator<Item = &Section> {
        self.sections.iter().filter(|s| !s.is_deleted)
    }

    /// Returns all non-deleted labels in the cache.
    pub fn labels(&self) -> impl Iterator<Item = &Label> {
        self.labels.iter().filter(|l| !l.is_deleted)
    }

    /// Merges a sync response into the cached state.
    ///
    /// For each resource type in the response:
    /// - Updates existing items with matching IDs
    /// - Adds new items that don't exist in cache
    fn merge_response(&mut self, response: &SyncResponse) {
        // Merge items
        Self::merge_vec(&mut self.items, &response.items, |item| &item.id);

        // Merge projects
        Self::merge_vec(&mut self.projects, &response.projects, |proj| &proj.id);

        // Merge sections
        Self::merge_vec(&mut self.sections, &response.sections, |sec| &sec.id);

        // Merge labels
        Self::merge_vec(&mut self.labels, &response.labels, |label| &label.id);

        // Merge notes
        Self::merge_vec(&mut self.notes, &response.notes, |note| &note.id);

        // Merge project notes
        Self::merge_vec(&mut self.project_notes, &response.project_notes, |pn| {
            &pn.id
        });

        // Merge reminders
        Self::merge_vec(&mut self.reminders, &response.reminders, |rem| &rem.id);

        // Merge filters
        Self::merge_vec(&mut self.filters, &response.filters, |filt| &filt.id);
    }

    /// Helper to merge a vector of items, updating existing or adding new.
    fn merge_vec<T: Clone, F>(cache: &mut Vec<T>, incoming: &[T], get_id: F)
    where
        F: Fn(&T) -> &String,
    {
        for item in incoming {
            let id = get_id(item);
            if let Some(existing) = cache.iter_mut().find(|c| get_id(c) == id) {
                *existing = item.clone();
            } else {
                cache.push(item.clone());
            }
        }
    }
}

/// Helper trait for creating test resources with cleanup.
impl TestContext {
    /// Creates a task and returns its real ID.
    ///
    /// This is a convenience helper that handles the temp_id mapping.
    pub async fn create_task(
        &mut self,
        content: &str,
        project_id: &str,
        extra_args: Option<serde_json::Value>,
    ) -> Result<String, todoist_api_rs::error::Error> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let mut args = serde_json::json!({
            "content": content,
            "project_id": project_id
        });

        if let Some(extra) = extra_args {
            if let (Some(obj), Some(extra_obj)) = (args.as_object_mut(), extra.as_object()) {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }

        let command = SyncCommand::with_temp_id(SyncCommandType::ItemAdd, &temp_id, args);
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "item_add failed: {:?}",
                response.errors()
            )));
        }

        response.real_id(&temp_id).cloned().ok_or_else(|| {
            todoist_api_rs::error::Error::Internal("No temp_id mapping returned".to_string())
        })
    }

    /// Creates a project and returns its real ID.
    pub async fn create_project(
        &mut self,
        name: &str,
    ) -> Result<String, todoist_api_rs::error::Error> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command = SyncCommand::with_temp_id(
            SyncCommandType::ProjectAdd,
            &temp_id,
            serde_json::json!({ "name": name }),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "project_add failed: {:?}",
                response.errors()
            )));
        }

        response.real_id(&temp_id).cloned().ok_or_else(|| {
            todoist_api_rs::error::Error::Internal("No temp_id mapping returned".to_string())
        })
    }

    /// Creates a section and returns its real ID.
    pub async fn create_section(
        &mut self,
        name: &str,
        project_id: &str,
    ) -> Result<String, todoist_api_rs::error::Error> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command = SyncCommand::with_temp_id(
            SyncCommandType::SectionAdd,
            &temp_id,
            serde_json::json!({
                "name": name,
                "project_id": project_id
            }),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "section_add failed: {:?}",
                response.errors()
            )));
        }

        response.real_id(&temp_id).cloned().ok_or_else(|| {
            todoist_api_rs::error::Error::Internal("No temp_id mapping returned".to_string())
        })
    }

    /// Creates a label and returns its real ID.
    pub async fn create_label(
        &mut self,
        name: &str,
    ) -> Result<String, todoist_api_rs::error::Error> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command = SyncCommand::with_temp_id(
            SyncCommandType::LabelAdd,
            &temp_id,
            serde_json::json!({ "name": name }),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "label_add failed: {:?}",
                response.errors()
            )));
        }

        response.real_id(&temp_id).cloned().ok_or_else(|| {
            todoist_api_rs::error::Error::Internal("No temp_id mapping returned".to_string())
        })
    }

    /// Deletes a task.
    pub async fn delete_task(&mut self, task_id: &str) -> Result<(), todoist_api_rs::error::Error> {
        let command = SyncCommand::new(
            SyncCommandType::ItemDelete,
            serde_json::json!({"id": task_id}),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "item_delete failed: {:?}",
                response.errors()
            )));
        }

        Ok(())
    }

    /// Deletes a project.
    pub async fn delete_project(
        &mut self,
        project_id: &str,
    ) -> Result<(), todoist_api_rs::error::Error> {
        let command = SyncCommand::new(
            SyncCommandType::ProjectDelete,
            serde_json::json!({"id": project_id}),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "project_delete failed: {:?}",
                response.errors()
            )));
        }

        Ok(())
    }

    /// Deletes a section.
    pub async fn delete_section(
        &mut self,
        section_id: &str,
    ) -> Result<(), todoist_api_rs::error::Error> {
        let command = SyncCommand::new(
            SyncCommandType::SectionDelete,
            serde_json::json!({"id": section_id}),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "section_delete failed: {:?}",
                response.errors()
            )));
        }

        Ok(())
    }

    /// Deletes a label.
    pub async fn delete_label(
        &mut self,
        label_id: &str,
    ) -> Result<(), todoist_api_rs::error::Error> {
        let command = SyncCommand::new(
            SyncCommandType::LabelDelete,
            serde_json::json!({"id": label_id}),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "label_delete failed: {:?}",
                response.errors()
            )));
        }

        Ok(())
    }

    /// Creates an absolute reminder and returns its real ID.
    ///
    /// Creates a reminder at a specific datetime.
    pub async fn create_absolute_reminder(
        &mut self,
        item_id: &str,
        due_datetime: &str,
    ) -> Result<String, todoist_api_rs::error::Error> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command = SyncCommand::with_temp_id(
            SyncCommandType::ReminderAdd,
            &temp_id,
            serde_json::json!({
                "item_id": item_id,
                "type": "absolute",
                "due": {
                    "date": due_datetime
                }
            }),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "reminder_add (absolute) failed: {:?}",
                response.errors()
            )));
        }

        response.real_id(&temp_id).cloned().ok_or_else(|| {
            todoist_api_rs::error::Error::Internal("No temp_id mapping returned".to_string())
        })
    }

    /// Creates a relative reminder and returns its real ID.
    ///
    /// Creates a reminder that fires `minute_offset` minutes before the task's due time.
    /// Note: The task must have a due date with time set.
    pub async fn create_relative_reminder(
        &mut self,
        item_id: &str,
        minute_offset: i32,
    ) -> Result<String, todoist_api_rs::error::Error> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command = SyncCommand::with_temp_id(
            SyncCommandType::ReminderAdd,
            &temp_id,
            serde_json::json!({
                "item_id": item_id,
                "type": "relative",
                "minute_offset": minute_offset
            }),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "reminder_add (relative) failed: {:?}",
                response.errors()
            )));
        }

        response.real_id(&temp_id).cloned().ok_or_else(|| {
            todoist_api_rs::error::Error::Internal("No temp_id mapping returned".to_string())
        })
    }

    /// Deletes a reminder.
    pub async fn delete_reminder(
        &mut self,
        reminder_id: &str,
    ) -> Result<(), todoist_api_rs::error::Error> {
        let command = SyncCommand::new(
            SyncCommandType::ReminderDelete,
            serde_json::json!({"id": reminder_id}),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "reminder_delete failed: {:?}",
                response.errors()
            )));
        }

        Ok(())
    }

    /// Finds a note (task comment) in the cached state by ID.
    ///
    /// Returns `None` if the note is not found or is deleted.
    /// This does NOT make an API call.
    pub fn find_note(&self, id: &str) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id && !n.is_deleted)
    }

    /// Finds all notes (comments) for a specific task.
    ///
    /// Returns a vector of non-deleted notes for the given item_id.
    /// This does NOT make an API call.
    pub fn find_notes_for_task(&self, item_id: &str) -> Vec<&Note> {
        self.notes
            .iter()
            .filter(|n| !n.is_deleted && n.item_id == item_id)
            .collect()
    }

    /// Finds a project note (project comment) in the cached state by ID.
    ///
    /// Returns `None` if the note is not found or is deleted.
    /// This does NOT make an API call.
    pub fn find_project_note(&self, id: &str) -> Option<&ProjectNote> {
        self.project_notes
            .iter()
            .find(|n| n.id == id && !n.is_deleted)
    }

    /// Finds all notes (comments) for a specific project.
    ///
    /// Returns a vector of non-deleted project notes for the given project_id.
    /// This does NOT make an API call.
    pub fn find_notes_for_project(&self, project_id: &str) -> Vec<&ProjectNote> {
        self.project_notes
            .iter()
            .filter(|n| !n.is_deleted && n.project_id == project_id)
            .collect()
    }

    /// Creates a task comment (note) and returns its real ID.
    pub async fn create_task_comment(
        &mut self,
        item_id: &str,
        content: &str,
    ) -> Result<String, todoist_api_rs::error::Error> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command = SyncCommand::with_temp_id(
            SyncCommandType::NoteAdd,
            &temp_id,
            serde_json::json!({
                "item_id": item_id,
                "content": content
            }),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "note_add failed: {:?}",
                response.errors()
            )));
        }

        response.real_id(&temp_id).cloned().ok_or_else(|| {
            todoist_api_rs::error::Error::Internal("No temp_id mapping returned".to_string())
        })
    }

    /// Deletes a task comment (note).
    pub async fn delete_note(&mut self, note_id: &str) -> Result<(), todoist_api_rs::error::Error> {
        let command = SyncCommand::new(
            SyncCommandType::NoteDelete,
            serde_json::json!({"id": note_id}),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "note_delete failed: {:?}",
                response.errors()
            )));
        }

        Ok(())
    }

    /// Creates a project comment (project note) and returns its real ID.
    pub async fn create_project_comment(
        &mut self,
        project_id: &str,
        content: &str,
    ) -> Result<String, todoist_api_rs::error::Error> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command = SyncCommand::with_temp_id(
            SyncCommandType::ProjectNoteAdd,
            &temp_id,
            serde_json::json!({
                "project_id": project_id,
                "content": content
            }),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "project_note_add failed: {:?}",
                response.errors()
            )));
        }

        response.real_id(&temp_id).cloned().ok_or_else(|| {
            todoist_api_rs::error::Error::Internal("No temp_id mapping returned".to_string())
        })
    }

    /// Deletes a project comment (project note).
    pub async fn delete_project_note(
        &mut self,
        note_id: &str,
    ) -> Result<(), todoist_api_rs::error::Error> {
        let command = SyncCommand::new(
            SyncCommandType::ProjectNoteDelete,
            serde_json::json!({"id": note_id}),
        );
        let response = self.execute(vec![command]).await?;

        if response.has_errors() {
            return Err(todoist_api_rs::error::Error::Internal(format!(
                "project_note_delete failed: {:?}",
                response.errors()
            )));
        }

        Ok(())
    }

    /// Batch delete multiple resources in one API call.
    ///
    /// This is more efficient than deleting one at a time.
    /// Note: Reminders should be deleted before their associated tasks.
    pub async fn batch_delete(
        &mut self,
        task_ids: &[&str],
        project_ids: &[&str],
        section_ids: &[&str],
        label_ids: &[&str],
    ) -> Result<(), todoist_api_rs::error::Error> {
        self.batch_delete_with_reminders(task_ids, project_ids, section_ids, label_ids, &[])
            .await
    }

    /// Batch delete multiple resources including reminders in one API call.
    ///
    /// This is more efficient than deleting one at a time.
    /// Reminders are deleted first, then tasks, sections, projects, and labels.
    pub async fn batch_delete_with_reminders(
        &mut self,
        task_ids: &[&str],
        project_ids: &[&str],
        section_ids: &[&str],
        label_ids: &[&str],
        reminder_ids: &[&str],
    ) -> Result<(), todoist_api_rs::error::Error> {
        self.batch_delete_all(
            task_ids,
            project_ids,
            section_ids,
            label_ids,
            reminder_ids,
            &[],
            &[],
        )
        .await
    }

    /// Batch delete multiple resources including notes in one API call.
    ///
    /// This is more efficient than deleting one at a time.
    /// Notes are deleted first (they depend on tasks/projects), then tasks, sections, projects, and labels.
    pub async fn batch_delete_with_notes(
        &mut self,
        task_ids: &[&str],
        project_ids: &[&str],
        note_ids: &[&str],
        project_note_ids: &[&str],
    ) -> Result<(), todoist_api_rs::error::Error> {
        self.batch_delete_all(
            task_ids,
            project_ids,
            &[],
            &[],
            &[],
            note_ids,
            project_note_ids,
        )
        .await
    }

    /// Batch delete all types of resources in one API call.
    ///
    /// This is the most flexible cleanup method. Resources are deleted in dependency order:
    /// 1. Notes and project notes (depend on tasks/projects)
    /// 2. Reminders (depend on tasks)
    /// 3. Tasks
    /// 4. Sections
    /// 5. Projects
    /// 6. Labels
    #[allow(clippy::too_many_arguments)]
    pub async fn batch_delete_all(
        &mut self,
        task_ids: &[&str],
        project_ids: &[&str],
        section_ids: &[&str],
        label_ids: &[&str],
        reminder_ids: &[&str],
        note_ids: &[&str],
        project_note_ids: &[&str],
    ) -> Result<(), todoist_api_rs::error::Error> {
        let mut commands = Vec::new();

        // Delete notes first (they depend on tasks)
        for id in note_ids {
            commands.push(SyncCommand::new(
                SyncCommandType::NoteDelete,
                serde_json::json!({"id": id}),
            ));
        }

        // Delete project notes (they depend on projects)
        for id in project_note_ids {
            commands.push(SyncCommand::new(
                SyncCommandType::ProjectNoteDelete,
                serde_json::json!({"id": id}),
            ));
        }

        // Delete reminders (they depend on tasks)
        for id in reminder_ids {
            commands.push(SyncCommand::new(
                SyncCommandType::ReminderDelete,
                serde_json::json!({"id": id}),
            ));
        }

        for id in task_ids {
            commands.push(SyncCommand::new(
                SyncCommandType::ItemDelete,
                serde_json::json!({"id": id}),
            ));
        }

        for id in section_ids {
            commands.push(SyncCommand::new(
                SyncCommandType::SectionDelete,
                serde_json::json!({"id": id}),
            ));
        }

        for id in project_ids {
            commands.push(SyncCommand::new(
                SyncCommandType::ProjectDelete,
                serde_json::json!({"id": id}),
            ));
        }

        for id in label_ids {
            commands.push(SyncCommand::new(
                SyncCommandType::LabelDelete,
                serde_json::json!({"id": id}),
            ));
        }

        if commands.is_empty() {
            return Ok(());
        }

        let response = self.execute(commands).await?;

        if response.has_errors() {
            // Log errors but don't fail - cleanup errors are common
            eprintln!(
                "Warning: Some cleanup operations failed: {:?}",
                response.errors()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_initialization() {
        let ctx = TestContext::new().await;

        match ctx {
            Ok(ctx) => {
                // Verify basic state
                assert!(
                    !ctx.inbox_id().is_empty(),
                    "inbox_id should not be empty after initialization"
                );
                assert!(
                    !ctx.sync_token().is_empty(),
                    "sync_token should not be empty after initialization"
                );
                // Should have at least the inbox project
                assert!(
                    ctx.projects().count() >= 1,
                    "should have at least the inbox project"
                );
            }
            Err(e) => {
                eprintln!("Skipping test: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_context_create_and_find_task() {
        let ctx = TestContext::new().await;

        let Ok(mut ctx) = ctx else {
            eprintln!("Skipping test: no API token");
            return;
        };

        let inbox_id = ctx.inbox_id().to_string();

        // Create a task
        let task_id = ctx
            .create_task("TestContext - test task", &inbox_id, None)
            .await
            .expect("Should create task");

        // Find it in cache (no API call)
        let task = ctx.find_item(&task_id).expect("Task should be in cache");
        assert_eq!(
            task.content, "TestContext - test task",
            "task content should match what was created"
        );
        assert_eq!(task.project_id, inbox_id, "task should be in inbox project");

        // Cleanup
        ctx.delete_task(&task_id).await.expect("Should delete task");

        // Verify deleted from cache
        assert!(
            ctx.find_item(&task_id).is_none(),
            "Deleted task should not be findable"
        );
    }

    #[tokio::test]
    async fn test_context_batch_operations() {
        let ctx = TestContext::new().await;

        let Ok(mut ctx) = ctx else {
            eprintln!("Skipping test: no API token");
            return;
        };

        let inbox_id = ctx.inbox_id().to_string();

        // Create multiple tasks in one batch
        let temp_ids: Vec<String> = (0..3).map(|_| uuid::Uuid::new_v4().to_string()).collect();
        let commands: Vec<SyncCommand> = temp_ids
            .iter()
            .enumerate()
            .map(|(i, temp_id)| {
                SyncCommand::with_temp_id(
                    SyncCommandType::ItemAdd,
                    temp_id,
                    serde_json::json!({
                        "content": format!("TestContext - batch task {}", i),
                        "project_id": inbox_id
                    }),
                )
            })
            .collect();

        let response = ctx
            .execute(commands)
            .await
            .expect("Batch create should work");
        assert!(
            !response.has_errors(),
            "batch create should not have errors"
        );

        // Get real IDs
        let task_ids: Vec<String> = temp_ids
            .iter()
            .map(|tid| response.real_id(tid).unwrap().clone())
            .collect();

        // Verify all tasks in cache
        for (i, task_id) in task_ids.iter().enumerate() {
            let task = ctx.find_item(task_id).expect("Task should be in cache");
            assert_eq!(
                task.content,
                format!("TestContext - batch task {}", i),
                "task {} content should match",
                i
            );
        }

        // Batch delete
        let task_refs: Vec<&str> = task_ids.iter().map(|s| s.as_str()).collect();
        ctx.batch_delete(&task_refs, &[], &[], &[])
            .await
            .expect("Batch delete should work");
    }
}
