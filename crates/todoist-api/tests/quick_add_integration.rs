//! Integration tests for the Quick Add endpoint.
//!
//! These tests use wiremock to mock the Todoist API responses.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use todoist_api::client::TodoistClient;
use todoist_api::quick_add::QuickAddRequest;
use wiremock::matchers::{body_json_string, header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// Test: Quick add with simple text returns created task
#[tokio::test]
async fn test_quick_add_simple() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "id": "task-123",
        "project_id": "inbox-456",
        "content": "Buy milk",
        "description": "",
        "priority": 1,
        "checked": false,
        "is_deleted": false,
        "child_order": 1,
        "labels": []
    });

    Mock::given(method("POST"))
        .and(path("/tasks/quick"))
        .and(header("Authorization", "Bearer test-token"))
        .and(header("Content-Type", "application/json"))
        .and(body_json_string(r#"{"text":"Buy milk"}"#))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = QuickAddRequest::new("Buy milk");
    let response = client.quick_add(request).await.unwrap();

    assert_eq!(response.id, "task-123");
    assert_eq!(response.project_id, "inbox-456");
    assert_eq!(response.content, "Buy milk");
    assert_eq!(response.priority, 1);
    assert!(!response.checked);
}

/// Test: Quick add with NLP parsed fields
#[tokio::test]
async fn test_quick_add_with_nlp_parsing() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "id": "task-456",
        "project_id": "shopping-123",
        "content": "Buy groceries",
        "description": "",
        "priority": 3,
        "due": {
            "date": "2026-01-26",
            "datetime": "2026-01-26T15:00:00Z",
            "string": "tomorrow at 3pm",
            "is_recurring": false
        },
        "section_id": null,
        "labels": ["errands", "shopping"],
        "child_order": 1,
        "checked": false,
        "is_deleted": false,
        "resolved_project_name": "Shopping"
    });

    Mock::given(method("POST"))
        .and(path("/tasks/quick"))
        .and(header("Authorization", "Bearer test-token"))
        .and(header("Content-Type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = QuickAddRequest::new("Buy groceries tomorrow at 3pm #Shopping p2 @errands @shopping");
    let response = client.quick_add(request).await.unwrap();

    assert_eq!(response.id, "task-456");
    assert_eq!(response.content, "Buy groceries");
    assert_eq!(response.priority, 3);
    assert!(response.has_due_date());

    let due = response.due.as_ref().unwrap();
    assert_eq!(due.date, "2026-01-26");
    assert_eq!(due.string, Some("tomorrow at 3pm".to_string()));

    assert!(response.has_labels());
    assert_eq!(response.labels.len(), 2);
    assert!(response.labels.contains(&"errands".to_string()));
    assert!(response.labels.contains(&"shopping".to_string()));
}

/// Test: Quick add with note attachment
#[tokio::test]
async fn test_quick_add_with_note() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "id": "task-789",
        "project_id": "inbox-123",
        "content": "Call mom",
        "description": "",
        "priority": 1,
        "checked": false,
        "is_deleted": false,
        "child_order": 1,
        "labels": []
    });

    Mock::given(method("POST"))
        .and(path("/tasks/quick"))
        .and(header("Authorization", "Bearer test-token"))
        .and(header("Content-Type", "application/json"))
        .and(body_json_string(r#"{"text":"Call mom","note":"Ask about dinner plans"}"#))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = QuickAddRequest::new("Call mom")
        .with_note("Ask about dinner plans");
    let response = client.quick_add(request).await.unwrap();

    assert_eq!(response.id, "task-789");
    assert_eq!(response.content, "Call mom");
}

/// Test: Quick add with all optional fields
#[tokio::test]
async fn test_quick_add_with_all_options() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "id": "task-full",
        "project_id": "work-123",
        "content": "Team meeting",
        "description": "",
        "priority": 4,
        "due": {
            "date": "2026-01-26",
            "datetime": "2026-01-26T14:00:00Z",
            "string": "tomorrow at 2pm",
            "is_recurring": false
        },
        "checked": false,
        "is_deleted": false,
        "child_order": 1,
        "labels": ["work"]
    });

    Mock::given(method("POST"))
        .and(path("/tasks/quick"))
        .and(header("Content-Type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = QuickAddRequest::new("Team meeting tomorrow at 2pm #Work p1 @work")
        .with_note("Prepare agenda")
        .with_reminder("30 minutes before")
        .with_auto_reminder(true);
    let response = client.quick_add(request).await.unwrap();

    assert_eq!(response.id, "task-full");
    assert_eq!(response.priority, 4);
    assert!(response.has_due_date());
}

/// Test: Quick add retries on 429 rate limit
#[tokio::test]
async fn test_quick_add_retry_on_rate_limit() {
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
                    "id": "task-retry",
                    "project_id": "inbox-123",
                    "content": "Test task",
                    "description": "",
                    "priority": 1,
                    "checked": false,
                    "is_deleted": false,
                    "child_order": 1,
                    "labels": []
                }))
            }
        }
    }

    Mock::given(method("POST"))
        .and(path("/tasks/quick"))
        .respond_with(RetryThenSuccessResponder {
            call_count: call_count.clone(),
        })
        .expect(2)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = QuickAddRequest::new("Test task");
    let response = client.quick_add(request).await.unwrap();

    assert_eq!(response.id, "task-retry");
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

/// Test: Quick add fails with auth error
#[tokio::test]
async fn test_quick_add_auth_failure() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/tasks/quick"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("invalid-token", mock_server.uri());
    let request = QuickAddRequest::new("Test task");
    let result = client.quick_add(request).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 2); // Auth error exit code
}

/// Test: Quick add fails with validation error
#[tokio::test]
async fn test_quick_add_validation_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/tasks/quick"))
        .respond_with(ResponseTemplate::new(400).set_body_string("Invalid request: text is required"))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = QuickAddRequest::new("");
    let result = client.quick_add(request).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 2); // Validation error maps to exit code 2
}

/// Test: Response can be converted to Item
#[tokio::test]
async fn test_quick_add_response_to_item() {
    let mock_server = MockServer::start().await;

    let response_json = serde_json::json!({
        "id": "task-convert",
        "project_id": "proj-123",
        "content": "Task for conversion",
        "description": "With description",
        "priority": 2,
        "due": {
            "date": "2026-01-26",
            "is_recurring": false
        },
        "section_id": "section-456",
        "labels": ["test"],
        "child_order": 3,
        "checked": false,
        "is_deleted": false,
        "added_by_uid": "user-789",
        "added_at": "2026-01-25T10:00:00Z"
    });

    Mock::given(method("POST"))
        .and(path("/tasks/quick"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = TodoistClient::with_base_url("test-token", mock_server.uri());
    let request = QuickAddRequest::new("Task for conversion");
    let response = client.quick_add(request).await.unwrap();

    // Convert to Item
    let item = response.into_item();

    assert_eq!(item.id, "task-convert");
    assert_eq!(item.project_id, "proj-123");
    assert_eq!(item.content, "Task for conversion");
    assert_eq!(item.description, "With description");
    assert_eq!(item.priority, 2);
    assert_eq!(item.section_id, Some("section-456".to_string()));
    assert_eq!(item.labels, vec!["test".to_string()]);
    assert_eq!(item.child_order, 3);
    assert!(!item.checked);
    assert_eq!(item.user_id, Some("user-789".to_string()));
}
