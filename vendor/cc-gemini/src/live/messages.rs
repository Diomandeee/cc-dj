//! Live API message types.
//!
//! Provides all message types for bidirectional communication with the
//! Gemini Live API over WebSockets.

use serde::{Deserialize, Serialize};

use super::config::{AudioTranscriptionConfig, LiveConfig, SessionResumptionConfig};
use super::vad::RealtimeInputConfig;

// ============================================================================
// Client Messages (sent to server)
// ============================================================================

/// Client message wrapper.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientMessage {
    /// Setup message (first message only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<BidiGenerateContentSetup>,

    /// Client content message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_content: Option<BidiGenerateContentClientContent>,

    /// Realtime input message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realtime_input: Option<BidiGenerateContentRealtimeInput>,

    /// Tool response message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_response: Option<BidiGenerateContentToolResponse>,
}

impl ClientMessage {
    /// Creates a setup message.
    ///
    /// Extracts setup-level fields (system instruction, VAD config, transcription,
    /// session resumption) from `LiveConfig` so they are serialized at the `setup`
    /// level rather than inside `generationConfig`, matching the Gemini API schema.
    pub fn setup(model: &str, config: LiveConfig) -> Self {
        let system_instruction = config
            .system_instruction
            .as_ref()
            .map(|text| SystemInstruction {
                parts: vec![SystemInstructionPart { text: text.clone() }],
            });
        let realtime_input_config = config.realtime_input_config.clone();
        let input_audio_transcription = config.input_audio_transcription.clone();
        let output_audio_transcription = config.output_audio_transcription.clone();
        let session_resumption = config.session_resumption.clone();

        Self {
            setup: Some(BidiGenerateContentSetup {
                model: model.to_string(),
                generation_config: Some(config),
                system_instruction,
                realtime_input_config,
                input_audio_transcription,
                output_audio_transcription,
                session_resumption,
            }),
            client_content: None,
            realtime_input: None,
            tool_response: None,
        }
    }

    /// Creates a client content message.
    pub fn client_content(content: BidiGenerateContentClientContent) -> Self {
        Self {
            setup: None,
            client_content: Some(content),
            realtime_input: None,
            tool_response: None,
        }
    }

    /// Creates a realtime input message.
    pub fn realtime_input(input: BidiGenerateContentRealtimeInput) -> Self {
        Self {
            setup: None,
            client_content: None,
            realtime_input: Some(input),
            tool_response: None,
        }
    }

    /// Creates a tool response message.
    pub fn tool_response(response: BidiGenerateContentToolResponse) -> Self {
        Self {
            setup: None,
            client_content: None,
            realtime_input: None,
            tool_response: Some(response),
        }
    }
}

/// Session setup message.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BidiGenerateContentSetup {
    /// Model identifier.
    pub model: String,

    /// Generation configuration (response modalities, speech config, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<LiveConfig>,

    /// System instruction (setup-level, NOT inside generation_config).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<SystemInstruction>,

    /// Realtime input / VAD configuration (setup-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realtime_input_config: Option<RealtimeInputConfig>,

    /// Input audio transcription (setup-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_transcription: Option<AudioTranscriptionConfig>,

    /// Output audio transcription (setup-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_audio_transcription: Option<AudioTranscriptionConfig>,

    /// Session resumption configuration (setup-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_resumption: Option<SessionResumptionConfig>,
}

/// System instruction wrapper for the Gemini API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInstruction {
    /// Instruction parts.
    pub parts: Vec<SystemInstructionPart>,
}

/// A single part of a system instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInstructionPart {
    /// Text content.
    pub text: String,
}

/// Client content message for text/turn-based input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BidiGenerateContentClientContent {
    /// Content turns.
    pub turns: Vec<ContentTurn>,

    /// Whether this completes the current turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_complete: Option<bool>,
}

impl BidiGenerateContentClientContent {
    /// Creates a new client content message with text.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            turns: vec![ContentTurn {
                role: Some("user".to_string()),
                parts: vec![ContentPart::Text { text: text.into() }],
            }],
            turn_complete: Some(true),
        }
    }

    /// Creates a new client content with multiple turns.
    pub fn with_turns(turns: Vec<ContentTurn>) -> Self {
        Self {
            turns,
            turn_complete: Some(true),
        }
    }

    /// Sets whether this is a complete turn.
    pub fn with_turn_complete(mut self, complete: bool) -> Self {
        self.turn_complete = Some(complete);
        self
    }
}

/// A content turn in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentTurn {
    /// Role (user or model).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Content parts.
    pub parts: Vec<ContentPart>,
}

/// A part of content (text or inline data).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentPart {
    /// Text content.
    Text {
        /// The text.
        text: String,
    },
    /// Inline data (audio, video, image).
    InlineData {
        /// The inline data.
        inline_data: InlineData,
    },
}

/// Inline data blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineData {
    /// Base64-encoded data.
    pub data: String,
    /// MIME type.
    pub mime_type: String,
}

