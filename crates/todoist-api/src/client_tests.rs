//! Unit and integration tests for the TodoistClient.

use super::*;
use crate::error::{ApiError, Error};

// Test: TodoistClient struct should exist and hold an API token
#[test]
fn test_todoist_client_struct_exists() {
    let _client: TodoistClient;
}

// Test: TodoistClient::new() constructor should accept a token string
#[test]
fn test_todoist_client_new_accepts_token() {
    let token = "test-api-token-12345";
    let client = TodoistClient::new(token).unwrap();
    let _ = client;
}

// Test: TodoistClient should store the token for later use
#[test]
fn test_todoist_client_stores_token() {
    let token = "my-secret-token";
    let client = TodoistClient::new(token).unwrap();
    assert_eq!(client.token(), token);
}

// Test: TodoistClient should hold a reqwest client internally
#[test]
fn test_todoist_client_has_http_client() {
    let client = TodoistClient::new("test-token").unwrap();
    let _http_client = client.http_client();
}

// Test: TodoistClient should implement Clone
#[test]
fn test_todoist_client_is_clone() {
    let client = TodoistClient::new("test-token").unwrap();
    let _cloned = client.clone();
}

// Test: TodoistClient should implement Debug
#[test]
fn test_todoist_client_is_debug() {
    let client = TodoistClient::new("test-token").unwrap();
    let debug_str = format!("{:?}", client);
    assert!(
        !debug_str.contains("test-token"),
        "Token should be redacted in debug output"
    );
}

// Test: TodoistClient should use the default base URL
#[test]
fn test_todoist_client_default_base_url() {
    let client = TodoistClient::new("test-token").unwrap();
    assert_eq!(client.base_url(), BASE_URL);
}

// Test: TodoistClient can be created with custom base URL
#[test]
fn test_todoist_client_with_custom_base_url() {
    let client = TodoistClient::with_base_url("test-token", "https://test.example.com").unwrap();
    assert_eq!(client.base_url(), "https://test.example.com");
}

// Test: calculate_backoff uses Retry-After when provided
#[test]
fn test_calculate_backoff_with_retry_after() {
    let client = TodoistClient::new("test-token").unwrap();

    // Should use the retry_after value
    let backoff = client.calculate_backoff(0, Some(5));
    assert_eq!(backoff, Duration::from_secs(5));

    // Should cap at max_backoff (default 30s)
    let backoff = client.calculate_backoff(0, Some(60));
    assert_eq!(backoff, Duration::from_secs(DEFAULT_MAX_BACKOFF_SECS));
}

// Test: calculate_backoff uses exponential backoff when no Retry-After
#[test]
fn test_calculate_backoff_exponential() {
    let client = TodoistClient::new("test-token").unwrap();

    // Attempt 0: 1 second
    let backoff = client.calculate_backoff(0, None);
    assert_eq!(backoff, Duration::from_secs(1));

    // Attempt 1: 2 seconds
    let backoff = client.calculate_backoff(1, None);
    assert_eq!(backoff, Duration::from_secs(2));

    // Attempt 2: 4 seconds
    let backoff = client.calculate_backoff(2, None);
    assert_eq!(backoff, Duration::from_secs(4));

    // Attempt 3: 8 seconds
    let backoff = client.calculate_backoff(3, None);
    assert_eq!(backoff, Duration::from_secs(8));
}

// Test: calculate_backoff caps at max_backoff
#[test]
fn test_calculate_backoff_caps_at_max() {
    let client = TodoistClient::new("test-token").unwrap();

    // Very high attempt number should still cap at max_backoff (default 30 seconds)
    let backoff = client.calculate_backoff(10, None);
    assert_eq!(backoff, Duration::from_secs(DEFAULT_MAX_BACKOFF_SECS));
}

// Test: TodoistClient uses the default timeout constant
#[test]
fn test_default_timeout_constant() {
    // Verify the timeout constant is set to 30 seconds
    assert_eq!(DEFAULT_TIMEOUT_SECS, 30);
}

