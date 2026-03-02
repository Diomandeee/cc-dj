//! Live API session configuration.
//!
//! Provides configuration types for Live API sessions including
//! voice settings, VAD configuration, and session parameters.

use serde::{Deserialize, Serialize};

/// Schema version for Live API configuration.
pub const LIVE_CONFIG_SCHEMA_VERSION: &str = "1.0.0";

/// Response modality for Live API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResponseModality {
    /// Audio response.
    #[default]
    Audio,
    /// Text response.
    Text,
}

/// Available voices for audio output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Voice {
    /// Aoede voice.
    Aoede,
    /// Charon voice.
    Charon,
    /// Fenrir voice.
    Fenrir,
    /// Kore voice (default).
    Kore,
    /// Puck voice.
    Puck,
    /// Custom voice name.
    Custom(String),
}

impl Default for Voice {
    fn default() -> Self {
        Self::Kore
    }
}

impl Voice {
    /// Returns the voice name as a string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Aoede => "Aoede",
            Self::Charon => "Charon",
            Self::Fenrir => "Fenrir",
            Self::Kore => "Kore",
            Self::Puck => "Puck",
            Self::Custom(name) => name,
        }
    }
}

/// Speech configuration for audio output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechConfig {
    /// Voice configuration.
    pub voice_config: VoiceConfig,
}

/// Voice configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceConfig {
    /// Prebuilt voice configuration.
    pub prebuilt_voice_config: PrebuiltVoiceConfig,
}

/// Prebuilt voice configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrebuiltVoiceConfig {
    /// Voice name.
    pub voice_name: String,
}

impl SpeechConfig {
    /// Creates a new speech config with the specified voice.
    pub fn new(voice: Voice) -> Self {
        Self {
            voice_config: VoiceConfig {
                prebuilt_voice_config: PrebuiltVoiceConfig {
                    voice_name: voice.as_str().to_string(),
                },
            },
        }
    }
}

/// Media resolution for input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaResolution {
    /// Low resolution (faster processing).
    MediaResolutionLow,
    /// Medium resolution (balanced).
    #[default]
    MediaResolutionMedium,
    /// High resolution (best quality).
    MediaResolutionHigh,
}

/// Context window compression configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextWindowCompressionConfig {
    /// Sliding window configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sliding_window: Option<SlidingWindow>,
    /// Number of tokens to trigger compression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_tokens: Option<u64>,
}

/// Sliding window compression.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlidingWindow {
    /// Target number of tokens to keep.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_tokens: Option<u64>,
}

impl Default for ContextWindowCompressionConfig {
    fn default() -> Self {
        Self {
            sliding_window: Some(SlidingWindow::default()),
            trigger_tokens: None,
        }
    }
}

/// Session resumption configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResumptionConfig {
    /// Handle from a previous session to resume.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
}

impl Default for SessionResumptionConfig {
    fn default() -> Self {
        Self { handle: None }
    }
}

/// Thinking configuration for native audio models.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingConfig {
    /// Thinking budget (number of tokens for thinking). Set to 0 to disable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<u32>,
    /// Whether to include thought summaries in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
}

impl Default for ThinkingConfig {
    fn default() -> Self {
        Self {
            thinking_budget: Some(1024),
            include_thoughts: Some(false),
        }
    }
}

/// Audio transcription configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTranscriptionConfig {}

/// Proactivity configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProactivityConfig {
    /// Enable proactive audio (model decides when to respond).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proactive_audio: Option<bool>,
}

impl Default for ProactivityConfig {
    fn default() -> Self {
        Self {
            proactive_audio: Some(false),
        }
    }
}

/// Live API session configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveConfig {
    /// Response modalities (AUDIO or TEXT).
    pub response_modalities: Vec<ResponseModality>,

    /// System instruction for the model.
    /// Note: Skipped during serialization — extracted to setup-level by ClientMessage::setup().
    #[serde(skip)]
    pub system_instruction: Option<String>,

    /// Speech configuration for audio output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speech_config: Option<SpeechConfig>,

    /// Media resolution for input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_resolution: Option<MediaResolution>,

    /// Context window compression configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window_compression: Option<ContextWindowCompressionConfig>,

    /// Session resumption configuration.
    /// Note: Skipped during serialization — extracted to setup-level by ClientMessage::setup().
    #[serde(skip)]
    pub session_resumption: Option<SessionResumptionConfig>,

    /// Realtime input configuration (VAD settings).
    /// Note: Skipped during serialization — extracted to setup-level by ClientMessage::setup().
    #[serde(skip)]
    pub realtime_input_config: Option<super::vad::RealtimeInputConfig>,

    /// Input audio transcription configuration.
    /// Note: Skipped during serialization — extracted to setup-level by ClientMessage::setup().
    #[serde(skip)]
    pub input_audio_transcription: Option<AudioTranscriptionConfig>,

    /// Output audio transcription configuration.
    /// Note: Skipped during serialization — extracted to setup-level by ClientMessage::setup().
    #[serde(skip)]
    pub output_audio_transcription: Option<AudioTranscriptionConfig>,

    /// Thinking configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,

    /// Proactivity configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proactivity: Option<ProactivityConfig>,

    /// Enable affective dialog (emotion-aware responses).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_affective_dialog: Option<bool>,

    /// Tools available to the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            response_modalities: vec![ResponseModality::Audio],
            system_instruction: None,
            speech_config: Some(SpeechConfig::new(Voice::default())),
            media_resolution: None,
            context_window_compression: None,
            session_resumption: None,
            realtime_input_config: None,
            input_audio_transcription: None,
            output_audio_transcription: None,
            thinking_config: None,
            proactivity: None,
            enable_affective_dialog: None,
            tools: None,
        }
    }
}

