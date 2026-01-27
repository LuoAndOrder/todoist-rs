//! Sync API request types.

use serde::{Deserialize, Serialize};

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
/// ## Create a simple command
///
/// ```
/// use todoist_api_rs::sync::SyncCommand;
/// use serde_json::json;
///
/// let cmd = SyncCommand::new("item_close", json!({"id": "task-123"}));
/// assert_eq!(cmd.command_type, "item_close");
/// assert!(cmd.temp_id.is_none());
/// ```
///
/// ## Create a command with temp_id for new resources
///
/// ```
/// use todoist_api_rs::sync::SyncCommand;
/// use serde_json::json;
///
/// // When creating a new item, use temp_id so you can reference it in subsequent commands
/// let cmd = SyncCommand::with_temp_id(
///     "item_add",
///     "temp-task-1",
///     json!({"content": "Buy groceries", "project_id": "inbox"})
/// );
/// assert_eq!(cmd.temp_id, Some("temp-task-1".to_string()));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncCommand {
    /// The type of command (e.g., "item_add", "project_update").
    #[serde(rename = "type")]
    pub command_type: String,

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
    pub fn new(command_type: impl Into<String>, args: serde_json::Value) -> Self {
        Self {
            command_type: command_type.into(),
            uuid: uuid::Uuid::new_v4().to_string(),
            temp_id: None,
            args,
        }
    }

    /// Creates a new command with a temp_id for resource creation.
    pub fn with_temp_id(
        command_type: impl Into<String>,
        temp_id: impl Into<String>,
        args: serde_json::Value,
    ) -> Self {
        Self {
            command_type: command_type.into(),
            uuid: uuid::Uuid::new_v4().to_string(),
            temp_id: Some(temp_id.into()),
            args,
        }
    }

    /// Creates a new command with explicit UUID and temp_id.
    pub fn with_uuid_and_temp_id(
        command_type: impl Into<String>,
        uuid: impl Into<String>,
        temp_id: impl Into<String>,
        args: serde_json::Value,
    ) -> Self {
        Self {
            command_type: command_type.into(),
            uuid: uuid.into(),
            temp_id: Some(temp_id.into()),
            args,
        }
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
        let cmd = SyncCommand::new("item_add", serde_json::json!({"content": "Test"}));
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
            command_type: "item_add".to_string(),
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
        let cmd = SyncCommand::new("item_add", serde_json::json!({"content": "Test"}));
        assert_eq!(cmd.command_type, "item_add");
        assert!(cmd.temp_id.is_none());
        // UUID should be a valid UUID
        assert!(uuid::Uuid::parse_str(&cmd.uuid).is_ok());
    }

    #[test]
    fn test_sync_command_with_temp_id() {
        let cmd = SyncCommand::with_temp_id(
            "item_add",
            "temp-123",
            serde_json::json!({"content": "Test"}),
        );
        assert_eq!(cmd.command_type, "item_add");
        assert_eq!(cmd.temp_id, Some("temp-123".to_string()));
    }

    #[test]
    fn test_sync_command_with_uuid_and_temp_id() {
        let cmd = SyncCommand::with_uuid_and_temp_id(
            "project_add",
            "my-uuid",
            "temp-456",
            serde_json::json!({"name": "Project"}),
        );
        assert_eq!(cmd.command_type, "project_add");
        assert_eq!(cmd.uuid, "my-uuid");
        assert_eq!(cmd.temp_id, Some("temp-456".to_string()));
    }

    #[test]
    fn test_sync_command_serialize() {
        let cmd = SyncCommand {
            command_type: "item_add".to_string(),
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
            command_type: "item_close".to_string(),
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
        assert_eq!(cmd.command_type, "item_add");
        assert_eq!(cmd.uuid, "abc-123");
        assert_eq!(cmd.temp_id, Some("temp-xyz".to_string()));
        assert_eq!(cmd.args["content"], "Test task");
    }
}
