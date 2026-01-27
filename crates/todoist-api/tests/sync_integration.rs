//! Integration tests for the Sync API endpoint.
//!
//! These tests use wiremock to mock the Todoist API responses.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{SyncCommand, SyncRequest};
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// Test: Full sync request returns all resources
#[tokio::test]
async fn test_sync_full_sync() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "sync_token": "new-token-abc123",
        "full_sync": true,
        "full_sync_date_utc": "2025-01-25T10:00:00Z",
        "items": [
            {
                "id": "item-1",
                "project_id": "proj-1",
                "content": "Buy milk",
                "description": "",
                "priority": 1,
                "checked": false,
                "is_deleted": false
            },
            {
                "id": "item-2",
                "project_id": "proj-1",
                "content": "Buy eggs",
                "description": "Organic eggs",
                "priority": 2,
                "checked": false,
                "is_deleted": false
            }
        ],
        "projects": [
            {
                "id": "proj-1",
                "name": "Shopping",
                "color": "blue",
                "is_deleted": false,
                "is_archived": false,
                "is_favorite": true,
                "inbox_project": false
            }
        ],
        "labels": [
            {
                "id": "label-1",
                "name": "urgent",
                "color": "red",
                "item_order": 0,
                "is_deleted": false,
                "is_favorite": false
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(header("Authorization", "Bearer test-token"))
        .and(header("Content-Type", "application/x-www-form-urlencoded"))
        .and(body_string_contains("sync_token=*")) // "*" is unreserved, no encoding needed
        .and(body_string_contains("resource_types="))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = SyncRequest::full_sync();
    let response = client.sync(request).await.unwrap();

    assert_eq!(response.sync_token, "new-token-abc123");
    assert!(response.full_sync);
    assert_eq!(response.items.len(), 2);
    assert_eq!(response.items[0].content, "Buy milk");
    assert_eq!(response.items[1].content, "Buy eggs");
    assert_eq!(response.projects.len(), 1);
    assert_eq!(response.projects[0].name, "Shopping");
    assert_eq!(response.labels.len(), 1);
    assert_eq!(response.labels[0].name, "urgent");
}

/// Test: Incremental sync uses the provided sync_token
#[tokio::test]
async fn test_sync_incremental() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "sync_token": "even-newer-token-xyz",
        "full_sync": false,
        "items": [
            {
                "id": "item-3",
                "project_id": "proj-1",
                "content": "New task since last sync",
                "description": "",
                "priority": 1,
                "checked": false,
                "is_deleted": false
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_string_contains("sync_token=previous-sync-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = SyncRequest::incremental("previous-sync-token");
    let response = client.sync(request).await.unwrap();

    assert_eq!(response.sync_token, "even-newer-token-xyz");
    assert!(!response.full_sync);
    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].content, "New task since last sync");
}

/// Test: Command execution returns sync_status and temp_id_mapping
#[tokio::test]
async fn test_sync_command_execution() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "sync_token": "post-command-token",
        "full_sync": false,
        "sync_status": {
            "cmd-uuid-1": "ok",
            "cmd-uuid-2": "ok"
        },
        "temp_id_mapping": {
            "temp-item-1": "real-item-id-abc",
            "temp-item-2": "real-item-id-xyz"
        }
    });

    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_string_contains("commands="))
        .and(body_string_contains("item_add"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());

    let commands = vec![
        SyncCommand::with_uuid_and_temp_id(
            "item_add",
            "cmd-uuid-1",
            "temp-item-1",
            serde_json::json!({"content": "Task 1", "project_id": "proj-1"}),
        ),
        SyncCommand::with_uuid_and_temp_id(
            "item_add",
            "cmd-uuid-2",
            "temp-item-2",
            serde_json::json!({"content": "Task 2", "project_id": "proj-1"}),
        ),
    ];

    let request = SyncRequest::with_commands(commands);
    let response = client.sync(request).await.unwrap();

    assert!(!response.has_errors());
    assert!(response.sync_status.get("cmd-uuid-1").unwrap().is_ok());
    assert!(response.sync_status.get("cmd-uuid-2").unwrap().is_ok());
    assert_eq!(
        response.real_id("temp-item-1"),
        Some(&"real-item-id-abc".to_string())
    );
    assert_eq!(
        response.real_id("temp-item-2"),
        Some(&"real-item-id-xyz".to_string())
    );
}

