//! Sync API request types.

use serde::{Deserialize, Serialize};

/// Valid command types for the Todoist Sync API.
///
/// This enum provides type-safe command types that serialize to the snake_case
/// format expected by the API (e.g., `ItemAdd` → `"item_add"`).
///
/// See: <https://developer.todoist.com/sync/v9/#sync-commands>
///
/// # Examples
///
/// ```
/// use todoist_api_rs::sync::SyncCommandType;
///
/// let cmd_type = SyncCommandType::ItemAdd;
/// let json = serde_json::to_string(&cmd_type).unwrap();
/// assert_eq!(json, "\"item_add\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncCommandType {
    // Item commands
    /// Add a new task/item
    ItemAdd,
    /// Update an existing task/item
    ItemUpdate,
    /// Move a task to a different project or section
    ItemMove,
    /// Delete a task/item
    ItemDelete,
    /// Complete/close a task
    ItemClose,
    /// Complete a task with a specific completion timestamp
    ItemComplete,
    /// Reopen a completed task
    ItemUncomplete,
    /// Archive a task
    ItemArchive,
    /// Unarchive a task
    ItemUnarchive,
    /// Reorder tasks within a project/section
    ItemReorder,
    /// Update day orders for tasks
    ItemUpdateDayOrders,
    /// Update the completion date of a task
    ItemUpdateDateCompleted,

    // Project commands
    /// Add a new project
    ProjectAdd,
    /// Update an existing project
    ProjectUpdate,
    /// Move a project (change parent)
    ProjectMove,
    /// Delete a project
    ProjectDelete,
    /// Archive a project
    ProjectArchive,
    /// Unarchive a project
    ProjectUnarchive,
    /// Reorder projects
    ProjectReorder,

    // Section commands
    /// Add a new section
    SectionAdd,
    /// Update an existing section
    SectionUpdate,
    /// Move a section to a different project
    SectionMove,
    /// Delete a section
    SectionDelete,
    /// Archive a section
    SectionArchive,
    /// Unarchive a section
    SectionUnarchive,
    /// Reorder sections within a project
    SectionReorder,

    // Label commands
    /// Add a new label
    LabelAdd,
    /// Update an existing label
    LabelUpdate,
    /// Delete a label
    LabelDelete,
    /// Update label ordering
    LabelUpdateOrders,

    // Note/Comment commands
    /// Add a note/comment to a task
    NoteAdd,
    /// Update an existing note/comment
    NoteUpdate,
    /// Delete a note/comment
    NoteDelete,

    // Project Note commands
    /// Add a note to a project
    ProjectNoteAdd,
    /// Update an existing project note
    ProjectNoteUpdate,
    /// Delete a project note
    ProjectNoteDelete,

    // Reminder commands
    /// Add a reminder to a task
    ReminderAdd,
    /// Update an existing reminder
    ReminderUpdate,
    /// Delete a reminder
    ReminderDelete,

    // Filter commands
    /// Add a custom filter
    FilterAdd,
    /// Update an existing filter
    FilterUpdate,
    /// Delete a filter
    FilterDelete,
    /// Update filter ordering
    FilterUpdateOrders,
}

/// Request body for the Sync API endpoint.
///
/// The Sync API uses `application/x-www-form-urlencoded` format, where
/// `resource_types` and `commands` are JSON-encoded strings.
///
/// # Examples
///
/// ## Full sync to fetch all data
///
/// ```
/// use todoist_api_rs::sync::SyncRequest;
///
/// let request = SyncRequest::full_sync();
/// assert_eq!(request.sync_token, "*");
/// assert_eq!(request.resource_types, vec!["all"]);
/// ```
///
/// ## Incremental sync with stored token
///
/// ```
/// use todoist_api_rs::sync::SyncRequest;
///
/// let request = SyncRequest::incremental("abc123token");
/// assert_eq!(request.sync_token, "abc123token");
/// ```
///
/// ## Fetch specific resource types
///
/// ```
/// use todoist_api_rs::sync::SyncRequest;
///
/// let request = SyncRequest::full_sync()
///     .with_resource_types(vec!["items".to_string(), "projects".to_string()]);
/// assert_eq!(request.resource_types, vec!["items", "projects"]);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SyncRequest {
    /// Sync token for incremental sync. Use "*" for a full sync.
    pub sync_token: String,

    /// Resource types to fetch. Use `["all"]` for all resources.
    pub resource_types: Vec<String>,

    /// Commands to execute (for write operations).
    pub commands: Vec<SyncCommand>,
}

