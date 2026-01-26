//! Error types for the Todoist API client.

use std::fmt;

/// Errors that can occur when interacting with the Todoist API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// HTTP-level error with status code.
    Http { status: u16, message: String },
    /// Authentication failure.
    Auth { message: String },
    /// Rate limit exceeded.
    RateLimit { retry_after: Option<u64> },
    /// Resource not found.
    NotFound { resource: String, id: String },
    /// API validation error.
    Validation {
        field: Option<String>,
        message: String,
    },
    /// Network/connection error.
    Network { message: String },
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Http { status, message } => write!(f, "HTTP error {}: {}", status, message),
            ApiError::Auth { message } => write!(f, "Auth error: {}", message),
            ApiError::RateLimit { retry_after } => match retry_after {
                Some(secs) => write!(f, "Rate limited, retry after {} seconds", secs),
                None => write!(f, "Rate limited"),
            },
            ApiError::NotFound { resource, id } => {
                write!(f, "{} not found: {}", resource, id)
            }
            ApiError::Validation { field, message } => match field {
                Some(f_name) => write!(f, "Validation error on {}: {}", f_name, message),
                None => write!(f, "Validation error: {}", message),
            },
            ApiError::Network { message } => write!(f, "Network error: {}", message),
        }
    }
}

impl std::error::Error for ApiError {}

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
}