/// Realtime input for streaming audio/video.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BidiGenerateContentRealtimeInput {
    /// Audio input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<RealtimeAudio>,

    /// Video input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<RealtimeVideo>,

    /// Text input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Activity start marker (for manual VAD).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_start: Option<ActivityMarker>,

    /// Activity end marker (for manual VAD).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_end: Option<ActivityMarker>,

    /// Audio stream end marker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_stream_end: Option<bool>,
}

impl BidiGenerateContentRealtimeInput {
    /// Creates an audio input message.
    pub fn audio(data: &[u8], sample_rate: u32) -> Self {
        Self {
            audio: Some(RealtimeAudio {
                data: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data),
                mime_type: format!("audio/pcm;rate={}", sample_rate),
            }),
            video: None,
            text: None,
            activity_start: None,
            activity_end: None,
            audio_stream_end: None,
        }
    }

    /// Creates a video input message.
    pub fn video(data: &[u8], mime_type: &str) -> Self {
        Self {
            audio: None,
            video: Some(RealtimeVideo {
                data: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data),
                mime_type: mime_type.to_string(),
            }),
            text: None,
            activity_start: None,
            activity_end: None,
            audio_stream_end: None,
        }
    }

    /// Creates a text input message.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            audio: None,
            video: None,
            text: Some(text.into()),
            activity_start: None,
            activity_end: None,
            audio_stream_end: None,
        }
    }

    /// Creates an activity start marker (for manual VAD).
    pub fn activity_start() -> Self {
        Self {
            audio: None,
            video: None,
            text: None,
            activity_start: Some(ActivityMarker {}),
            activity_end: None,
            audio_stream_end: None,
        }
    }

    /// Creates an activity end marker (for manual VAD).
    pub fn activity_end() -> Self {
        Self {
            audio: None,
            video: None,
            text: None,
            activity_start: None,
            activity_end: Some(ActivityMarker {}),
            audio_stream_end: None,
        }
    }

    /// Creates an audio stream end marker.
    pub fn audio_stream_end() -> Self {
        Self {
            audio: None,
            video: None,
            text: None,
            activity_start: None,
            activity_end: None,
            audio_stream_end: Some(true),
        }
    }
}

/// Realtime audio data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeAudio {
    /// Base64-encoded PCM audio.
    pub data: String,
    /// MIME type with sample rate.
    pub mime_type: String,
}

/// Realtime video data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeVideo {
    /// Base64-encoded video frame.
    pub data: String,
    /// MIME type.
    pub mime_type: String,
}

/// Activity marker (empty struct).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivityMarker {}

/// Tool response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BidiGenerateContentToolResponse {
    /// Function responses.
    pub function_responses: Vec<FunctionResponse>,
}

/// Function response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionResponse {
    /// Function call ID.
    pub id: String,
    /// Function name.
    pub name: String,
    /// Response data.
    pub response: serde_json::Value,
}

// ============================================================================
// Server Messages (received from server)
// ============================================================================

/// Server message wrapper.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerMessage {
    /// Usage metadata.
    #[serde(default)]
    pub usage_metadata: Option<UsageMetadata>,

    /// Setup complete notification.
    #[serde(default)]
    pub setup_complete: Option<SetupComplete>,

    /// Server content.
    #[serde(default)]
    pub server_content: Option<BidiGenerateContentServerContent>,

    /// Tool call request.
    #[serde(default)]
    pub tool_call: Option<BidiGenerateContentToolCall>,

    /// Tool call cancellation.
    #[serde(default)]
    pub tool_call_cancellation: Option<ToolCallCancellation>,

    /// GoAway notification.
    #[serde(default)]
    pub go_away: Option<GoAway>,

    /// Session resumption update.
    #[serde(default)]
    pub session_resumption_update: Option<SessionResumptionUpdate>,
}

impl ServerMessage {
    /// Returns true if this is a setup complete message.
    pub fn is_setup_complete(&self) -> bool {
        self.setup_complete.is_some()
    }

    /// Returns true if this is a turn complete message.
    pub fn is_turn_complete(&self) -> bool {
        self.server_content
            .as_ref()
            .map(|c| c.turn_complete.unwrap_or(false))
            .unwrap_or(false)
    }

    /// Returns true if this is an interruption.
    pub fn is_interrupted(&self) -> bool {
        self.server_content
            .as_ref()
            .map(|c| c.interrupted.unwrap_or(false))
            .unwrap_or(false)
    }

    /// Returns true if generation is complete.
    pub fn is_generation_complete(&self) -> bool {
        self.server_content
            .as_ref()
            .map(|c| c.generation_complete.unwrap_or(false))
            .unwrap_or(false)
    }

