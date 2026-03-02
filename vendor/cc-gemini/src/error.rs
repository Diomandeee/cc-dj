//! Comprehensive error types for Gemini API operations.
//!
//! This module provides a rich error taxonomy that enables precise error handling
//! and recovery strategies. All error variants include contextual information
//! to aid debugging and operational monitoring.
//!
//! # Error Categories
//!
//! - **Authentication**: API key issues, invalid credentials
//! - **Rate Limiting**: RPM/TPM exhaustion, quota exceeded
//! - **Cost Control**: Budget limits exceeded
//! - **Content Safety**: Blocked by safety filters
//! - **Network**: Connectivity, timeout, TLS issues
//! - **API**: Server errors, malformed responses
//! - **Configuration**: Invalid settings, missing required fields
//!
//! # Example
//!
//! ```rust,ignore
//! use cc_gemini::{GeminiClient, GeminiError};
//!
//! async fn analyze_with_retry(client: &GeminiClient, image: &[u8]) -> Result<String, GeminiError> {
//!     match client.analyze_image(image, "image/jpeg", "Describe this image").await {
//!         Ok(result) => Ok(result.text),
//!         Err(GeminiError::RateLimitExceeded { retry_after }) => {
//!             if let Some(duration) = retry_after {
//!                 tokio::time::sleep(duration).await;
//!                 // Retry logic...
//!             }
//!             Err(GeminiError::RateLimitExceeded { retry_after })
//!         }
//!         Err(e) => Err(e),
//!     }
//! }
//! ```

use std::time::Duration;

/// Result type alias for Gemini operations.
pub type Result<T> = std::result::Result<T, GeminiError>;

/// Comprehensive error type for Gemini API operations.
///
/// Each variant includes contextual information to enable precise error handling
/// and debugging. The error type implements `std::error::Error` and provides
/// human-readable messages via `Display`.
#[derive(Debug, thiserror::Error)]
pub enum GeminiError {
    // =========================================================================
    // Authentication Errors
    // =========================================================================
    /// API key is invalid, expired, or lacks required permissions.
    ///
    /// # Recovery
    /// - Verify the API key is correct and not expired
    /// - Check that the key has access to the requested model
    /// - Ensure billing is enabled on the associated project
    #[error("authentication failed: {message}")]
    AuthenticationError {
        /// Detailed message explaining the authentication failure.
        message: String,
    },

    // =========================================================================
    // Rate Limiting Errors
    // =========================================================================
    /// Request was rejected due to rate limit exhaustion.
    ///
    /// Gemini enforces both requests-per-minute (RPM) and tokens-per-minute (TPM)
    /// limits. This error indicates one or both limits were exceeded.
    ///
    /// # Recovery
    /// - Wait for `retry_after` duration before retrying
    /// - Implement exponential backoff if `retry_after` is not provided
    /// - Consider reducing request concurrency
    #[error("rate limit exceeded{}", retry_after.map(|d| format!(", retry after {:?}", d)).unwrap_or_default())]
    RateLimitExceeded {
        /// Suggested wait duration before retrying.
        /// If `None`, use exponential backoff starting at 1 second.
        retry_after: Option<Duration>,
    },

    /// Daily or monthly quota has been exhausted.
    ///
    /// # Recovery
    /// - Wait until the quota resets (typically daily or monthly)
    /// - Request a quota increase from Google Cloud
    #[error("quota exhausted: {message}")]
    QuotaExhausted {
        /// Details about the exhausted quota.
        message: String,
    },

    // =========================================================================
    // Cost Control Errors
    // =========================================================================
    /// Request would exceed the configured cost limit.
    ///
    /// This error is raised proactively before making API calls to prevent
    /// unexpected charges.
    ///
    /// # Recovery
    /// - Increase the cost limit if appropriate
    /// - Reset the cost tracker for a new session
    /// - Reduce the scope of analysis (fewer frames, lower resolution)
    #[error("cost limit exceeded: ${current:.4} >= ${limit:.4}")]
    CostLimitExceeded {
        /// Current accumulated cost in USD.
        current: f64,
        /// Configured cost limit in USD.
        limit: f64,
    },