/// Internal struct for form encoding.
/// The Sync API expects resource_types and commands as JSON-encoded strings.
#[derive(Serialize)]
struct SyncRequestForm<'a> {
    sync_token: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_types: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commands: Option<String>,
}

impl SyncRequest {
    /// Creates a new SyncRequest for a full sync of all resources.
    pub fn full_sync() -> Self {
        Self {
            sync_token: "*".to_string(),
            resource_types: vec!["all".to_string()],
            commands: Vec::new(),
        }
    }

    /// Creates a new SyncRequest for an incremental sync.
    pub fn incremental(sync_token: impl Into<String>) -> Self {
        Self {
            sync_token: sync_token.into(),
            resource_types: vec!["all".to_string()],
            commands: Vec::new(),
        }
    }

    /// Creates a new SyncRequest with only commands (for write operations).
    pub fn with_commands(commands: Vec<SyncCommand>) -> Self {
        Self {
            sync_token: "*".to_string(),
            resource_types: Vec::new(),
            commands,
        }
    }

    /// Creates a new SyncRequest with pre-allocated command capacity.
    ///
    /// Use this when you know ahead of time how many commands will be added,
    /// to avoid reallocations during batch operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncRequest;
    ///
    /// // Pre-allocate for a batch of 100 commands
    /// let mut request = SyncRequest::with_commands_capacity(100);
    /// assert!(request.commands.capacity() >= 100);
    /// ```
    pub fn with_commands_capacity(capacity: usize) -> Self {
        Self {
            sync_token: "*".to_string(),
            resource_types: Vec::new(),
            commands: Vec::with_capacity(capacity),
        }
    }

    /// Sets specific resource types to fetch.
    pub fn with_resource_types(mut self, types: Vec<String>) -> Self {
        self.resource_types = types;
        self
    }

    /// Adds commands to the request.
    pub fn add_commands(mut self, commands: Vec<SyncCommand>) -> Self {
        self.commands.extend(commands);
        self
    }

    /// Serializes the request to form-urlencoded format.
    ///
    /// The Sync API expects:
    /// - `sync_token`: string
    /// - `resource_types`: JSON-encoded array of strings
    /// - `commands`: JSON-encoded array of command objects (if any)
    pub fn to_form_body(&self) -> String {
        let form = SyncRequestForm {
            sync_token: &self.sync_token,
            resource_types: if self.resource_types.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_string(&self.resource_types)
                        .expect("resource_types serialization should not fail"),
                )
            },
            commands: if self.commands.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_string(&self.commands)
                        .expect("commands serialization should not fail"),
                )
            },
        };

        serde_urlencoded::to_string(&form).expect("form serialization should not fail")
    }
}

