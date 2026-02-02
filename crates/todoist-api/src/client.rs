//! HTTP client wrapper for the Todoist API.

use std::fmt;
use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};
use tokio::time::sleep;

use crate::error::{ApiError, Error, Result};
use crate::quick_add::{QuickAddRequest, QuickAddResponse};
use crate::sync::{SyncRequest, SyncResponse};

/// Base URL for the Todoist API v1.
const BASE_URL: &str = "https://api.todoist.com/api/v1";

/// Default initial backoff duration for retries (1 second).
const DEFAULT_INITIAL_BACKOFF_SECS: u64 = 1;

/// Default maximum backoff duration for retries (30 seconds).
const DEFAULT_MAX_BACKOFF_SECS: u64 = 30;

/// Default maximum number of retry attempts.
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default request timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Configuration for retry behavior.
#[derive(Clone, Debug)]
struct RetryConfig {
    /// Maximum number of retry attempts.
    max_retries: u32,
    /// Initial backoff duration for retries.
    initial_backoff: Duration,
    /// Maximum backoff duration for retries.
    max_backoff: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff: Duration::from_secs(DEFAULT_INITIAL_BACKOFF_SECS),
            max_backoff: Duration::from_secs(DEFAULT_MAX_BACKOFF_SECS),
        }
    }
}

/// Builder for creating a [`TodoistClient`] with custom configuration.
///
/// # Thread Safety
///
/// The builder itself is [`Send`] and [`Sync`]. The built [`TodoistClient`]
/// is also thread-safe and can be freely shared across threads.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use todoist_api_rs::client::TodoistClientBuilder;
///
/// let client = TodoistClientBuilder::new("your-api-token")
///     .max_retries(5)
///     .initial_backoff(Duration::from_millis(500))
///     .max_backoff(Duration::from_secs(60))
///     .request_timeout(Duration::from_secs(45))
///     .build()
///     .expect("Failed to build client");
/// ```
#[derive(Clone, Debug)]
pub struct TodoistClientBuilder {
    token: String,
    base_url: String,
    max_retries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
    request_timeout: Duration,
}

impl TodoistClientBuilder {
    /// Creates a new builder with the given API token and default configuration.
    ///
    /// Default values:
    /// - `max_retries`: 3
    /// - `initial_backoff`: 1 second
    /// - `max_backoff`: 30 seconds
    /// - `request_timeout`: 30 seconds
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            base_url: BASE_URL.to_string(),
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff: Duration::from_secs(DEFAULT_INITIAL_BACKOFF_SECS),
            max_backoff: Duration::from_secs(DEFAULT_MAX_BACKOFF_SECS),
            request_timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }

    /// Sets a custom base URL (primarily for testing).
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Sets the maximum number of retry attempts for rate-limited requests.
    ///
    /// Default: 3
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Sets the initial backoff duration for exponential backoff.
    ///
    /// Default: 1 second
    pub fn initial_backoff(mut self, initial_backoff: Duration) -> Self {
        self.initial_backoff = initial_backoff;
        self
    }

    /// Sets the maximum backoff duration for exponential backoff.
    ///
    /// Default: 30 seconds
    pub fn max_backoff(mut self, max_backoff: Duration) -> Self {
        self.max_backoff = max_backoff;
        self
    }

    /// Sets the request timeout duration.
    ///
    /// Default: 30 seconds
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Builds the [`TodoistClient`] with the configured settings.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying HTTP client fails to build,
    /// which can happen due to TLS configuration issues or invalid settings.
    pub fn build(self) -> Result<TodoistClient> {
        let http_client = reqwest::Client::builder()
            .timeout(self.request_timeout)
            .build()
            .map_err(Error::Http)?;

        Ok(TodoistClient {
            token: self.token,
            http_client,
            base_url: self.base_url,
            retry_config: RetryConfig {
                max_retries: self.max_retries,
                initial_backoff: self.initial_backoff,
                max_backoff: self.max_backoff,
            },
        })
    }
}

