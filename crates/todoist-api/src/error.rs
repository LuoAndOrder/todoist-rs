//! Error types for the Todoist API client.

use thiserror::Error;

/// Top-level error type for the Todoist API client.
///
/// This wraps all possible errors that can occur when using the client,
/// including API-specific errors, network errors, and serialization errors.
#[derive(Debug, Error)]
pub enum Error {
    /// An API-specific error (auth failure, rate limiting, validation, etc.)
    #[error(transparent)]
    Api(#[from] ApiError),

    /// An HTTP request/response error from reqwest.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Internal/unexpected error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias using our Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// API-specific errors from the Todoist API.
///
/// These represent errors returned by the API itself (not transport-level errors).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApiError {
    /// HTTP-level error with status code (for unexpected status codes).
    #[error("HTTP error {status}: {message}")]
    Http {
        /// HTTP status code
        status: u16,
        /// Error message from the response
        message: String,
    },

    /// Authentication failure (401 Unauthorized, 403 Forbidden).
    #[error("Authentication failed: {message}")]
    Auth {
        /// Descriptive error message
        message: String,
    },

    /// Rate limit exceeded (429 Too Many Requests).
    #[error("{}", match .retry_after {
        Some(secs) => format!("Rate limited, retry after {} seconds", secs),
        None => "Rate limited".to_string(),
    })]
    RateLimit {
        /// Seconds to wait before retrying (from Retry-After header)
        retry_after: Option<u64>,
    },

    /// Resource not found (404 Not Found).
    #[error("{resource} not found: {id}")]
    NotFound {
        /// Type of resource (e.g., "task", "project")
        resource: String,
        /// ID of the resource that was not found
        id: String,
    },

    /// API validation error (400 Bad Request with validation details).
    #[error("{}", match .field {
        Some(f) => format!("Validation error on {}: {}", f, .message),
        None => format!("Validation error: {}", .message),
    })]
    Validation {
        /// Field that failed validation (if known)
        field: Option<String>,
        /// Validation error message
        message: String,
    },

    /// Network/connection error.
    #[error("Network error: {message}")]
    Network {
        /// Descriptive error message
        message: String,
    },
}

impl Error {
    /// Returns true if this error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Api(api_err) => api_err.is_retryable(),
            Error::Http(req_err) => req_err.is_timeout() || req_err.is_connect(),
            Error::Json(_) => false,
            Error::Internal(_) => false,
        }
    }

    /// Returns the appropriate CLI exit code for this error.
    ///
    /// Exit codes follow the spec:
    /// - 2: API error (auth failure, not found, validation error)
    /// - 3: Network error (connection failed, timeout)
    /// - 4: Rate limited (with retry-after information)
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Api(api_err) => api_err.exit_code(),
            Error::Http(req_err) => {
                if req_err.is_timeout() || req_err.is_connect() {
                    3 // Network error
                } else {
                    2 // API error
                }
            }
            Error::Json(_) => 2, // API error (bad response)
            Error::Internal(_) => 2, // Treat as API error
        }
    }

    /// Returns the underlying API error if this is an API error variant.
    pub fn as_api_error(&self) -> Option<&ApiError> {
        match self {
            Error::Api(api_err) => Some(api_err),
            _ => None,
        }
    }
}