/// A command to execute via the Sync API.
///
/// Commands are write operations that modify resources in Todoist.
/// Each command has a UUID for idempotency and optional temp_id for
/// creating resources that can be referenced by other commands.
///
/// # Examples
///
/// ## Create a simple command using type-safe builder
///
/// ```
/// use todoist_api_rs::sync::{SyncCommand, SyncCommandType};
/// use serde_json::json;
///
/// let cmd = SyncCommand::new(SyncCommandType::ItemClose, json!({"id": "task-123"}));
/// assert_eq!(cmd.command_type, SyncCommandType::ItemClose);
/// assert!(cmd.temp_id.is_none());
/// ```
///
/// ## Use convenience builders for common operations
///
/// ```
/// use todoist_api_rs::sync::SyncCommand;
///
/// let cmd = SyncCommand::item_close("task-123");
/// assert!(cmd.args["id"].as_str() == Some("task-123"));
/// ```
///
/// ## Create a command with temp_id for new resources
///
/// ```
/// use todoist_api_rs::sync::{SyncCommand, SyncCommandType};
/// use serde_json::json;
///
/// // When creating a new item, use temp_id so you can reference it in subsequent commands
/// let cmd = SyncCommand::with_temp_id(
///     SyncCommandType::ItemAdd,
///     "temp-task-1",
///     json!({"content": "Buy groceries", "project_id": "inbox"})
/// );
/// assert_eq!(cmd.temp_id, Some("temp-task-1".to_string()));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncCommand {
    /// The type of command (e.g., ItemAdd, ProjectUpdate).
    #[serde(rename = "type")]
    pub command_type: SyncCommandType,

    /// Unique identifier for this command (for idempotency).
    pub uuid: String,

    /// Temporary ID for newly created resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_id: Option<String>,

    /// Command-specific arguments.
    pub args: serde_json::Value,
}

impl SyncCommand {
    /// Creates a new command with a generated UUID.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::{SyncCommand, SyncCommandType};
    /// use serde_json::json;
    ///
    /// let cmd = SyncCommand::new(SyncCommandType::ItemClose, json!({"id": "task-123"}));
    /// assert_eq!(cmd.command_type, SyncCommandType::ItemClose);
    /// ```
    pub fn new(command_type: SyncCommandType, args: serde_json::Value) -> Self {
        Self {
            command_type,
            uuid: uuid::Uuid::new_v4().to_string(),
            temp_id: None,
            args,
        }
    }

    /// Creates a new command with a temp_id for resource creation.
    ///
    /// Use this when creating new resources that need to be referenced by
    /// subsequent commands in the same batch.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::{SyncCommand, SyncCommandType};
    /// use serde_json::json;
    ///
    /// let cmd = SyncCommand::with_temp_id(
    ///     SyncCommandType::ItemAdd,
    ///     "temp-123",
    ///     json!({"content": "Buy groceries"})
    /// );
    /// assert_eq!(cmd.temp_id, Some("temp-123".to_string()));
    /// ```
    pub fn with_temp_id(
        command_type: SyncCommandType,
        temp_id: impl Into<String>,
        args: serde_json::Value,
    ) -> Self {
        Self {
            command_type,
            uuid: uuid::Uuid::new_v4().to_string(),
            temp_id: Some(temp_id.into()),
            args,
        }
    }

    /// Creates a new command with explicit UUID and temp_id.
    ///
    /// Use this when you need deterministic UUIDs for testing or idempotency.
    pub fn with_uuid_and_temp_id(
        command_type: SyncCommandType,
        uuid: impl Into<String>,
        temp_id: impl Into<String>,
        args: serde_json::Value,
    ) -> Self {
        Self {
            command_type,
            uuid: uuid.into(),
            temp_id: Some(temp_id.into()),
            args,
        }
    }

    // =========================================================================
    // Item command builders
    // =========================================================================