/// Test: Command execution with partial failures
#[tokio::test]
async fn test_sync_command_partial_failure() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "sync_token": "token-after-partial-fail",
        "full_sync": false,
        "sync_status": {
            "cmd-success": "ok",
            "cmd-failure": {
                "error_code": 15,
                "error": "Invalid temporary id"
            }
        },
        "temp_id_mapping": {
            "temp-success": "real-id-success"
        }
    });

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());

    let commands = vec![
        SyncCommand::with_uuid_and_temp_id(
            "item_add",
            "cmd-success",
            "temp-success",
            serde_json::json!({"content": "Valid task", "project_id": "proj-1"}),
        ),
        SyncCommand::with_uuid_and_temp_id(
            "item_add",
            "cmd-failure",
            "temp-invalid",
            serde_json::json!({"content": "Invalid task", "project_id": "invalid-temp-id"}),
        ),
    ];

    let request = SyncRequest::with_commands(commands);
    let response = client.sync(request).await.unwrap();

    assert!(response.has_errors());

    let errors = response.errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].0, "cmd-failure");
    assert_eq!(errors[0].1.error_code, 15);
    assert_eq!(errors[0].1.error, "Invalid temporary id");

    // Successful command should still have its mapping
    assert_eq!(
        response.real_id("temp-success"),
        Some(&"real-id-success".to_string())
    );
}

/// Test: Sync with specific resource types
#[tokio::test]
async fn test_sync_specific_resource_types() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "sync_token": "resource-type-token",
        "full_sync": true,
        "items": [
            {
                "id": "item-1",
                "project_id": "proj-1",
                "content": "Task 1",
                "description": "",
                "priority": 1,
                "checked": false,
                "is_deleted": false
            }
        ],
        "projects": [
            {
                "id": "proj-1",
                "name": "Project 1",
                "is_deleted": false,
                "is_archived": false,
                "is_favorite": false,
                "inbox_project": false
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(body_string_contains("items"))
        .and(body_string_contains("projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = SyncRequest::full_sync()
        .with_resource_types(vec!["items".to_string(), "projects".to_string()]);
    let response = client.sync(request).await.unwrap();

    assert_eq!(response.items.len(), 1);
    assert_eq!(response.projects.len(), 1);
}

/// Test: Sync retries on 429 rate limit
#[tokio::test]
async fn test_sync_retry_on_rate_limit() {
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    struct RetryThenSuccessResponder {
        call_count: Arc<AtomicU32>,
    }

    impl Respond for RetryThenSuccessResponder {
        fn respond(&self, _request: &Request) -> ResponseTemplate {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            if count == 0 {
                // First call: return 429
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "1")
                    .set_body_string("Rate limited")
            } else {
                // Second call: return success
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "sync_token": "after-retry-token",
                    "full_sync": true
                }))
            }
        }
    }

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(RetryThenSuccessResponder {
            call_count: call_count.clone(),
        })
        .expect(2)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let response = client.sync(SyncRequest::full_sync()).await.unwrap();

    assert_eq!(response.sync_token, "after-retry-token");
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

/// Test: Sync fails with auth error
#[tokio::test]
async fn test_sync_auth_failure() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("invalid-token", mock_server.uri());
    let result = client.sync(SyncRequest::full_sync()).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 2); // Auth error exit code
}

