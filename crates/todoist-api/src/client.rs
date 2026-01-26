//! HTTP client wrapper for the Todoist API.

use std::fmt;

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{ApiError, Error, Result};

/// Base URL for the Todoist API v1.
const BASE_URL: &str = "https://api.todoist.com/api/v1";

/// Client for interacting with the Todoist API.
#[derive(Clone)]
pub struct TodoistClient {
    token: String,
    http_client: reqwest::Client,
    base_url: String,
}

impl TodoistClient {
    /// Creates a new TodoistClient with the given API token.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            http_client: reqwest::Client::new(),
            base_url: BASE_URL.to_string(),
        }
    }

    /// Creates a new TodoistClient with a custom base URL (for testing).
    #[cfg(test)]
    pub fn with_base_url(token: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            http_client: reqwest::Client::new(),
            base_url: base_url.into(),
        }
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

    /// Performs a GET request to the given endpoint.
    ///
    /// # Arguments
    /// * `endpoint` - The API endpoint path (e.g., "/tasks", "/projects/123")
    ///
    /// # Returns
    /// The deserialized response body.
    pub async fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, endpoint);

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Performs a POST request to the given endpoint with a JSON body.
    ///
    /// # Arguments
    /// * `endpoint` - The API endpoint path
    /// * `body` - The request body to serialize as JSON
    ///
    /// # Returns
    /// The deserialized response body.
    pub async fn post<T: DeserializeOwned, B: Serialize>(&self, endpoint: &str, body: &B) -> Result<T> {
        let url = format!("{}{}", self.base_url, endpoint);

        let response = self
            .http_client
            .post(&url)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Performs a POST request without a body (for endpoints like /tasks/{id}/close).
    ///
    /// # Arguments
    /// * `endpoint` - The API endpoint path
    ///
    /// # Returns
    /// The deserialized response body.
    pub async fn post_empty<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, endpoint);

        let response = self
            .http_client
            .post(&url)
            .bearer_auth(&self.token)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Performs a DELETE request to the given endpoint.
    ///
    /// # Arguments
    /// * `endpoint` - The API endpoint path
    ///
    /// # Returns
    /// Ok(()) on success.
    pub async fn delete(&self, endpoint: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, endpoint);

        let response = self
            .http_client
            .delete(&url)
            .bearer_auth(&self.token)
            .send()
            .await?;

        // DELETE typically returns 204 No Content
        self.handle_empty_response(response).await
    }

    /// Handles the HTTP response, converting it to our error types.
    async fn handle_response<T: DeserializeOwned>(&self, response: reqwest::Response) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            let body = response.json::<T>().await?;
            return Ok(body);
        }

        // Handle error responses
        Err(self.parse_error_response(response).await)
    }

    /// Handles responses that should have no body (e.g., 204 No Content).
    async fn handle_empty_response(&self, response: reqwest::Response) -> Result<()> {
        let status = response.status();

        if status.is_success() {
            return Ok(());
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
                    status.canonical_reason().unwrap_or("Unknown error").to_string()
                } else {
                    message
                },
            },
        };

        Error::Api(api_error)
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
        let client = TodoistClient::new(token);
        let _ = client;
    }

    // Test: TodoistClient should store the token for later use
    #[test]
    fn test_todoist_client_stores_token() {
        let token = "my-secret-token";
        let client = TodoistClient::new(token);
        assert_eq!(client.token(), token);
    }

    // Test: TodoistClient should hold a reqwest client internally
    #[test]
    fn test_todoist_client_has_http_client() {
        let client = TodoistClient::new("test-token");
        let _http_client = client.http_client();
    }

    // Test: TodoistClient should implement Clone
    #[test]
    fn test_todoist_client_is_clone() {
        let client = TodoistClient::new("test-token");
        let _cloned = client.clone();
    }

    // Test: TodoistClient should implement Debug
    #[test]
    fn test_todoist_client_is_debug() {
        let client = TodoistClient::new("test-token");
        let debug_str = format!("{:?}", client);
        assert!(!debug_str.contains("test-token"), "Token should be redacted in debug output");
    }

    // Test: TodoistClient should use the default base URL
    #[test]
    fn test_todoist_client_default_base_url() {
        let client = TodoistClient::new("test-token");
        assert_eq!(client.base_url(), BASE_URL);
    }

    // Test: TodoistClient can be created with custom base URL
    #[test]
    fn test_todoist_client_with_custom_base_url() {
        let client = TodoistClient::with_base_url("test-token", "https://test.example.com");
        assert_eq!(client.base_url(), "https://test.example.com");
    }
}