    /// Creates an item_close command to complete a task.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::item_close("task-123");
    /// assert_eq!(cmd.args["id"], "task-123");
    /// ```
    pub fn item_close(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::ItemClose,
            serde_json::json!({ "id": id.into() }),
        )
    }

    /// Creates an item_uncomplete command to reopen a task.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::item_uncomplete("task-123");
    /// assert_eq!(cmd.args["id"], "task-123");
    /// ```
    pub fn item_uncomplete(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::ItemUncomplete,
            serde_json::json!({ "id": id.into() }),
        )
    }

    /// Creates an item_delete command to delete a task.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::item_delete("task-123");
    /// assert_eq!(cmd.args["id"], "task-123");
    /// ```
    pub fn item_delete(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::ItemDelete,
            serde_json::json!({ "id": id.into() }),
        )
    }

    // =========================================================================
    // Project command builders
    // =========================================================================

    /// Creates a project_delete command to delete a project.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::project_delete("proj-123");
    /// assert_eq!(cmd.args["id"], "proj-123");
    /// ```
    pub fn project_delete(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::ProjectDelete,
            serde_json::json!({ "id": id.into() }),
        )
    }

    /// Creates a project_archive command to archive a project.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::project_archive("proj-123");
    /// assert_eq!(cmd.args["id"], "proj-123");
    /// ```
    pub fn project_archive(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::ProjectArchive,
            serde_json::json!({ "id": id.into() }),
        )
    }

    /// Creates a project_unarchive command to unarchive a project.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::project_unarchive("proj-123");
    /// assert_eq!(cmd.args["id"], "proj-123");
    /// ```
    pub fn project_unarchive(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::ProjectUnarchive,
            serde_json::json!({ "id": id.into() }),
        )
    }

    // =========================================================================
    // Section command builders
    // =========================================================================

    /// Creates a section_delete command to delete a section.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::section_delete("section-123");
    /// assert_eq!(cmd.args["id"], "section-123");
    /// ```
    pub fn section_delete(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::SectionDelete,
            serde_json::json!({ "id": id.into() }),
        )
    }

    /// Creates a section_archive command to archive a section.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::section_archive("section-123");
    /// assert_eq!(cmd.args["id"], "section-123");
    /// ```
    pub fn section_archive(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::SectionArchive,
            serde_json::json!({ "id": id.into() }),
        )
    }

    /// Creates a section_unarchive command to unarchive a section.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::section_unarchive("section-123");
    /// assert_eq!(cmd.args["id"], "section-123");
    /// ```
    pub fn section_unarchive(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::SectionUnarchive,
            serde_json::json!({ "id": id.into() }),
        )
    }

    // =========================================================================
    // Label command builders
    // =========================================================================

    /// Creates a label_delete command to delete a label.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::label_delete("label-123");
    /// assert_eq!(cmd.args["id"], "label-123");
    /// ```
    pub fn label_delete(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::LabelDelete,
            serde_json::json!({ "id": id.into() }),
        )
    }

    // =========================================================================
    // Note/Comment command builders
    // =========================================================================

    /// Creates a note_delete command to delete a task comment.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::note_delete("note-123");
    /// assert_eq!(cmd.args["id"], "note-123");
    /// ```
    pub fn note_delete(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::NoteDelete,
            serde_json::json!({ "id": id.into() }),
        )
    }

    /// Creates a project_note_delete command to delete a project comment.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::project_note_delete("note-123");
    /// assert_eq!(cmd.args["id"], "note-123");
    /// ```
    pub fn project_note_delete(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::ProjectNoteDelete,
            serde_json::json!({ "id": id.into() }),
        )
    }

    // =========================================================================
    // Reminder command builders
    // =========================================================================

    /// Creates a reminder_delete command to delete a reminder.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::reminder_delete("reminder-123");
    /// assert_eq!(cmd.args["id"], "reminder-123");
    /// ```
    pub fn reminder_delete(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::ReminderDelete,
            serde_json::json!({ "id": id.into() }),
        )
    }

    // =========================================================================
    // Filter command builders
    // =========================================================================

    /// Creates a filter_delete command to delete a filter.
    ///
    /// # Examples
    ///
    /// ```
    /// use todoist_api_rs::sync::SyncCommand;
    ///
    /// let cmd = SyncCommand::filter_delete("filter-123");
    /// assert_eq!(cmd.args["id"], "filter-123");
    /// ```
    pub fn filter_delete(id: impl Into<String>) -> Self {
        Self::new(
            SyncCommandType::FilterDelete,
            serde_json::json!({ "id": id.into() }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_request_full_sync() {
        let request = SyncRequest::full_sync();
        assert_eq!(request.sync_token, "*");
        assert_eq!(request.resource_types, vec!["all"]);
        assert!(request.commands.is_empty());
    }

    #[test]
    fn test_sync_request_incremental() {
        let request = SyncRequest::incremental("abc123token");
        assert_eq!(request.sync_token, "abc123token");
        assert_eq!(request.resource_types, vec!["all"]);
    }

    #[test]
    fn test_sync_request_with_commands() {
        let cmd = SyncCommand::new(SyncCommandType::ItemAdd, serde_json::json!({"content": "Test"}));
        let request = SyncRequest::with_commands(vec![cmd]);
        assert_eq!(request.commands.len(), 1);
        assert!(request.resource_types.is_empty());
    }

    #[test]
    fn test_sync_request_with_resource_types() {
        let request = SyncRequest::full_sync()
            .with_resource_types(vec!["items".to_string(), "projects".to_string()]);
        assert_eq!(request.resource_types, vec!["items", "projects"]);
    }

    #[test]
    fn test_sync_request_to_form_body_full_sync() {
        let request = SyncRequest::full_sync();
        let body = request.to_form_body();

        // Decode and verify the fields are correctly encoded
        let decoded: std::collections::HashMap<String, String> =
            serde_urlencoded::from_str(&body).unwrap();
        assert_eq!(decoded.get("sync_token").unwrap(), "*");
        let resource_types: Vec<String> =
            serde_json::from_str(decoded.get("resource_types").unwrap()).unwrap();
        assert_eq!(resource_types, vec!["all"]);
    }

    #[test]
    fn test_sync_request_to_form_body_with_token() {
        let request = SyncRequest::incremental("mytoken123");
        let body = request.to_form_body();

        assert!(body.contains("sync_token=mytoken123"));
    }

    #[test]
    fn test_sync_request_to_form_body_with_commands() {
        let cmd = SyncCommand {
            command_type: SyncCommandType::ItemAdd,
            uuid: "test-uuid".to_string(),
            temp_id: Some("temp-123".to_string()),
            args: serde_json::json!({"content": "Buy milk"}),
        };
        let request = SyncRequest::with_commands(vec![cmd]);
        let body = request.to_form_body();

        // Verify commands are included and properly encoded
        assert!(body.contains("commands="));
        // Decode the form body and check the commands field
        let decoded: std::collections::HashMap<String, String> =
            serde_urlencoded::from_str(&body).unwrap();
        let commands_json = decoded.get("commands").unwrap();
        assert!(commands_json.contains("item_add"));
        assert!(commands_json.contains("test-uuid"));
        assert!(commands_json.contains("temp-123"));
        assert!(commands_json.contains("Buy milk"));
    }

    #[test]
    fn test_sync_request_to_form_body_multiple_resource_types() {
        let request = SyncRequest::full_sync()
            .with_resource_types(vec!["items".to_string(), "projects".to_string()]);
        let body = request.to_form_body();

        // Decode the form body and check the resource_types field
        let decoded: std::collections::HashMap<String, String> =
            serde_urlencoded::from_str(&body).unwrap();
        let resource_types_json = decoded.get("resource_types").unwrap();
        let types: Vec<String> = serde_json::from_str(resource_types_json).unwrap();
        assert_eq!(types, vec!["items", "projects"]);
    }

    #[test]
    fn test_sync_command_new() {
        let cmd = SyncCommand::new(SyncCommandType::ItemAdd, serde_json::json!({"content": "Test"}));
        assert_eq!(cmd.command_type, SyncCommandType::ItemAdd);
        assert!(cmd.temp_id.is_none());
        // UUID should be a valid UUID
        assert!(uuid::Uuid::parse_str(&cmd.uuid).is_ok());
    }

    #[test]
    fn test_sync_command_with_temp_id() {
        let cmd = SyncCommand::with_temp_id(
            SyncCommandType::ItemAdd,
            "temp-123",
            serde_json::json!({"content": "Test"}),
        );
        assert_eq!(cmd.command_type, SyncCommandType::ItemAdd);
        assert_eq!(cmd.temp_id, Some("temp-123".to_string()));
    }

    #[test]
    fn test_sync_command_with_uuid_and_temp_id() {
        let cmd = SyncCommand::with_uuid_and_temp_id(
            SyncCommandType::ProjectAdd,
            "my-uuid",
            "temp-456",
            serde_json::json!({"name": "Project"}),
        );
        assert_eq!(cmd.command_type, SyncCommandType::ProjectAdd);
        assert_eq!(cmd.uuid, "my-uuid");
        assert_eq!(cmd.temp_id, Some("temp-456".to_string()));
    }

    #[test]
    fn test_sync_command_serialize() {
        let cmd = SyncCommand {
            command_type: SyncCommandType::ItemAdd,
            uuid: "cmd-uuid".to_string(),
            temp_id: Some("temp-id".to_string()),
            args: serde_json::json!({"content": "Task", "project_id": "proj-123"}),
        };

        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains(r#""type":"item_add""#));
        assert!(json.contains(r#""uuid":"cmd-uuid""#));
        assert!(json.contains(r#""temp_id":"temp-id""#));
        assert!(json.contains(r#""content":"Task""#));
    }

    #[test]
    fn test_sync_command_serialize_without_temp_id() {
        let cmd = SyncCommand {
            command_type: SyncCommandType::ItemClose,
            uuid: "cmd-uuid".to_string(),
            temp_id: None,
            args: serde_json::json!({"id": "item-123"}),
        };

        let json = serde_json::to_string(&cmd).unwrap();
        assert!(!json.contains("temp_id"));
    }

    #[test]
    fn test_sync_command_deserialize() {
        let json = r#"{
            "type": "item_add",
            "uuid": "abc-123",
            "temp_id": "temp-xyz",
            "args": {"content": "Test task"}
        }"#;

        let cmd: SyncCommand = serde_json::from_str(json).unwrap();
        assert_eq!(cmd.command_type, SyncCommandType::ItemAdd);
        assert_eq!(cmd.uuid, "abc-123");
        assert_eq!(cmd.temp_id, Some("temp-xyz".to_string()));
        assert_eq!(cmd.args["content"], "Test task");
    }

    // =========================================================================
    // SyncCommandType enum tests
    // =========================================================================

    #[test]
    fn test_command_type_serializes_to_snake_case() {
        let cmd_type = SyncCommandType::ItemAdd;
        let json = serde_json::to_string(&cmd_type).unwrap();
        assert_eq!(json, "\"item_add\"");
    }

    #[test]
    fn test_command_type_deserializes_from_snake_case() {
        let cmd_type: SyncCommandType = serde_json::from_str("\"item_close\"").unwrap();
        assert_eq!(cmd_type, SyncCommandType::ItemClose);
    }

    #[test]
    fn test_all_command_types_serialize_correctly() {
        // Item commands
        assert_eq!(serde_json::to_string(&SyncCommandType::ItemAdd).unwrap(), "\"item_add\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ItemUpdate).unwrap(), "\"item_update\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ItemMove).unwrap(), "\"item_move\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ItemDelete).unwrap(), "\"item_delete\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ItemClose).unwrap(), "\"item_close\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ItemUncomplete).unwrap(), "\"item_uncomplete\"");

        // Project commands
        assert_eq!(serde_json::to_string(&SyncCommandType::ProjectAdd).unwrap(), "\"project_add\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ProjectUpdate).unwrap(), "\"project_update\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ProjectDelete).unwrap(), "\"project_delete\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ProjectArchive).unwrap(), "\"project_archive\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ProjectUnarchive).unwrap(), "\"project_unarchive\"");

        // Section commands
        assert_eq!(serde_json::to_string(&SyncCommandType::SectionAdd).unwrap(), "\"section_add\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::SectionDelete).unwrap(), "\"section_delete\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::SectionArchive).unwrap(), "\"section_archive\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::SectionUnarchive).unwrap(), "\"section_unarchive\"");

        // Label commands
        assert_eq!(serde_json::to_string(&SyncCommandType::LabelAdd).unwrap(), "\"label_add\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::LabelDelete).unwrap(), "\"label_delete\"");

        // Note commands
        assert_eq!(serde_json::to_string(&SyncCommandType::NoteAdd).unwrap(), "\"note_add\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::NoteDelete).unwrap(), "\"note_delete\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ProjectNoteAdd).unwrap(), "\"project_note_add\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ProjectNoteDelete).unwrap(), "\"project_note_delete\"");

        // Reminder commands
        assert_eq!(serde_json::to_string(&SyncCommandType::ReminderAdd).unwrap(), "\"reminder_add\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::ReminderDelete).unwrap(), "\"reminder_delete\"");

        // Filter commands
        assert_eq!(serde_json::to_string(&SyncCommandType::FilterAdd).unwrap(), "\"filter_add\"");
        assert_eq!(serde_json::to_string(&SyncCommandType::FilterDelete).unwrap(), "\"filter_delete\"");
    }

    #[test]
    fn test_sync_command_serializes_correctly() {
        let cmd = SyncCommand::item_close("12345");
        let json = serde_json::to_value(&cmd).unwrap();
        assert_eq!(json["type"], "item_close");
        assert_eq!(json["args"]["id"], "12345");
    }

    // =========================================================================
    // Builder method tests
    // =========================================================================

    #[test]
    fn test_item_close_builder() {
        let cmd = SyncCommand::item_close("task-123");
        assert_eq!(cmd.command_type, SyncCommandType::ItemClose);
        assert_eq!(cmd.args["id"], "task-123");
        assert!(cmd.temp_id.is_none());
    }

    #[test]
    fn test_item_uncomplete_builder() {
        let cmd = SyncCommand::item_uncomplete("task-456");
        assert_eq!(cmd.command_type, SyncCommandType::ItemUncomplete);
        assert_eq!(cmd.args["id"], "task-456");
    }

    #[test]
    fn test_item_delete_builder() {
        let cmd = SyncCommand::item_delete("task-789");
        assert_eq!(cmd.command_type, SyncCommandType::ItemDelete);
        assert_eq!(cmd.args["id"], "task-789");
    }

    #[test]
    fn test_project_delete_builder() {
        let cmd = SyncCommand::project_delete("proj-123");
        assert_eq!(cmd.command_type, SyncCommandType::ProjectDelete);
        assert_eq!(cmd.args["id"], "proj-123");
    }

    #[test]
    fn test_project_archive_builder() {
        let cmd = SyncCommand::project_archive("proj-456");
        assert_eq!(cmd.command_type, SyncCommandType::ProjectArchive);
        assert_eq!(cmd.args["id"], "proj-456");
    }

    #[test]
    fn test_section_delete_builder() {
        let cmd = SyncCommand::section_delete("section-123");
        assert_eq!(cmd.command_type, SyncCommandType::SectionDelete);
        assert_eq!(cmd.args["id"], "section-123");
    }

    #[test]
    fn test_label_delete_builder() {
        let cmd = SyncCommand::label_delete("label-123");
        assert_eq!(cmd.command_type, SyncCommandType::LabelDelete);
        assert_eq!(cmd.args["id"], "label-123");
    }

    #[test]
    fn test_note_delete_builder() {
        let cmd = SyncCommand::note_delete("note-123");
        assert_eq!(cmd.command_type, SyncCommandType::NoteDelete);
        assert_eq!(cmd.args["id"], "note-123");
    }

    #[test]
    fn test_reminder_delete_builder() {
        let cmd = SyncCommand::reminder_delete("reminder-123");
        assert_eq!(cmd.command_type, SyncCommandType::ReminderDelete);
        assert_eq!(cmd.args["id"], "reminder-123");
    }

    #[test]
    fn test_filter_delete_builder() {
        let cmd = SyncCommand::filter_delete("filter-123");
        assert_eq!(cmd.command_type, SyncCommandType::FilterDelete);
        assert_eq!(cmd.args["id"], "filter-123");
    }
}