    /// Returns the text content if any.
    pub fn text(&self) -> Option<String> {
        self.server_content.as_ref().and_then(|c| {
            c.model_turn.as_ref().and_then(|t| {
                t.parts.iter().find_map(|p| {
                    if let ServerContentPart::Text { text } = p {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
            })
        })
    }

    /// Returns the audio data if any (base64 encoded).
    pub fn audio_data(&self) -> Option<Vec<u8>> {
        self.server_content.as_ref().and_then(|c| {
            c.model_turn.as_ref().and_then(|t| {
                t.parts.iter().find_map(|p| {
                    if let ServerContentPart::InlineData { inline_data } = p {
                        if inline_data.mime_type.starts_with("audio/") {
                            base64::Engine::decode(
                                &base64::engine::general_purpose::STANDARD,
                                &inline_data.data,
                            )
                            .ok()
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            })
        })
    }

    /// Returns the input transcription if any.
    pub fn input_transcription(&self) -> Option<&str> {
        self.server_content
            .as_ref()
            .and_then(|c| c.input_transcription.as_ref())
            .map(|t| t.text.as_str())
    }

    /// Returns the output transcription if any.
    pub fn output_transcription(&self) -> Option<&str> {
        self.server_content
            .as_ref()
            .and_then(|c| c.output_transcription.as_ref())
            .map(|t| t.text.as_str())
    }
}

/// Setup complete notification.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SetupComplete {}

/// Server content message.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BidiGenerateContentServerContent {
    /// Model turn content.
    #[serde(default)]
    pub model_turn: Option<ModelTurn>,

    /// Generation is complete.
    #[serde(default)]
    pub generation_complete: Option<bool>,

    /// Turn is complete.
    #[serde(default)]
    pub turn_complete: Option<bool>,

    /// Generation was interrupted.
    #[serde(default)]
    pub interrupted: Option<bool>,

    /// Input transcription.
    #[serde(default)]
    pub input_transcription: Option<Transcription>,

    /// Output transcription.
    #[serde(default)]
    pub output_transcription: Option<Transcription>,

    /// Grounding metadata.
    #[serde(default)]
    pub grounding_metadata: Option<serde_json::Value>,
}

/// Model turn content.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTurn {
    /// Content parts.
    #[serde(default)]
    pub parts: Vec<ServerContentPart>,
}

/// Server content part.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerContentPart {
    /// Text content.
    Text {
        /// The text.
        text: String,
    },
    /// Inline data (audio).
    InlineData {
        /// The inline data.
        inline_data: InlineData,
    },
    /// Executable code.
    ExecutableCode {
        /// The code.
        code: String,
    },
    /// Code execution result.
    CodeExecutionResult {
        /// The output.
        output: String,
    },
}

/// Transcription data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcription {
    /// Transcription text.
    pub text: String,
}

/// Tool call request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BidiGenerateContentToolCall {
    /// Function calls.
    pub function_calls: Vec<FunctionCall>,
}

/// Function call.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCall {
    /// Call ID.
    pub id: String,
    /// Function name.
    pub name: String,
    /// Arguments.
    #[serde(default)]
    pub args: serde_json::Value,
}

/// Tool call cancellation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallCancellation {
    /// IDs of cancelled calls.
    pub ids: Vec<String>,
}

/// GoAway notification.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoAway {
    /// Time remaining before disconnect.
    pub time_left: String,
}

/// Session resumption update.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResumptionUpdate {
    /// New session handle.
    #[serde(default)]
    pub new_handle: Option<String>,

    /// Whether the session is resumable.
    #[serde(default)]
    pub resumable: bool,
}

/// Usage metadata.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    /// Prompt token count.
    #[serde(default)]
    pub prompt_token_count: u64,

    /// Response token count.
    #[serde(default)]
    pub response_token_count: u64,

    /// Total token count.
    #[serde(default)]
    pub total_token_count: u64,

    /// Thoughts token count.
    #[serde(default)]
    pub thoughts_token_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_content_text() {
        let content = BidiGenerateContentClientContent::text("Hello");
        assert_eq!(content.turns.len(), 1);
        assert_eq!(content.turn_complete, Some(true));
    }

    #[test]
    fn test_realtime_audio() {
        let input = BidiGenerateContentRealtimeInput::audio(b"test", 16000);
        assert!(input.audio.is_some());
        let audio = input.audio.unwrap();
        assert_eq!(audio.mime_type, "audio/pcm;rate=16000");
    }

    #[test]
    fn test_activity_markers() {
        let start = BidiGenerateContentRealtimeInput::activity_start();
        assert!(start.activity_start.is_some());

        let end = BidiGenerateContentRealtimeInput::activity_end();
        assert!(end.activity_end.is_some());
    }

    #[test]
    fn test_server_message_helpers() {
        let msg = ServerMessage {
            usage_metadata: None,
            setup_complete: Some(SetupComplete {}),
            server_content: None,
            tool_call: None,
            tool_call_cancellation: None,
            go_away: None,
            session_resumption_update: None,
        };
        assert!(msg.is_setup_complete());
        assert!(!msg.is_turn_complete());
    }
}
