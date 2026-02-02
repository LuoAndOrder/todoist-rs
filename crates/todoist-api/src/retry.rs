//! Retry logic for HTTP requests with exponential backoff.

use std::time::Duration;

use serde::de::DeserializeOwned;
use tokio::time::sleep;

use crate::error::{ApiError, Error, Result};

/// Default initial backoff duration for retries (1 second).
pub(crate) const DEFAULT_INITIAL_BACKOFF_SECS: u64 = 1;

/// Default maximum backoff duration for retries (30 seconds).
pub(crate) const DEFAULT_MAX_BACKOFF_SECS: u64 = 30;

/// Default maximum number of retry attempts.
pub(crate) const DEFAULT_MAX_RETRIES: u32 = 3;

/// Configuration for retry behavior.
#[derive(Clone, Debug)]
pub(crate) struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Initial backoff duration for retries.
    pub initial_backoff: Duration,
    /// Maximum backoff duration for retries.
    pub max_backoff: Duration,
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

impl RetryConfig {
    /// Calculates the backoff duration for a retry attempt.
    ///
    /// If `retry_after` is provided (from a 429 response), uses that value.
    /// Otherwise, uses exponential backoff: initial * 2^attempt, capped at max_backoff.
    pub fn calculate_backoff(&self, attempt: u32, retry_after: Option<u64>) -> Duration {
        let max_backoff_secs = self.max_backoff.as_secs();
        if let Some(secs) = retry_after {
            // Use Retry-After header value, but cap it at max_backoff
            Duration::from_secs(secs.min(max_backoff_secs))
        } else {
            // Exponential backoff: initial * 2^attempt, capped at max_backoff
            let initial_secs = self.initial_backoff.as_secs();
            let backoff_secs = initial_secs.saturating_mul(1 << attempt);
            Duration::from_secs(backoff_secs.min(max_backoff_secs))
        }
    }
}

/// Decision type for retry logic.
pub(crate) enum RetryDecision<T> {
    /// Request succeeded with this value.
    Success(T),
    /// Request should be retried.
    Retry { retry_after: Option<u64> },
}

/// Handles the HTTP response, returning a retry decision or error.
pub(crate) async fn handle_response_with_retry<T: DeserializeOwned>(
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
    Err(parse_error_response(response).await)
}

/// Handles empty responses (e.g., DELETE), returning a retry decision or error.
pub(crate) async fn handle_empty_response_with_retry(
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

    Err(parse_error_response(response).await)
}

/// Parses an error response into our error types.
pub(crate) async fn parse_error_response(response: reqwest::Response) -> Error {
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

/// Executes a request with retry logic.
pub(crate) async fn execute_with_retry<T, F, Fut>(
    config: &RetryConfig,
    mut make_request: F,
) -> Result<T>
where
    T: DeserializeOwned,
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Response>>,
{
    for attempt in 0..=config.max_retries {
        let response = make_request().await?;

        match handle_response_with_retry(response, attempt, config.max_retries).await {
            Ok(RetryDecision::Success(value)) => return Ok(value),
            Ok(RetryDecision::Retry { retry_after }) => {
                let backoff = config.calculate_backoff(attempt, retry_after);
                sleep(backoff).await;
            }
            Err(e) => return Err(e),
        }
    }

    // All retries exhausted, return rate limit error
    Err(Error::Api(ApiError::RateLimit { retry_after: None }))
}

/// Executes a request that returns an empty response with retry logic.
pub(crate) async fn execute_empty_with_retry<F, Fut>(
    config: &RetryConfig,
    mut make_request: F,
) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Response>>,
{
    for attempt in 0..=config.max_retries {
        let response = make_request().await?;

        match handle_empty_response_with_retry(response, attempt, config.max_retries).await {
            Ok(RetryDecision::Success(())) => return Ok(()),
            Ok(RetryDecision::Retry { retry_after }) => {
                let backoff = config.calculate_backoff(attempt, retry_after);
                sleep(backoff).await;
            }
            Err(e) => return Err(e),
        }
    }

    Err(Error::Api(ApiError::RateLimit { retry_after: None }))
}