impl ApiError {
    /// Returns true if this error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(self, ApiError::RateLimit { .. } | ApiError::Network { .. })
    }

    /// Returns the appropriate CLI exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            ApiError::Network { .. } => 3,
            ApiError::RateLimit { .. } => 4,
            _ => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_http_variant_exists() {
        // ApiError should have an Http variant for HTTP-level errors
        let status = 500;
        let message = "Internal Server Error".to_string();
        let error = ApiError::Http { status, message };

        match error {
            ApiError::Http {
                status: s,
                message: m,
            } => {
                assert_eq!(s, 500);
                assert_eq!(m, "Internal Server Error");
            }
            _ => panic!("Expected Http variant"),
        }
    }

    #[test]
    fn test_api_error_auth_variant_exists() {
        // ApiError should have an Auth variant for authentication failures
        let error = ApiError::Auth {
            message: "Invalid token".to_string(),
        };

        match error {
            ApiError::Auth { message } => {
                assert_eq!(message, "Invalid token");
            }
            _ => panic!("Expected Auth variant"),
        }
    }

    #[test]
    fn test_api_error_rate_limit_variant_exists() {
        // ApiError should have a RateLimit variant with optional retry_after
        let error = ApiError::RateLimit {
            retry_after: Some(30),
        };

        match error {
            ApiError::RateLimit { retry_after } => {
                assert_eq!(retry_after, Some(30));
            }
            _ => panic!("Expected RateLimit variant"),
        }
    }

    #[test]
    fn test_api_error_not_found_variant_exists() {
        // ApiError should have a NotFound variant for 404 responses
        let error = ApiError::NotFound {
            resource: "task".to_string(),
            id: "abc123".to_string(),
        };

        match error {
            ApiError::NotFound { resource, id } => {
                assert_eq!(resource, "task");
                assert_eq!(id, "abc123");
            }
            _ => panic!("Expected NotFound variant"),
        }
    }

    #[test]
    fn test_api_error_validation_variant_exists() {
        // ApiError should have a Validation variant for API validation errors
        let error = ApiError::Validation {
            field: Some("due_date".to_string()),
            message: "Invalid date format".to_string(),
        };

        match error {
            ApiError::Validation { field, message } => {
                assert_eq!(field, Some("due_date".to_string()));
                assert_eq!(message, "Invalid date format");
            }
            _ => panic!("Expected Validation variant"),
        }
    }

    #[test]
    fn test_api_error_network_variant_exists() {
        // ApiError should have a Network variant for connection issues
        let error = ApiError::Network {
            message: "Connection refused".to_string(),
        };

        match error {
            ApiError::Network { message } => {
                assert_eq!(message, "Connection refused");
            }
            _ => panic!("Expected Network variant"),
        }
    }

    #[test]
    fn test_api_error_implements_std_error() {
        // ApiError should implement std::error::Error
        let error: Box<dyn std::error::Error> = Box::new(ApiError::Network {
            message: "timeout".to_string(),
        });
        assert!(error.to_string().contains("timeout"));
    }

    #[test]
    fn test_api_error_display_http() {
        let error = ApiError::Http {
            status: 503,
            message: "Service Unavailable".to_string(),
        };
        let display = error.to_string();
        assert!(display.contains("503") || display.contains("Service Unavailable"));
    }

    #[test]
    fn test_api_error_display_auth() {
        let error = ApiError::Auth {
            message: "Token expired".to_string(),
        };
        let display = error.to_string();
        assert!(display.to_lowercase().contains("auth") || display.contains("Token expired"));
    }

    #[test]
    fn test_api_error_display_rate_limit() {
        let error = ApiError::RateLimit {
            retry_after: Some(60),
        };
        let display = error.to_string();
        assert!(display.to_lowercase().contains("rate") || display.contains("60"));
    }

    #[test]
    fn test_api_error_display_not_found() {
        let error = ApiError::NotFound {
            resource: "project".to_string(),
            id: "xyz789".to_string(),
        };
        let display = error.to_string();
        assert!(
            display.contains("project")
                || display.contains("xyz789")
                || display.to_lowercase().contains("not found")
        );
    }

    #[test]
    fn test_api_error_display_validation() {
        let error = ApiError::Validation {
            field: Some("priority".to_string()),
            message: "Must be between 1 and 4".to_string(),
        };
        let display = error.to_string();
        assert!(display.contains("priority") || display.contains("Must be between 1 and 4"));
    }

    #[test]
    fn test_api_error_display_network() {
        let error = ApiError::Network {
            message: "DNS lookup failed".to_string(),
        };
        let display = error.to_string();
        assert!(
            display.contains("DNS lookup failed") || display.to_lowercase().contains("network")
        );
    }

    #[test]
    fn test_api_error_is_retryable_rate_limit() {
        // Rate limit errors should be retryable
        let error = ApiError::RateLimit {
            retry_after: Some(5),
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn test_api_error_is_retryable_network() {
        // Network errors should be retryable
        let error = ApiError::Network {
            message: "Connection reset".to_string(),
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn test_api_error_is_not_retryable_auth() {
        // Auth errors should not be retryable
        let error = ApiError::Auth {
            message: "Invalid credentials".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_api_error_is_not_retryable_not_found() {
        // NotFound errors should not be retryable
        let error = ApiError::NotFound {
            resource: "task".to_string(),
            id: "123".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_api_error_is_not_retryable_validation() {
        // Validation errors should not be retryable
        let error = ApiError::Validation {
            field: None,
            message: "Invalid request".to_string(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_api_error_exit_code_auth() {
        // Auth errors should map to exit code 2
        let error = ApiError::Auth {
            message: "Unauthorized".to_string(),
        };
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn test_api_error_exit_code_not_found() {
        // NotFound errors should map to exit code 2 (API error)
        let error = ApiError::NotFound {
            resource: "task".to_string(),
            id: "abc".to_string(),
        };
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn test_api_error_exit_code_validation() {
        // Validation errors should map to exit code 2 (API error)
        let error = ApiError::Validation {
            field: Some("content".to_string()),
            message: "Required".to_string(),
        };
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn test_api_error_exit_code_network() {
        // Network errors should map to exit code 3
        let error = ApiError::Network {
            message: "Timeout".to_string(),
        };
        assert_eq!(error.exit_code(), 3);
    }

    #[test]
    fn test_api_error_exit_code_rate_limit() {
        // Rate limit errors should map to exit code 4
        let error = ApiError::RateLimit { retry_after: None };
        assert_eq!(error.exit_code(), 4);
    }

    #[test]
    fn test_api_error_exit_code_http() {
        // Generic HTTP errors should map to exit code 2 (API error)
        let error = ApiError::Http {
            status: 500,
            message: "Server error".to_string(),
        };
        assert_eq!(error.exit_code(), 2);
    }

    // Tests for the top-level Error type

    #[test]
    fn test_error_from_api_error() {
        let api_error = ApiError::Auth {
            message: "test".to_string(),
        };
        let error: Error = api_error.into();
        assert!(matches!(error, Error::Api(_)));
    }

    #[test]
    fn test_error_api_variant_is_retryable() {
        let error: Error = ApiError::RateLimit {
            retry_after: Some(5),
        }
        .into();
        assert!(error.is_retryable());
    }

    #[test]
    fn test_error_api_variant_not_retryable() {
        let error: Error = ApiError::Auth {
            message: "bad token".to_string(),
        }
        .into();
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_error_json_not_retryable() {
        // Create a JSON error by trying to parse invalid JSON
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let error: Error = json_err.into();
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_error_internal_not_retryable() {
        let error = Error::Internal("something went wrong".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_error_exit_code_api() {
        let error: Error = ApiError::RateLimit { retry_after: None }.into();
        assert_eq!(error.exit_code(), 4);

        let error: Error = ApiError::Network {
            message: "timeout".to_string(),
        }
        .into();
        assert_eq!(error.exit_code(), 3);

        let error: Error = ApiError::Auth {
            message: "bad".to_string(),
        }
        .into();
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn test_error_exit_code_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
        let error: Error = json_err.into();
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn test_error_exit_code_internal() {
        let error = Error::Internal("panic".to_string());
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn test_error_as_api_error() {
        let api_error = ApiError::NotFound {
            resource: "task".to_string(),
            id: "123".to_string(),
        };
        let error: Error = api_error.clone().into();
        assert_eq!(error.as_api_error(), Some(&api_error));
    }

    #[test]
    fn test_error_as_api_error_none() {
        let error = Error::Internal("test".to_string());
        assert_eq!(error.as_api_error(), None);
    }

    #[test]
    fn test_error_display_api() {
        let error: Error = ApiError::Auth {
            message: "Invalid token".to_string(),
        }
        .into();
        let display = error.to_string();
        assert!(display.contains("Invalid token"));
    }

    #[test]
    fn test_error_display_internal() {
        let error = Error::Internal("unexpected state".to_string());
        let display = error.to_string();
        assert!(display.contains("unexpected state"));
    }

    #[test]
    fn test_error_implements_std_error() {
        let error: Box<dyn std::error::Error> = Box::new(Error::Internal("test".to_string()));
        assert!(error.to_string().contains("test"));
    }

    #[test]
    fn test_result_type_alias() {
        fn returns_result() -> Result<i32> {
            Ok(42)
        }
        assert_eq!(returns_result().unwrap(), 42);
    }

    #[test]
    fn test_result_type_alias_error() {
        fn returns_error() -> Result<i32> {
            Err(Error::Internal("failed".to_string()))
        }
        assert!(returns_error().is_err());
    }
}
