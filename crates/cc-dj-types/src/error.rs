//! Error types for cc-dj.

use thiserror::Error;

/// Errors that can occur in the DJ agent system.
#[derive(Debug, Error)]
pub enum DJError {
    /// Command not found.
    #[error("Command not found: {0}")]
    CommandNotFound(String),

    /// Action not allowed due to safety constraints.
    #[error("Action not allowed: {reason}")]
    ActionNotAllowed {
        /// Reason the action was blocked.
        reason: String,
    },

    /// Tier not unlocked.
    #[error("Tier {tier} not unlocked")]
    TierLocked {
        /// The locked tier.
        tier: u8,
    },

    /// Deck not found.
    #[error("Deck not found: {0}")]
    DeckNotFound(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// YAML parsing error.
    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yml::Error),

    /// JSON parsing error.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Bridge execution error.
    #[error("Bridge error: {0}")]
    BridgeError(String),

    /// Voice recognition error.
    #[error("Voice error: {0}")]
    VoiceError(String),

    /// Gesture recognition error.
    #[error("Gesture error: {0}")]
    GestureError(String),

    /// MIDI error.
    #[error("MIDI error: {0}")]
    MidiError(String),

    /// Execution error.
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// Cooldown active.
    #[error("Action on cooldown: {beats_remaining} beats remaining")]
    CooldownActive {
        /// Beats remaining on cooldown.
        beats_remaining: f64,
    },

    /// Quantization miss.
    #[error("Quantization miss: phase error {phase_error_deg}°")]
    QuantizationMiss {
        /// Phase error in degrees.
        phase_error_deg: f64,
    },
}

/// Result type for DJ operations.
pub type Result<T> = std::result::Result<T, DJError>;

impl DJError {
    /// Creates an action not allowed error.
    pub fn action_not_allowed(reason: impl Into<String>) -> Self {
        Self::ActionNotAllowed {
            reason: reason.into(),
        }
    }

    /// Creates a bridge error.
    pub fn bridge(message: impl Into<String>) -> Self {
        Self::BridgeError(message.into())
    }

    /// Creates a voice error.
    pub fn voice(message: impl Into<String>) -> Self {
        Self::VoiceError(message.into())
    }

    /// Creates a gesture error.
    pub fn gesture(message: impl Into<String>) -> Self {
        Self::GestureError(message.into())
    }

    /// Creates a MIDI error.
    pub fn midi(message: impl Into<String>) -> Self {
        Self::MidiError(message.into())
    }

    /// Creates an execution error.
    pub fn execution(message: impl Into<String>) -> Self {
        Self::ExecutionError(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DJError::CommandNotFound("PLAY_A".to_string());
        assert!(err.to_string().contains("PLAY_A"));
    }

    #[test]
    fn test_action_not_allowed() {
        let err = DJError::action_not_allowed("deck is playing");
        assert!(err.to_string().contains("deck is playing"));
    }
}
