//! Gemini Live API support.
//!
//! This module provides real-time, bidirectional audio and video streaming
//! with Gemini models using WebSockets.
//!
//! # Features
//!
//! - **Real-time streaming**: Low-latency bidirectional audio/video communication
//! - **Voice Activity Detection (VAD)**: Automatic or manual speech detection
//! - **Session management**: Long-running sessions with resumption support
//! - **Audio transcription**: Real-time transcription of input and output audio
//! - **Tool use**: Function calling during live sessions
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use cc_gemini::live::{LiveSession, LiveConfig, ChannelCallbacks};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create callbacks to receive messages
//!     let (callbacks, mut receiver) = ChannelCallbacks::new();
//!     let callbacks = Arc::new(callbacks);
//!
//!     // Create and connect session
//!     let session = LiveSession::builder("your_api_key")
//!         .system_instruction("You are a helpful assistant")
//!         .with_input_transcription()
//!         .with_output_transcription()
//!         .build();
//!
//!     session.connect(callbacks.clone()).await?;
//!
//!     // Send text
//!     session.send_text("Hello, how are you?").await?;
//!
//!     // Receive responses
//!     while let Some(msg) = receiver.recv().await {
//!         if let Some(text) = msg.text() {
//!             println!("Model: {}", text);
//!         }
//!         if msg.is_turn_complete() {
//!             break;
//!         }
//!     }
//!
//!     session.close().await?;
//!     Ok(())
//! }
//! ```
//!
//! # Audio Streaming
//!
//! ```rust,ignore
//! // Send audio (16-bit PCM at 16kHz)
//! let audio_data: Vec<u8> = get_microphone_data();
//! session.send_audio(&audio_data, 16000).await?;
//!
//! // Receive audio (24kHz output)
//! while let Some(msg) = receiver.recv().await {
//!     if let Some(audio) = msg.audio_data() {
//!         play_audio(&audio, 24000);
//!     }
//! }
//! ```
//!
//! # Voice Activity Detection
//!
//! The Live API supports automatic VAD by default. For manual control:
//!
//! ```rust,ignore
//! use cc_gemini::live::{LiveConfig, RealtimeInputConfig};
//!
//! // Configure manual VAD
//! let config = LiveConfig::audio()
//!     .with_realtime_input(RealtimeInputConfig::manual());
//!
//! // Then manually signal activity
//! session.send_activity_start().await?;
//! session.send_audio(&audio_data, 16000).await?;
//! session.send_activity_end().await?;
//! ```
//!
//! # Session Limits
//!
//! | Session Type | Duration Limit | Context Window |
//! |--------------|----------------|----------------|
//! | Audio-only   | 15 minutes     | 128k tokens (native) / 32k tokens (other) |
//! | Audio+Video  | 2 minutes      | 128k tokens (native) / 32k tokens (other) |
//!
//! Use context window compression for longer sessions:
//!
//! ```rust,ignore
//! let config = LiveConfig::audio()
//!     .with_context_compression();
//! ```

pub mod config;
pub mod error;
pub mod messages;
pub mod session;
pub mod vad;

// Re-export main types
pub use config::{
    AudioTranscriptionConfig, ContextWindowCompressionConfig, LiveConfig, LiveModel,
    MediaResolution, ProactivityConfig, ResponseModality, SessionResumptionConfig, SpeechConfig,
    ThinkingConfig, Voice, VoiceConfig, LIVE_CONFIG_SCHEMA_VERSION,
};
pub use error::{LiveError, LiveResult};
pub use messages::{
    BidiGenerateContentClientContent, BidiGenerateContentRealtimeInput,
    BidiGenerateContentServerContent, BidiGenerateContentToolCall, BidiGenerateContentToolResponse,
    ClientMessage, ContentPart, ContentTurn, FunctionCall, FunctionResponse, GoAway, InlineData,
    ModelTurn, RealtimeAudio, RealtimeVideo, ServerMessage, SessionResumptionUpdate, Transcription,
    UsageMetadata,
};
pub use session::{
    collect_turn, wait_for_message, ChannelCallbacks, LiveSession, LiveSessionBuilder,
    LiveSessionCallbacks, SessionState,
};
pub use vad::{
    ActivityHandling, AutomaticActivityDetection, EndSensitivity, RealtimeInputConfig,
    StartSensitivity, TurnCoverage, VadConfigBuilder,
};

/// Prelude for convenient imports.
pub mod prelude {
    pub use super::config::{LiveConfig, LiveModel, ResponseModality, Voice};
    pub use super::error::{LiveError, LiveResult};
    pub use super::messages::{ServerMessage, Transcription};
    pub use super::session::{
        collect_turn, wait_for_message, ChannelCallbacks, LiveSession, LiveSessionCallbacks,
        SessionState,
    };
    pub use super::vad::{RealtimeInputConfig, VadConfigBuilder};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_module_exports() {
        // Verify main types are accessible
        let _config = LiveConfig::default();
        let _model = LiveModel::Flash25NativeAudio;
        let _voice = Voice::Kore;
    }

    #[test]
    fn test_prelude_exports() {
        use prelude::*;

        let config = LiveConfig::audio();
        assert_eq!(config.response_modalities, vec![ResponseModality::Audio]);
    }

    #[test]
    fn test_vad_builder() {
        let config = VadConfigBuilder::new()
            .low_start_sensitivity()
            .silence_duration(500)
            .build();

        assert!(config.automatic_activity_detection.is_some());
    }
}