/// Client for interacting with the Todoist API.
///
/// # Thread Safety
///
/// `TodoistClient` is both [`Send`] and [`Sync`], making it safe to share across
/// threads. The underlying HTTP client (`reqwest::Client`) handles connection
/// pooling and is designed for concurrent use.
///
/// For optimal performance, create a single client instance and share it
/// (via `Arc` or cloning) across tasks rather than creating new clients.
///
/// ```
/// use std::sync::Arc;
/// use todoist_api_rs::client::TodoistClient;
///
/// let client = Arc::new(TodoistClient::new("token").unwrap());
///
/// // Clone the Arc to share across tasks
/// let client_clone = Arc::clone(&client);
/// ```
#[derive(Clone)]
pub struct TodoistClient {
    token: String,
    http_client: reqwest::Client,
    base_url: String,
    retry_config: RetryConfig,
}

impl TodoistClient {
    /// Creates a new TodoistClient with the given API token and default configuration.
    ///
    /// This is a convenience method equivalent to:
    /// ```
    /// # use todoist_api_rs::client::TodoistClientBuilder;
    /// # let token = "your-api-token";
    /// TodoistClientBuilder::new(token).build().unwrap();
    /// ```
    ///
    /// Default configuration:
    /// - Request timeout: 30 seconds
    /// - Max retries: 3
    /// - Initial backoff: 1 second
    /// - Max backoff: 30 seconds
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying HTTP client fails to build,
    /// which can happen due to TLS configuration issues or invalid settings.
    pub fn new(token: impl Into<String>) -> Result<Self> {
        TodoistClientBuilder::new(token).build()
    }

    /// Creates a new TodoistClient with a custom base URL (for testing).
    ///
    /// The client is configured with default retry/timeout settings.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying HTTP client fails to build.
    pub fn with_base_url(token: impl Into<String>, base_url: impl Into<String>) -> Result<Self> {
        TodoistClientBuilder::new(token).base_url(base_url).build()
    }

    /// Returns a builder for creating a client with custom configuration.
    pub fn builder(token: impl Into<String>) -> TodoistClientBuilder {
        TodoistClientBuilder::new(token)
    }

    /// Returns the API token.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Returns a reference to the underlying HTTP client.
    pub fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    /// Returns the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns the maximum number of retries configured.
    pub fn max_retries(&self) -> u32 {
        self.retry_config.max_retries
    }

    /// Returns the initial backoff duration configured.
    pub fn initial_backoff(&self) -> Duration {
        self.retry_config.initial_backoff
    }

    /// Returns the maximum backoff duration configured.
    pub fn max_backoff(&self) -> Duration {
        self.retry_config.max_backoff
    }

    /// Calculates the backoff duration for a retry attempt.
    ///
    /// If `retry_after` is provided (from a 429 response), uses that value.
    /// Otherwise, uses exponential backoff: initial * 2^attempt, capped at max_backoff.
    fn calculate_backoff(&self, attempt: u32, retry_after: Option<u64>) -> Duration {
        let max_backoff_secs = self.retry_config.max_backoff.as_secs();
        if let Some(secs) = retry_after {
            // Use Retry-After header value, but cap it at max_backoff
            Duration::from_secs(secs.min(max_backoff_secs))
        } else {
            // Exponential backoff: initial * 2^attempt, capped at max_backoff
            let initial_secs = self.retry_config.initial_backoff.as_secs();
            let backoff_secs = initial_secs.saturating_mul(1 << attempt);
            Duration::from_secs(backoff_secs.min(max_backoff_secs))
        }
    }