// Test: TodoistClientBuilder creates client with default values
#[test]
fn test_builder_default_values() {
    let client = TodoistClientBuilder::new("test-token").build().unwrap();

    assert_eq!(client.token(), "test-token");
    assert_eq!(client.base_url(), BASE_URL);
    assert_eq!(client.max_retries(), DEFAULT_MAX_RETRIES);
    assert_eq!(
        client.initial_backoff(),
        Duration::from_secs(DEFAULT_INITIAL_BACKOFF_SECS)
    );
    assert_eq!(
        client.max_backoff(),
        Duration::from_secs(DEFAULT_MAX_BACKOFF_SECS)
    );
}

// Test: TodoistClientBuilder allows customizing max_retries
#[test]
fn test_builder_custom_max_retries() {
    let client = TodoistClientBuilder::new("test-token")
        .max_retries(5)
        .build()
        .unwrap();

    assert_eq!(client.max_retries(), 5);
}

// Test: TodoistClientBuilder allows customizing initial_backoff
#[test]
fn test_builder_custom_initial_backoff() {
    let client = TodoistClientBuilder::new("test-token")
        .initial_backoff(Duration::from_millis(500))
        .build()
        .unwrap();

    assert_eq!(client.initial_backoff(), Duration::from_millis(500));
}

// Test: TodoistClientBuilder allows customizing max_backoff
#[test]
fn test_builder_custom_max_backoff() {
    let client = TodoistClientBuilder::new("test-token")
        .max_backoff(Duration::from_secs(60))
        .build()
        .unwrap();

    assert_eq!(client.max_backoff(), Duration::from_secs(60));
}

// Test: TodoistClientBuilder allows chaining all options
#[test]
fn test_builder_chaining() {
    let client = TodoistClientBuilder::new("test-token")
        .base_url("https://custom.example.com")
        .max_retries(5)
        .initial_backoff(Duration::from_millis(500))
        .max_backoff(Duration::from_secs(60))
        .request_timeout(Duration::from_secs(45))
        .build()
        .unwrap();

    assert_eq!(client.base_url(), "https://custom.example.com");
    assert_eq!(client.max_retries(), 5);
    assert_eq!(client.initial_backoff(), Duration::from_millis(500));
    assert_eq!(client.max_backoff(), Duration::from_secs(60));
}

// Test: TodoistClient::builder() returns a builder
#[test]
fn test_client_builder_method() {
    let client = TodoistClient::builder("test-token")
        .max_retries(10)
        .build()
        .unwrap();

    assert_eq!(client.max_retries(), 10);
}

// Test: Custom initial_backoff affects calculate_backoff
#[test]
fn test_custom_initial_backoff_affects_calculation() {
    let client = TodoistClientBuilder::new("test-token")
        .initial_backoff(Duration::from_secs(2))
        .build()
        .unwrap();

    // Attempt 0: 2 seconds (custom initial)
    let backoff = client.calculate_backoff(0, None);
    assert_eq!(backoff, Duration::from_secs(2));

    // Attempt 1: 4 seconds (2 * 2)
    let backoff = client.calculate_backoff(1, None);
    assert_eq!(backoff, Duration::from_secs(4));
}

// Test: Custom max_backoff caps calculation
#[test]
fn test_custom_max_backoff_caps_calculation() {
    let client = TodoistClientBuilder::new("test-token")
        .max_backoff(Duration::from_secs(10))
        .build()
        .unwrap();

    // High attempt should cap at custom max_backoff (10s)
    let backoff = client.calculate_backoff(10, None);
    assert_eq!(backoff, Duration::from_secs(10));

    // Retry-After should also be capped at custom max_backoff
    let backoff = client.calculate_backoff(0, Some(60));
    assert_eq!(backoff, Duration::from_secs(10));
}

mod wiremock_tests {
    use super::*;
    use crate::error::Result;
    use serde::Deserialize;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestTask {
        id: String,
        content: String,
    }

    // Test: GET request succeeds on first try
    #[tokio::test]
    async fn test_get_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/123"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "123",
                "content": "Test task"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let task: TestTask = client.get("/tasks/123").await.unwrap();