/// Test: Combined read and write in single sync request
#[tokio::test]
async fn test_sync_combined_read_write() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "sync_token": "combined-token",
        "full_sync": true,
        "items": [
            {
                "id": "existing-item",
                "project_id": "proj-1",
                "content": "Existing task",
                "description": "",
                "priority": 1,
                "checked": false,
                "is_deleted": false
            },
            {
                "id": "real-new-item-id",
                "project_id": "proj-1",
                "content": "New task via command",
                "description": "",
                "priority": 1,
                "checked": false,
                "is_deleted": false
            }
        ],
        "projects": [
            {
                "id": "proj-1",
                "name": "Inbox",
                "is_deleted": false,
                "is_archived": false,
                "is_favorite": false,
                "inbox_project": true
            }
        ],
        "sync_status": {
            "add-item-cmd": "ok"
        },
        "temp_id_mapping": {
            "temp-new-item": "real-new-item-id"
        }
    });

    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(body_string_contains("sync_token"))
        .and(body_string_contains("resource_types"))
        .and(body_string_contains("commands"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());

    let command = SyncCommand::with_uuid_and_temp_id(
        "item_add",
        "add-item-cmd",
        "temp-new-item",
        serde_json::json!({"content": "New task via command", "project_id": "proj-1"}),
    );

    let request = SyncRequest::full_sync().add_commands(vec![command]);
    let response = client.sync(request).await.unwrap();

    // Check read results
    assert!(response.full_sync);
    assert_eq!(response.items.len(), 2);
    assert_eq!(response.projects.len(), 1);

    // Check write results
    assert!(!response.has_errors());
    assert!(response.sync_status.get("add-item-cmd").unwrap().is_ok());
    assert_eq!(
        response.real_id("temp-new-item"),
        Some(&"real-new-item-id".to_string())
    );
}

/// Test: Sync with user data
#[tokio::test]
async fn test_sync_with_user_data() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "sync_token": "user-token",
        "full_sync": true,
        "user": {
            "id": "user-123",
            "email": "test@example.com",
            "full_name": "Test User",
            "timezone": "America/New_York",
            "inbox_project_id": "inbox-456",
            "is_premium": true
        }
    });

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = SyncRequest::full_sync().with_resource_types(vec!["user".to_string()]);
    let response = client.sync(request).await.unwrap();

    let user = response.user.expect("User should be present");
    assert_eq!(user.id, "user-123");
    assert_eq!(user.email, Some("test@example.com".to_string()));
    assert_eq!(user.full_name, Some("Test User".to_string()));
    assert!(user.is_premium);
}

/// Test: Sync with sections and labels
#[tokio::test]
async fn test_sync_with_sections_and_labels() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "sync_token": "sections-labels-token",
        "full_sync": true,
        "sections": [
            {
                "id": "section-1",
                "name": "To Do",
                "project_id": "proj-1",
                "section_order": 1,
                "is_collapsed": false,
                "is_deleted": false,
                "is_archived": false
            },
            {
                "id": "section-2",
                "name": "In Progress",
                "project_id": "proj-1",
                "section_order": 2,
                "is_collapsed": false,
                "is_deleted": false,
                "is_archived": false
            }
        ],
        "labels": [
            {
                "id": "label-1",
                "name": "work",
                "color": "blue",
                "item_order": 0,
                "is_deleted": false,
                "is_favorite": true
            },
            {
                "id": "label-2",
                "name": "personal",
                "color": "green",
                "item_order": 1,
                "is_deleted": false,
                "is_favorite": false
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/sync"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = SyncRequest::full_sync()
        .with_resource_types(vec!["sections".to_string(), "labels".to_string()]);
    let response = client.sync(request).await.unwrap();

    assert_eq!(response.sections.len(), 2);
    assert_eq!(response.sections[0].name, "To Do");
    assert_eq!(response.sections[1].name, "In Progress");

    assert_eq!(response.labels.len(), 2);
    assert_eq!(response.labels[0].name, "work");
    assert!(response.labels[0].is_favorite);
    assert_eq!(response.labels[1].name, "personal");
}

/// Test: Sync with item_close command
#[tokio::test]
async fn test_sync_item_close_command() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "sync_token": "item-close-token",
        "full_sync": false,
        "sync_status": {
            "close-cmd-uuid": "ok"
        }
    });

    Mock::given(method("POST"))
        .and(path("/sync"))
        .and(body_string_contains("item_close"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());

    let command = SyncCommand {
        command_type: "item_close".to_string(),
        uuid: "close-cmd-uuid".to_string(),
        temp_id: None,
        args: serde_json::json!({"id": "item-to-close"}),
    };

    let request = SyncRequest::with_commands(vec![command]);
    let response = client.sync(request).await.unwrap();

    assert!(!response.has_errors());
    assert!(response.sync_status.get("close-cmd-uuid").unwrap().is_ok());
}