    // =========================================================================
    // Content Safety Errors
    // =========================================================================
    /// Request or response was blocked by Gemini's safety filters.
    ///
    /// # Recovery
    /// - Review the content for policy violations
    /// - Adjust safety settings if appropriate for your use case
    /// - Skip the problematic content and continue with other items
    #[error("content blocked by safety filter: {reason}")]
    ContentBlocked {
        /// The safety category that triggered the block.
        reason: String,
        /// The specific safety category (if available).
        category: Option<String>,
    },

    /// Response was truncated due to content safety concerns.
    #[error("response truncated: {reason}")]
    ResponseTruncated {
        /// Reason for truncation.
        reason: String,
        /// Partial response that was generated before truncation.
        partial_response: Option<String>,
    },

    // =========================================================================
    // Model Errors
    // =========================================================================
    /// The requested model is not available or does not exist.
    ///
    /// # Recovery
    /// - Verify the model name is spelled correctly
    /// - Check that the model is available in your region
    /// - Use a fallback model if available
    #[error("model unavailable: {model}")]
    ModelUnavailable {
        /// The model that was requested.
        model: String,
    },

    /// The model does not support the requested capability.
    #[error("unsupported capability for model {model}: {capability}")]
    UnsupportedCapability {
        /// The model being used.
        model: String,
        /// The capability that is not supported.
        capability: String,
    },

    // =========================================================================
    // Request Validation Errors
    // =========================================================================
    /// The request is malformed or contains invalid parameters.
    ///
    /// # Recovery
    /// - Review the error message for specific validation failures
    /// - Ensure all required fields are provided
    /// - Verify data types and formats are correct
    #[error("invalid request: {message}")]
    InvalidRequest {
        /// Details about the validation failure.
        message: String,
    },

    /// Image data is invalid, corrupted, or in an unsupported format.
    #[error("invalid image: {message}")]
    InvalidImage {
        /// Details about the image validation failure.
        message: String,
    },

    /// Prompt exceeds the model's context window.
    #[error("prompt too large: {tokens} tokens exceeds limit of {limit}")]
    PromptTooLarge {
        /// Number of tokens in the prompt.
        tokens: u64,
        /// Maximum allowed tokens for the model.
        limit: u64,
    },

    // =========================================================================
    // API Errors
    // =========================================================================
    /// The Gemini API returned an error response.
    ///
    /// # Recovery
    /// - Check the status code and message for specific guidance
    /// - 5xx errors are often transient and can be retried
    /// - 4xx errors typically require request modification
    #[error("API error ({status}): {message}")]
    ApiError {
        /// HTTP status code returned by the API.
        status: u16,
        /// Error message from the API.
        message: String,
        /// Raw error response body (for debugging).
        raw_response: Option<String>,
    },

    /// The API returned a response that could not be parsed.
    #[error("malformed API response: {message}")]
    MalformedResponse {
        /// Description of the parsing failure.
        message: String,
        /// Raw response body that failed to parse.
        raw_response: Option<String>,
    },

    // =========================================================================
    // Network Errors
    // =========================================================================
    /// Network connectivity error during API communication.
    ///
    /// # Recovery
    /// - Check network connectivity
    /// - Retry with exponential backoff
    /// - Consider using a different network path
    #[error("network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    /// Request timed out before receiving a response.
    ///
    /// # Recovery
    /// - Increase the timeout duration
    /// - Reduce request complexity (smaller images, shorter prompts)
    /// - Retry with exponential backoff
    #[error("request timed out after {duration:?}")]
    Timeout {
        /// Duration waited before timeout.
        duration: Duration,
    },

    // =========================================================================
    // Serialization Errors
    // =========================================================================
    /// Failed to serialize request or deserialize response.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    // =========================================================================
    // Configuration Errors
    // =========================================================================
    /// Configuration is invalid or incomplete.
    ///
    /// # Recovery
    /// - Review the configuration and provide all required fields
    /// - Check environment variables are set correctly
    #[error("configuration error: {message}")]
    ConfigError {
        /// Details about the configuration error.
        message: String,
    },

    /// Required environment variable is missing.
    #[error("missing environment variable: {name}")]
    MissingEnvVar {
        /// Name of the missing environment variable.
        name: String,
    },

    // =========================================================================
    // Internal Errors
    // =========================================================================
    /// An internal error occurred that should not happen in normal operation.
    ///
    /// If you encounter this error, please report it as a bug.
    #[error("internal error: {message}")]
    InternalError {
        /// Description of the internal error.
        message: String,
    },
}