impl LiveConfig {
    /// Creates a new Live config with audio response modality.
    pub fn audio() -> Self {
        Self::default()
    }

    /// Creates a new Live config with text response modality.
    pub fn text() -> Self {
        Self {
            response_modalities: vec![ResponseModality::Text],
            speech_config: None,
            ..Default::default()
        }
    }

    /// Sets the system instruction.
    pub fn with_system_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.system_instruction = Some(instruction.into());
        self
    }

    /// Sets the voice for audio output.
    pub fn with_voice(mut self, voice: Voice) -> Self {
        self.speech_config = Some(SpeechConfig::new(voice));
        self
    }

    /// Enables input audio transcription.
    pub fn with_input_transcription(mut self) -> Self {
        self.input_audio_transcription = Some(AudioTranscriptionConfig {});
        self
    }

    /// Enables output audio transcription.
    pub fn with_output_transcription(mut self) -> Self {
        self.output_audio_transcription = Some(AudioTranscriptionConfig {});
        self
    }

    /// Enables context window compression for long sessions.
    pub fn with_context_compression(mut self) -> Self {
        self.context_window_compression = Some(ContextWindowCompressionConfig::default());
        self
    }

    /// Enables session resumption.
    pub fn with_session_resumption(mut self, handle: Option<String>) -> Self {
        self.session_resumption = Some(SessionResumptionConfig { handle });
        self
    }

    /// Sets thinking configuration.
    pub fn with_thinking(mut self, budget: u32, include_thoughts: bool) -> Self {
        self.thinking_config = Some(ThinkingConfig {
            thinking_budget: Some(budget),
            include_thoughts: Some(include_thoughts),
        });
        self
    }

    /// Enables affective dialog.
    pub fn with_affective_dialog(mut self) -> Self {
        self.enable_affective_dialog = Some(true);
        self
    }

    /// Enables proactive audio.
    pub fn with_proactive_audio(mut self) -> Self {
        self.proactivity = Some(ProactivityConfig {
            proactive_audio: Some(true),
        });
        self
    }
}

/// Gemini Live API models.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LiveModel {
    /// Gemini 2.5 Flash native audio (latest stable).
    #[default]
    Flash25NativeAudio,
    /// Gemini 2.5 Flash native audio (Dec 2025 preview).
    Flash25NativeAudioDec2025,
    /// Custom model string for forward compatibility.
    Custom(String),
}

impl LiveModel {
    /// Returns the model identifier string (with `models/` prefix).
    pub fn as_str(&self) -> &str {
        match self {
            Self::Flash25NativeAudio => "models/gemini-2.5-flash-native-audio-latest",
            Self::Flash25NativeAudioDec2025 => "models/gemini-2.5-flash-native-audio-preview-12-2025",
            Self::Custom(s) => s,
        }
    }

    /// Returns the WebSocket endpoint for this model.
    pub fn ws_endpoint(&self) -> String {
        format!(
            "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent?key={{}}",
        )
    }

    /// Returns the maximum session duration for audio-only.
    pub fn max_audio_duration_secs(&self) -> u64 {
        15 * 60 // 15 minutes
    }

    /// Returns the maximum session duration for audio+video.
    pub fn max_video_duration_secs(&self) -> u64 {
        2 * 60 // 2 minutes
    }

    /// Returns the context window limit in tokens.
    pub fn context_window_tokens(&self) -> u64 {
        match self {
            Self::Flash25NativeAudio => 128_000,
            Self::Flash25NativeAudioDec2025 => 128_000,
            Self::Custom(_) => 32_000,
        }
    }
}

impl std::fmt::Display for LiveModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LiveConfig::default();
        assert_eq!(config.response_modalities, vec![ResponseModality::Audio]);
        assert!(config.speech_config.is_some());
    }

    #[test]
    fn test_text_config() {
        let config = LiveConfig::text();
        assert_eq!(config.response_modalities, vec![ResponseModality::Text]);
        assert!(config.speech_config.is_none());
    }

    #[test]
    fn test_config_builder() {
        let config = LiveConfig::audio()
            .with_system_instruction("You are a helpful assistant")
            .with_voice(Voice::Puck)
            .with_input_transcription()
            .with_output_transcription()
            .with_context_compression();

        assert!(config.system_instruction.is_some());
        assert!(config.input_audio_transcription.is_some());
        assert!(config.output_audio_transcription.is_some());
        assert!(config.context_window_compression.is_some());
    }

    #[test]
    fn test_live_model() {
        let model = LiveModel::Flash25NativeAudio;
        assert_eq!(
            model.as_str(),
            "models/gemini-2.5-flash-native-audio-latest"
        );
        assert_eq!(model.context_window_tokens(), 128_000);
    }

    #[test]
    fn test_voice_as_str() {
        assert_eq!(Voice::Kore.as_str(), "Kore");
        assert_eq!(Voice::Custom("MyVoice".to_string()).as_str(), "MyVoice");
    }
}

