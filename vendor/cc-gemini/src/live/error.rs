//! Live API specific errors.
//!
//! Provides error types for WebSocket connections and Live API interactions.

use std::time::Duration;
use thiserror::Error;

/// Errors specific to the Gemini Live API.
#[derive(Debug, Error)]
pub enum LiveError {
    /// WebSocket connection failed.
    #[error("WebSocket connection failed: {message}")]
    ConnectionFailed {
        /// Error message.
        message: String,
        /// Whether the connection can be retried.
        retryable: bool,
    },

    /// WebSocket connection closed unexpectedly.
    #[error("Connection closed: code={code}, reason={reason}")]
    ConnectionClosed {
        /// Close code.
        code: u16,
        /// Close reason.
        reason: String,
    },

    /// Session setup failed.
    #[error("Session setup failed: {0}")]
    SetupFailed(String),

    /// Session has expired or been terminated.
    #[error("Session expired or terminated")]
    SessionExpired,

    /// Session resumption failed.
    #[error("Session resumption failed: {0}")]
    ResumptionFailed(String),

    /// Invalid session handle for resumption.
    #[error("Invalid session handle: {0}")]
    InvalidSessionHandle(String),

    /// Message serialization error.
    #[error("Failed to serialize message: {0}")]
    SerializationError(String),

    /// Message deserialization error.
    #[error("Failed to deserialize message: {0}")]
    DeserializationError(String),

    /// Invalid message received from server.
    #[error("Invalid server message: {0}")]
    InvalidServerMessage(String),

    /// Server sent a GoAway message (connection will terminate soon).
    #[error("Server sent GoAway: {time_left:?} remaining")]
    GoAway {
        /// Time remaining before connection terminates.
        time_left: Duration,
    },

    /// Audio format error.
    #[error("Audio format error: {0}")]
    AudioFormatError(String),

    /// Video format error.
    #[error("Video format error: {0}")]
    VideoFormatError(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded, retry after {retry_after:?}")]
    RateLimitExceeded {
        /// Suggested retry delay.
        retry_after: Option<Duration>,
    },

    /// Content was blocked by safety filters.
    #[error("Content blocked: {reason}")]
    ContentBlocked {
        /// Reason for blocking.
        reason: String,
    },

    /// Tool call error.
    #[error("Tool call error: {0}")]
    ToolCallError(String),

    /// Session time limit exceeded.
    #[error("Session time limit exceeded: {limit_type}")]
    SessionTimeLimitExceeded {
        /// Type of limit (audio-only: 15min, audio+video: 2min).
        limit_type: String,
    },

    /// Context window exceeded.
    #[error("Context window exceeded: {tokens} tokens")]
    ContextWindowExceeded {
        /// Number of tokens.
        tokens: u64,
    },

    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl LiveError {
    /// Returns true if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::ConnectionFailed { retryable, .. } => *retryable,
            Self::ConnectionClosed { code, .. } => {
                // 1000 = normal close, 1001 = going away - not retryable
                // Other codes may be retryable
                !matches!(*code, 1000 | 1001)
            }
            Self::RateLimitExceeded { .. } => true,
            Self::GoAway { .. } => true, // Can reconnect with session resumption
            Self::SessionExpired => false,
            Self::ContentBlocked { .. } => false,
            Self::ConfigError(_) => false,
            Self::Internal(_) => false,
            _ => false,
        }
    }

    /// Returns the suggested retry delay if applicable.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimitExceeded { retry_after } => *retry_after,
            Self::GoAway { time_left } => Some(*time_left),
            Self::ConnectionFailed { retryable: true, .. } => Some(Duration::from_secs(1)),
            _ => None,
        }
    }

    /// Creates a connection failed error.
    pub fn connection_failed(message: impl Into<String>, retryable: bool) -> Self {
        Self::ConnectionFailed {
            message: message.into(),
            retryable,
        }
    }

    /// Creates a connection closed error.
    pub fn connection_closed(code: u16, reason: impl Into<String>) -> Self {
        Self::ConnectionClosed {
            code,
            reason: reason.into(),
        }
    }
}

/// Result type for Live API operations.
pub type LiveResult<T> = Result<T, LiveError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_retryable() {
        let retryable = LiveError::connection_failed("timeout", true);
        assert!(retryable.is_retryable());

        let not_retryable = LiveError::connection_failed("auth failed", false);
        assert!(!not_retryable.is_retryable());

        let rate_limit = LiveError::RateLimitExceeded {
            retry_after: Some(Duration::from_secs(5)),
        };
        assert!(rate_limit.is_retryable());
        assert_eq!(rate_limit.retry_after(), Some(Duration::from_secs(5)));
    }

    #[test]
    fn test_connection_closed_retryable() {
        // Normal close - not retryable
        let normal = LiveError::connection_closed(1000, "normal");
        assert!(!normal.is_retryable());

        // Going away - not retryable
        let going_away = LiveError::connection_closed(1001, "going away");
        assert!(!going_away.is_retryable());

        // Other codes - potentially retryable
        let other = LiveError::connection_closed(1006, "abnormal");
        assert!(other.is_retryable());
    }
}