    /// Performs a GET request to the given endpoint with automatic retry on rate limiting.
    ///
    /// # Arguments
    /// * `endpoint` - The API endpoint path (e.g., "/tasks", "/projects/123")
    ///
    /// # Returns
    /// The deserialized response body.
    pub async fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, endpoint);
        let max_retries = self.retry_config.max_retries;

        for attempt in 0..=max_retries {
            let response = self
                .http_client
                .get(&url)
                .bearer_auth(&self.token)
                .send()
                .await?;

            match self
                .handle_response_with_retry(response, attempt, max_retries)
                .await
            {
                Ok(RetryDecision::Success(value)) => return Ok(value),
                Ok(RetryDecision::Retry { retry_after }) => {
                    let backoff = self.calculate_backoff(attempt, retry_after);
                    sleep(backoff).await;
                }
                Err(e) => return Err(e),
            }
        }

        // All retries exhausted, return rate limit error
        Err(Error::Api(ApiError::RateLimit { retry_after: None }))
    }

    /// Performs a POST request to the given endpoint with a JSON body and automatic retry.
    ///
    /// # Arguments
    /// * `endpoint` - The API endpoint path
    /// * `body` - The request body to serialize as JSON
    ///
    /// # Returns
    /// The deserialized response body.
    pub async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, endpoint);
        let max_retries = self.retry_config.max_retries;

        for attempt in 0..=max_retries {
            let response = self
                .http_client
                .post(&url)
                .bearer_auth(&self.token)
                .json(body)
                .send()
                .await?;

            match self
                .handle_response_with_retry(response, attempt, max_retries)
                .await
            {
                Ok(RetryDecision::Success(value)) => return Ok(value),
                Ok(RetryDecision::Retry { retry_after }) => {
                    let backoff = self.calculate_backoff(attempt, retry_after);
                    sleep(backoff).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::Api(ApiError::RateLimit { retry_after: None }))
    }

    /// Performs a POST request without a body with automatic retry.
    ///
    /// # Arguments
    /// * `endpoint` - The API endpoint path
    ///
    /// # Returns
    /// The deserialized response body.
    pub async fn post_empty<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, endpoint);
        let max_retries = self.retry_config.max_retries;

        for attempt in 0..=max_retries {
            let response = self
                .http_client
                .post(&url)
                .bearer_auth(&self.token)
                .send()
                .await?;

            match self
                .handle_response_with_retry(response, attempt, max_retries)
                .await
            {
                Ok(RetryDecision::Success(value)) => return Ok(value),
                Ok(RetryDecision::Retry { retry_after }) => {
                    let backoff = self.calculate_backoff(attempt, retry_after);
                    sleep(backoff).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::Api(ApiError::RateLimit { retry_after: None }))
    }

    /// Performs a DELETE request to the given endpoint with automatic retry.
    ///
    /// # Arguments
    /// * `endpoint` - The API endpoint path
    ///
    /// # Returns
    /// Ok(()) on success.
    pub async fn delete(&self, endpoint: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, endpoint);
        let max_retries = self.retry_config.max_retries;

        for attempt in 0..=max_retries {
            let response = self
                .http_client
                .delete(&url)
                .bearer_auth(&self.token)
                .send()
                .await?;

            match self
                .handle_empty_response_with_retry(response, attempt, max_retries)
                .await
            {
                Ok(RetryDecision::Success(())) => return Ok(()),
                Ok(RetryDecision::Retry { retry_after }) => {
                    let backoff = self.calculate_backoff(attempt, retry_after);
                    sleep(backoff).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::Api(ApiError::RateLimit { retry_after: None }))
    }

    /// Performs a sync request to the Todoist Sync API.
    ///
    /// The Sync API is the primary mechanism for reading and writing data in Todoist.
    /// It supports:
    /// - Full sync (sync_token = "*") to retrieve all data
    /// - Incremental sync using a stored sync_token
    /// - Command execution for write operations
    ///
    /// # Arguments
    /// * `request` - The sync request containing sync_token, resource_types, and/or commands
    ///
    /// # Returns
    /// A `SyncResponse` containing the requested resources and command results.
    ///
    /// # Example
    /// ```no_run
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_api_rs::sync::SyncRequest;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = TodoistClient::new("your-api-token").unwrap();
    ///     let request = SyncRequest::full_sync();
    ///     let response = client.sync(request).await.unwrap();
    ///     println!("Got {} projects", response.projects.len());
    /// }
    /// ```
    pub async fn sync(&self, request: SyncRequest) -> Result<SyncResponse> {
        let url = format!("{}/sync", self.base_url);
        let max_retries = self.retry_config.max_retries;

        for attempt in 0..=max_retries {
            let response = self
                .http_client
                .post(&url)
                .bearer_auth(&self.token)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(request.to_form_body())
                .send()
                .await?;

            match self
                .handle_response_with_retry(response, attempt, max_retries)
                .await
            {
                Ok(RetryDecision::Success(value)) => return Ok(value),
                Ok(RetryDecision::Retry { retry_after }) => {
                    let backoff = self.calculate_backoff(attempt, retry_after);
                    sleep(backoff).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::Api(ApiError::RateLimit { retry_after: None }))
    }

    /// Creates a task using the Quick Add endpoint with NLP parsing.
    ///
    /// The Quick Add endpoint parses natural language input to extract project,
    /// labels, priority, due date, etc., using the same syntax as Todoist's quick add.
    ///
    /// # Arguments
    /// * `request` - The quick add request containing the text to parse
    ///
    /// # Returns
    /// A `QuickAddResponse` containing the created task and parsed metadata.
    ///
    /// # Example
    /// ```no_run
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_api_rs::quick_add::QuickAddRequest;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = TodoistClient::new("your-api-token").unwrap();
    ///     let request = QuickAddRequest::new("Buy milk tomorrow #Shopping p2 @errands").unwrap();
    ///     let response = client.quick_add(request).await.unwrap();
    ///     println!("Created task: {} in project {}", response.content, response.project_id);
    /// }
    /// ```
    pub async fn quick_add(&self, request: QuickAddRequest) -> Result<QuickAddResponse> {
        let url = format!("{}/tasks/quick", self.base_url);
        let max_retries = self.retry_config.max_retries;

        for attempt in 0..=max_retries {
            let response = self
                .http_client
                .post(&url)
                .bearer_auth(&self.token)
                .json(&request)
                .send()
                .await?;

            match self
                .handle_response_with_retry(response, attempt, max_retries)
                .await
            {
                Ok(RetryDecision::Success(value)) => return Ok(value),
                Ok(RetryDecision::Retry { retry_after }) => {
                    let backoff = self.calculate_backoff(attempt, retry_after);
                    sleep(backoff).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::Api(ApiError::RateLimit { retry_after: None }))
    }

    /// Handles the HTTP response, returning a retry decision or error.
    async fn handle_response_with_retry<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
        attempt: u32,
        max_retries: u32,
    ) -> Result<RetryDecision<T>> {
        let status = response.status();

        if status.is_success() {
            let body = response.json::<T>().await?;
            return Ok(RetryDecision::Success(body));
        }

        // Check for rate limiting (429)
        if status.as_u16() == 429 && attempt < max_retries {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            return Ok(RetryDecision::Retry { retry_after });
        }

        // Non-retryable error or max retries exceeded
        Err(self.parse_error_response(response).await)
    }

    /// Handles empty responses (e.g., DELETE), returning a retry decision or error.
    async fn handle_empty_response_with_retry(
        &self,
        response: reqwest::Response,
        attempt: u32,
        max_retries: u32,
    ) -> Result<RetryDecision<()>> {
        let status = response.status();

        if status.is_success() {
            return Ok(RetryDecision::Success(()));
        }

        // Check for rate limiting (429)
        if status.as_u16() == 429 && attempt < max_retries {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            return Ok(RetryDecision::Retry { retry_after });
        }

        Err(self.parse_error_response(response).await)
    }

    /// Parses an error response into our error types.
    async fn parse_error_response(&self, response: reqwest::Response) -> Error {
        let status = response.status();
        let status_code = status.as_u16();

        // Extract retry-after header for rate limiting
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());

        // Try to get error message from body
        let message = response.text().await.unwrap_or_default();

        let api_error = match status_code {
            401 | 403 => ApiError::Auth {
                message: if message.is_empty() {
                    "Authentication failed".to_string()
                } else {
                    message
                },
            },
            404 => ApiError::NotFound {
                resource: "resource".to_string(),
                id: "unknown".to_string(),
            },
            429 => ApiError::RateLimit { retry_after },
            400 => ApiError::Validation {
                field: None,
                message: if message.is_empty() {
                    "Bad request".to_string()
                } else {
                    message
                },
            },
            _ => ApiError::Http {
                status: status_code,
                message: if message.is_empty() {
                    status
                        .canonical_reason()
                        .unwrap_or("Unknown error")
                        .to_string()
                } else {
                    message
                },
            },
        };

        Error::Api(api_error)
    }
}

/// Decision type for retry logic.
enum RetryDecision<T> {
    /// Request succeeded with this value.
    Success(T),
    /// Request should be retried.
    Retry { retry_after: Option<u64> },
}

impl fmt::Debug for TodoistClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TodoistClient")
            .field("token", &"[REDACTED]")
            .field("http_client", &self.http_client)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let client = TodoistClient::builder("test-token").max_retries(10).build().unwrap();

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
}

#[cfg(test)]
mod wiremock_tests {
    use super::*;
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