impl GeminiError {
    /// Returns `true` if this error is potentially transient and the operation
    /// may succeed if retried.
    ///
    /// # Retryable Errors
    /// - Rate limit exceeded
    /// - Network errors
    /// - Timeouts
    /// - 5xx API errors
    ///
    /// # Non-Retryable Errors
    /// - Authentication errors
    /// - Invalid requests
    /// - Content blocked
    /// - Cost limit exceeded
    pub fn is_retryable(&self) -> bool {
        match self {
            GeminiError::RateLimitExceeded { .. }
            | GeminiError::NetworkError(_)
            | GeminiError::Timeout { .. } => true,
            GeminiError::ApiError { status, .. } => *status >= 500,
            _ => false,
        }
    }

    /// Returns `true` if this error is related to rate limiting.
    pub fn is_rate_limit(&self) -> bool {
        matches!(
            self,
            GeminiError::RateLimitExceeded { .. } | GeminiError::QuotaExhausted { .. }
        )
    }

    /// Returns `true` if this error is related to authentication.
    pub fn is_auth_error(&self) -> bool {
        matches!(self, GeminiError::AuthenticationError { .. })
    }

    /// Returns `true` if this error is related to content safety.
    pub fn is_safety_error(&self) -> bool {
        matches!(
            self,
            GeminiError::ContentBlocked { .. } | GeminiError::ResponseTruncated { .. }
        )
    }

    /// Returns the suggested retry delay if this is a rate limit error.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            GeminiError::RateLimitExceeded { retry_after } => *retry_after,
            _ => None,
        }
    }

    /// Create an authentication error with a message.
    pub fn auth_error(message: impl Into<String>) -> Self {
        GeminiError::AuthenticationError {
            message: message.into(),
        }
    }

    /// Create a rate limit error with optional retry duration.
    pub fn rate_limit(retry_after: Option<Duration>) -> Self {
        GeminiError::RateLimitExceeded { retry_after }
    }

    /// Create a cost limit error.
    pub fn cost_limit(current: f64, limit: f64) -> Self {
        GeminiError::CostLimitExceeded { current, limit }
    }

    /// Create a content blocked error.
    pub fn content_blocked(reason: impl Into<String>) -> Self {
        GeminiError::ContentBlocked {
            reason: reason.into(),
            category: None,
        }
    }

    /// Create an API error from status code and message.
    pub fn api_error(status: u16, message: impl Into<String>) -> Self {
        GeminiError::ApiError {
            status,
            message: message.into(),
            raw_response: None,
        }
    }

    /// Create a configuration error.
    pub fn config_error(message: impl Into<String>) -> Self {
        GeminiError::ConfigError {
            message: message.into(),
        }
    }

    /// Create a missing environment variable error.
    pub fn missing_env(name: impl Into<String>) -> Self {
        GeminiError::MissingEnvVar { name: name.into() }
    }

    /// Create an invalid request error.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        GeminiError::InvalidRequest {
            message: message.into(),
        }
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        GeminiError::InternalError {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_errors() {
        assert!(GeminiError::rate_limit(None).is_retryable());
        assert!(GeminiError::api_error(500, "server error").is_retryable());
        assert!(GeminiError::api_error(503, "service unavailable").is_retryable());

        assert!(!GeminiError::auth_error("invalid key").is_retryable());
        assert!(!GeminiError::cost_limit(10.0, 5.0).is_retryable());
        assert!(!GeminiError::content_blocked("violence").is_retryable());
        assert!(!GeminiError::api_error(400, "bad request").is_retryable());
    }

    #[test]
    fn test_error_classification() {
        assert!(GeminiError::auth_error("test").is_auth_error());
        assert!(GeminiError::rate_limit(None).is_rate_limit());
        assert!(GeminiError::content_blocked("test").is_safety_error());
    }

    #[test]
    fn test_retry_after() {
        let duration = Duration::from_secs(30);
        let err = GeminiError::rate_limit(Some(duration));
        assert_eq!(err.retry_after(), Some(duration));

        let err = GeminiError::auth_error("test");
        assert_eq!(err.retry_after(), None);
    }

    #[test]
    fn test_error_display() {
        let err = GeminiError::cost_limit(10.50, 5.00);
        let msg = err.to_string();
        assert!(msg.contains("10.5000"));
        assert!(msg.contains("5.0000"));
    }
}