        assert_eq!(task.id, "123");
        assert_eq!(task.content, "Test task");
    }

    // Test: GET retries on 429 and succeeds
    #[tokio::test]
    async fn test_get_retry_on_429_then_success() {
        let mock_server = MockServer::start().await;
        let call_count = Arc::new(AtomicU32::new(0));

        struct RetryThenSuccessResponder {
            call_count: Arc<AtomicU32>,
        }

        impl Respond for RetryThenSuccessResponder {
            fn respond(&self, _request: &Request) -> ResponseTemplate {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First call: return 429 with Retry-After
                    ResponseTemplate::new(429)
                        .insert_header("Retry-After", "1")
                        .set_body_string("Rate limited")
                } else {
                    // Second call: return success
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "id": "123",
                        "content": "Test task"
                    }))
                }
            }
        }

        Mock::given(method("GET"))
            .and(path("/tasks/123"))
            .respond_with(RetryThenSuccessResponder {
                call_count: call_count.clone(),
            })
            .expect(2)
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let task: TestTask = client.get("/tasks/123").await.unwrap();

        assert_eq!(task.id, "123");
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    // Test: GET fails after max retries on 429
    #[tokio::test]
    async fn test_get_fails_after_max_retries() {
        let mock_server = MockServer::start().await;
        let call_count = Arc::new(AtomicU32::new(0));

        struct AlwaysRateLimitResponder {
            call_count: Arc<AtomicU32>,
        }

        impl Respond for AlwaysRateLimitResponder {
            fn respond(&self, _request: &Request) -> ResponseTemplate {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "1")
                    .set_body_string("Rate limited")
            }
        }

        Mock::given(method("GET"))
            .and(path("/tasks/123"))
            .respond_with(AlwaysRateLimitResponder {
                call_count: call_count.clone(),
            })
            .expect(4) // Initial + 3 retries = 4 total attempts
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let result: Result<TestTask> = client.get("/tasks/123").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Api(ApiError::RateLimit { .. }) => {}
            e => panic!("Expected RateLimit error, got: {:?}", e),
        }
        assert_eq!(call_count.load(Ordering::SeqCst), 4);
    }

    // Test: POST request succeeds on first try
    #[tokio::test]
    async fn test_post_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/tasks"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "456",
                "content": "New task"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let task: TestTask = client
            .post("/tasks", &serde_json::json!({"content": "New task"}))
            .await
            .unwrap();

        assert_eq!(task.id, "456");
        assert_eq!(task.content, "New task");
    }

    // Test: POST retries on 429
    #[tokio::test]
    async fn test_post_retry_on_429() {
        let mock_server = MockServer::start().await;
        let call_count = Arc::new(AtomicU32::new(0));

        struct RetryThenSuccessResponder {
            call_count: Arc<AtomicU32>,
        }

        impl Respond for RetryThenSuccessResponder {
            fn respond(&self, _request: &Request) -> ResponseTemplate {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    ResponseTemplate::new(429)
                        .insert_header("Retry-After", "1")
                        .set_body_string("Rate limited")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "id": "456",
                        "content": "New task"
                    }))
                }
            }
        }

        Mock::given(method("POST"))
            .and(path("/tasks"))
            .respond_with(RetryThenSuccessResponder {
                call_count: call_count.clone(),
            })
            .expect(3)
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let task: TestTask = client
            .post("/tasks", &serde_json::json!({"content": "New task"}))
            .await
            .unwrap();

        assert_eq!(task.id, "456");
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    // Test: DELETE request succeeds on first try
    #[tokio::test]
    async fn test_delete_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/tasks/123"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let result = client.delete("/tasks/123").await;

        assert!(result.is_ok());
    }

    // Test: DELETE retries on 429
    #[tokio::test]
    async fn test_delete_retry_on_429() {
        let mock_server = MockServer::start().await;
        let call_count = Arc::new(AtomicU32::new(0));

        struct RetryThenSuccessResponder {
            call_count: Arc<AtomicU32>,
        }

        impl Respond for RetryThenSuccessResponder {
            fn respond(&self, _request: &Request) -> ResponseTemplate {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    ResponseTemplate::new(429)
                        .insert_header("Retry-After", "1")
                        .set_body_string("Rate limited")
                } else {
                    ResponseTemplate::new(204)
                }
            }
        }

        Mock::given(method("DELETE"))
            .and(path("/tasks/123"))
            .respond_with(RetryThenSuccessResponder {
                call_count: call_count.clone(),
            })
            .expect(2)
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let result = client.delete("/tasks/123").await;

        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    // Test: post_empty request succeeds
    #[tokio::test]
    async fn test_post_empty_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/tasks/123/close"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "123",
                "content": "Completed task"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let task: TestTask = client.post_empty("/tasks/123/close").await.unwrap();

        assert_eq!(task.id, "123");
    }

    // Test: post_empty retries on 429
    #[tokio::test]
    async fn test_post_empty_retry_on_429() {
        let mock_server = MockServer::start().await;
        let call_count = Arc::new(AtomicU32::new(0));

        struct RetryThenSuccessResponder {
            call_count: Arc<AtomicU32>,
        }

        impl Respond for RetryThenSuccessResponder {
            fn respond(&self, _request: &Request) -> ResponseTemplate {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    ResponseTemplate::new(429)
                        .insert_header("Retry-After", "1")
                        .set_body_string("Rate limited")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "id": "123",
                        "content": "Completed task"
                    }))
                }
            }
        }

        Mock::given(method("POST"))
            .and(path("/tasks/123/close"))
            .respond_with(RetryThenSuccessResponder {
                call_count: call_count.clone(),
            })
            .expect(2)
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let task: TestTask = client.post_empty("/tasks/123/close").await.unwrap();

        assert_eq!(task.id, "123");
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    // Test: Non-429 errors are not retried
    #[tokio::test]
    async fn test_non_retryable_errors_not_retried() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/123"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
            .expect(1) // Should only be called once
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let result: Result<TestTask> = client.get("/tasks/123").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Api(ApiError::NotFound { .. }) => {}
            e => panic!("Expected NotFound error, got: {:?}", e),
        }
    }

    // Test: 401 errors are not retried
    #[tokio::test]
    async fn test_auth_errors_not_retried() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/123"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let result: Result<TestTask> = client.get("/tasks/123").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Api(ApiError::Auth { .. }) => {}
            e => panic!("Expected Auth error, got: {:?}", e),
        }
    }

    // Test: Uses Retry-After header value when present
    #[tokio::test]
    async fn test_uses_retry_after_header() {
        let mock_server = MockServer::start().await;
        let call_count = Arc::new(AtomicU32::new(0));

        struct RetryThenSuccessResponder {
            call_count: Arc<AtomicU32>,
        }

        impl Respond for RetryThenSuccessResponder {
            fn respond(&self, _request: &Request) -> ResponseTemplate {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First call returns 429 with a small Retry-After so test doesn't take long
                    ResponseTemplate::new(429)
                        .insert_header("Retry-After", "1")
                        .set_body_string("Rate limited")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "id": "123",
                        "content": "Test task"
                    }))
                }
            }
        }

        Mock::given(method("GET"))
            .and(path("/tasks/123"))
            .respond_with(RetryThenSuccessResponder {
                call_count: call_count.clone(),
            })
            .expect(2)
            .mount(&mock_server)
            .await;

        let client = TodoistClient::with_base_url("test-token", mock_server.uri()).unwrap();
        let start = std::time::Instant::now();
        let task: TestTask = client.get("/tasks/123").await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(task.id, "123");
        // Should have waited at least 1 second (the Retry-After value)
        assert!(
            elapsed >= Duration::from_millis(900),
            "Expected delay of ~1s, got {:?}",
            elapsed
        );
    }

    // Test: Client has timeout configured and times out on slow responses
    #[tokio::test]
    async fn test_client_timeout_on_slow_response() {
        let mock_server = MockServer::start().await;

        // Response that delays longer than the test client's timeout
        // We'll create a client with a short timeout for testing
        Mock::given(method("GET"))
            .and(path("/tasks/slow"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({
                        "id": "123",
                        "content": "Test task"
                    }))
                    .set_delay(Duration::from_secs(5)),
            ) // Delay longer than our test timeout
            .mount(&mock_server)
            .await;

        // Create a client with a very short timeout for testing using the builder
        let client = TodoistClientBuilder::new("test-token")
            .base_url(mock_server.uri())
            .request_timeout(Duration::from_secs(1))
            .build()
            .unwrap();

        let result: Result<TestTask> = client.get("/tasks/slow").await;

        // Should fail with a timeout error
        assert!(result.is_err(), "Expected timeout error");
        match result {
            Err(Error::Http(req_err)) => {
                assert!(
                    req_err.is_timeout(),
                    "Expected timeout error, got: {:?}",
                    req_err
                );
            }
            Err(e) => panic!("Expected HTTP timeout error, got: {:?}", e),
            Ok(_) => panic!("Expected error, got success"),
        }
    }
}
