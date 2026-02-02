//! HTTP client wrapper for the Todoist API.

use std::fmt;
use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};

use crate::error::Result;
use crate::quick_add::{QuickAddRequest, QuickAddResponse};
use crate::retry::{
    execute_empty_with_retry, execute_with_retry, RetryConfig, DEFAULT_INITIAL_BACKOFF_SECS,
    DEFAULT_MAX_BACKOFF_SECS, DEFAULT_MAX_RETRIES,
};
use crate::sync::{SyncRequest, SyncResponse};

/// Base URL for the Todoist API v1.
const BASE_URL: &str = "https://api.todoist.com/api/v1";

/// Default request timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

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
            .map_err(crate::error::Error::Http)?;

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
    #[cfg(test)]
    fn calculate_backoff(&self, attempt: u32, retry_after: Option<u64>) -> Duration {
        self.retry_config.calculate_backoff(attempt, retry_after)
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
        let http_client = self.http_client.clone();
        let token = self.token.clone();

        execute_with_retry(&self.retry_config, || {
            let url = url.clone();
            let http_client = http_client.clone();
            let token = token.clone();
            async move {
                http_client
                    .get(&url)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(crate::error::Error::Http)
            }
        })
        .await
    }

    /// Performs a POST request to the given endpoint with a JSON body and automatic retry.
    ///
    /// # Arguments
    /// * `endpoint` - The API endpoint path
    /// * `body` - The request body to serialize as JSON
    ///
    /// # Returns
    /// The deserialized response body.
    pub async fn post<T: DeserializeOwned, B: Serialize + Clone>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, endpoint);
        let http_client = self.http_client.clone();
        let token = self.token.clone();
        let body = body.clone();

        execute_with_retry(&self.retry_config, || {
            let url = url.clone();
            let http_client = http_client.clone();
            let token = token.clone();
            let body = body.clone();
            async move {
                http_client
                    .post(&url)
                    .bearer_auth(&token)
                    .json(&body)
                    .send()
                    .await
                    .map_err(crate::error::Error::Http)
            }
        })
        .await
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
        let http_client = self.http_client.clone();
        let token = self.token.clone();

        execute_with_retry(&self.retry_config, || {
            let url = url.clone();
            let http_client = http_client.clone();
            let token = token.clone();
            async move {
                http_client
                    .post(&url)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(crate::error::Error::Http)
            }
        })
        .await
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
        let http_client = self.http_client.clone();
        let token = self.token.clone();

        execute_empty_with_retry(&self.retry_config, || {
            let url = url.clone();
            let http_client = http_client.clone();
            let token = token.clone();
            async move {
                http_client
                    .delete(&url)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(crate::error::Error::Http)
            }
        })
        .await
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
        let http_client = self.http_client.clone();
        let token = self.token.clone();
        let form_body = request.to_form_body();

        execute_with_retry(&self.retry_config, || {
            let url = url.clone();
            let http_client = http_client.clone();
            let token = token.clone();
            let form_body = form_body.clone();
            async move {
                http_client
                    .post(&url)
                    .bearer_auth(&token)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(form_body)
                    .send()
                    .await
                    .map_err(crate::error::Error::Http)
            }
        })
        .await
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
        let http_client = self.http_client.clone();
        let token = self.token.clone();

        execute_with_retry(&self.retry_config, || {
            let url = url.clone();
            let http_client = http_client.clone();
            let token = token.clone();
            let request = request.clone();
            async move {
                http_client
                    .post(&url)
                    .bearer_auth(&token)
                    .json(&request)
                    .send()
                    .await
                    .map_err(crate::error::Error::Http)
            }
        })
        .await
    }
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
#[path = "client_tests.rs"]
mod tests;
