//! HTTP client wrapper for the Todoist API.

use std::fmt;

/// Client for interacting with the Todoist API.
#[derive(Clone)]
pub struct TodoistClient {
    token: String,
    http_client: reqwest::Client,
}

impl TodoistClient {
    /// Creates a new TodoistClient with the given API token.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            http_client: reqwest::Client::new(),
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
    // Test: TodoistClient struct should exist and hold an API token
    #[test]
    fn test_todoist_client_struct_exists() {
        // TodoistClient should be a struct that can be constructed
        use super::TodoistClient;

        let _client: TodoistClient;
    }

    // Test: TodoistClient::new() constructor should accept a token string
    #[test]
    fn test_todoist_client_new_accepts_token() {
        use super::TodoistClient;

        let token = "test-api-token-12345";
        let client = TodoistClient::new(token);

        // Client should be constructed successfully
        let _ = client;
    }

    // Test: TodoistClient should store the token for later use
    #[test]
    fn test_todoist_client_stores_token() {
        use super::TodoistClient;

        let token = "my-secret-token";
        let client = TodoistClient::new(token);

        // The client should have a method to access the token (for auth header construction)
        assert_eq!(client.token(), token);
    }

    // Test: TodoistClient should hold a reqwest client internally
    #[test]
    fn test_todoist_client_has_http_client() {
        use super::TodoistClient;

        let client = TodoistClient::new("test-token");

        // The client should provide access to the underlying HTTP client
        let _http_client = client.http_client();
    }

    // Test: TodoistClient should implement Clone
    #[test]
    fn test_todoist_client_is_clone() {
        use super::TodoistClient;

        let client = TodoistClient::new("test-token");
        let _cloned = client.clone();
    }

    // Test: TodoistClient should implement Debug
    #[test]
    fn test_todoist_client_is_debug() {
        use super::TodoistClient;

        let client = TodoistClient::new("test-token");

        // Should be able to format as debug (but token should be redacted)
        let debug_str = format!("{:?}", client);

        // The debug output should NOT contain the actual token (security)
        assert!(!debug_str.contains("test-token"), "Token should be redacted in debug output");
    }
}
