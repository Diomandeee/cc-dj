//! Voice Activity Detection (VAD) configuration.
//!
//! Provides configuration types for automatic voice activity detection
//! in the Live API.

use serde::{Deserialize, Serialize};

/// Start of speech sensitivity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StartSensitivity {
    /// Unspecified (defaults to HIGH).
    StartSensitivityUnspecified,
    /// High sensitivity - detects speech more often.
    #[default]
    StartSensitivityHigh,
    /// Low sensitivity - detects speech less often.
    StartSensitivityLow,
}

/// End of speech sensitivity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EndSensitivity {
    /// Unspecified (defaults to HIGH).
    EndSensitivityUnspecified,
    /// High sensitivity - ends speech more often.
    #[default]
    EndSensitivityHigh,
    /// Low sensitivity - ends speech less often.
    EndSensitivityLow,
}

/// Activity handling modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActivityHandling {
    /// Unspecified (defaults to START_OF_ACTIVITY_INTERRUPTS).
    ActivityHandlingUnspecified,
    /// Start of activity interrupts the model's response (barge-in).
    #[default]
    StartOfActivityInterrupts,
    /// Model's response will not be interrupted.
    NoInterruption,
}

/// Turn coverage options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TurnCoverage {
    /// Unspecified (defaults to TURN_INCLUDES_ONLY_ACTIVITY).
    TurnCoverageUnspecified,
    /// Turn includes only activity since last turn.
    #[default]
    TurnIncludesOnlyActivity,
    /// Turn includes all input since last turn (including silence).
    TurnIncludesAllInput,
}

/// Automatic activity detection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomaticActivityDetection {
    /// If true, automatic detection is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,

    /// Start of speech sensitivity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_of_speech_sensitivity: Option<StartSensitivity>,

    /// End of speech sensitivity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_of_speech_sensitivity: Option<EndSensitivity>,

    /// Duration of detected speech before start-of-speech is committed (ms).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<u32>,

    /// Duration of silence before end-of-speech is committed (ms).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<u32>,
}

impl Default for AutomaticActivityDetection {
    fn default() -> Self {
        Self {
            disabled: Some(false),
            start_of_speech_sensitivity: Some(StartSensitivity::StartSensitivityHigh),
            end_of_speech_sensitivity: Some(EndSensitivity::EndSensitivityHigh),
            prefix_padding_ms: Some(20),
            silence_duration_ms: Some(100),
        }
    }
}

impl AutomaticActivityDetection {
    /// Creates a new automatic activity detection config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disables automatic activity detection.
    pub fn disabled() -> Self {
        Self {
            disabled: Some(true),
            ..Default::default()
        }
    }

    /// Sets start sensitivity to low (less false positives).
    pub fn with_low_start_sensitivity(mut self) -> Self {
        self.start_of_speech_sensitivity = Some(StartSensitivity::StartSensitivityLow);
        self
    }

    /// Sets end sensitivity to low (allows longer pauses).
    pub fn with_low_end_sensitivity(mut self) -> Self {
        self.end_of_speech_sensitivity = Some(EndSensitivity::EndSensitivityLow);
        self
    }

    /// Sets the prefix padding (speech detection delay in ms).
    pub fn with_prefix_padding(mut self, ms: u32) -> Self {
        self.prefix_padding_ms = Some(ms);
        self
    }

    /// Sets the silence duration before end-of-speech (ms).
    pub fn with_silence_duration(mut self, ms: u32) -> Self {
        self.silence_duration_ms = Some(ms);
        self
    }
}

/// Realtime input configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeInputConfig {
    /// Automatic activity detection settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_activity_detection: Option<AutomaticActivityDetection>,

    /// How activity affects the model's response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_handling: Option<ActivityHandling>,

    /// What input is included in the user's turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_coverage: Option<TurnCoverage>,
}

impl RealtimeInputConfig {
    /// Creates a new realtime input config with automatic VAD.
    pub fn automatic() -> Self {
        Self {
            automatic_activity_detection: Some(AutomaticActivityDetection::default()),
            activity_handling: Some(ActivityHandling::StartOfActivityInterrupts),
            turn_coverage: Some(TurnCoverage::TurnIncludesOnlyActivity),
        }
    }

    /// Creates a new realtime input config with manual activity control.
    pub fn manual() -> Self {
        Self {
            automatic_activity_detection: Some(AutomaticActivityDetection::disabled()),
            activity_handling: Some(ActivityHandling::StartOfActivityInterrupts),
            turn_coverage: Some(TurnCoverage::TurnIncludesOnlyActivity),
        }
    }

    /// Disables interruption (model won't be interrupted by user).
    pub fn with_no_interruption(mut self) -> Self {
        self.activity_handling = Some(ActivityHandling::NoInterruption);
        self
    }
}

/// VAD builder for easy configuration.
#[derive(Debug, Default)]
pub struct VadConfigBuilder {
    automatic: bool,
    start_sensitivity: StartSensitivity,
    end_sensitivity: EndSensitivity,
    prefix_padding_ms: u32,
    silence_duration_ms: u32,
    allow_interruption: bool,
}

impl VadConfigBuilder {
    /// Creates a new VAD config builder.
    pub fn new() -> Self {
        Self {
            automatic: true,
            start_sensitivity: StartSensitivity::StartSensitivityHigh,
            end_sensitivity: EndSensitivity::EndSensitivityHigh,
            prefix_padding_ms: 20,
            silence_duration_ms: 100,
            allow_interruption: true,
        }
    }

    /// Disables automatic VAD.
    pub fn manual(mut self) -> Self {
        self.automatic = false;
        self
    }

    /// Sets low start sensitivity.
    pub fn low_start_sensitivity(mut self) -> Self {
        self.start_sensitivity = StartSensitivity::StartSensitivityLow;
        self
    }

    /// Sets low end sensitivity.
    pub fn low_end_sensitivity(mut self) -> Self {
        self.end_sensitivity = EndSensitivity::EndSensitivityLow;
        self
    }

    /// Sets the prefix padding in milliseconds.
    pub fn prefix_padding(mut self, ms: u32) -> Self {
        self.prefix_padding_ms = ms;
        self
    }

    /// Sets the silence duration in milliseconds.
    pub fn silence_duration(mut self, ms: u32) -> Self {
        self.silence_duration_ms = ms;
        self
    }

    /// Disables interruption.
    pub fn no_interruption(mut self) -> Self {
        self.allow_interruption = false;
        self
    }

    /// Builds the realtime input config.
    pub fn build(self) -> RealtimeInputConfig {
        RealtimeInputConfig {
            automatic_activity_detection: Some(AutomaticActivityDetection {
                disabled: Some(!self.automatic),
                start_of_speech_sensitivity: Some(self.start_sensitivity),
                end_of_speech_sensitivity: Some(self.end_sensitivity),
                prefix_padding_ms: Some(self.prefix_padding_ms),
                silence_duration_ms: Some(self.silence_duration_ms),
            }),
            activity_handling: Some(if self.allow_interruption {
                ActivityHandling::StartOfActivityInterrupts
            } else {
                ActivityHandling::NoInterruption
            }),
            turn_coverage: Some(TurnCoverage::TurnIncludesOnlyActivity),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automatic_vad() {
        let config = RealtimeInputConfig::automatic();
        assert!(config.automatic_activity_detection.is_some());

        let vad = config.automatic_activity_detection.unwrap();
        assert_eq!(vad.disabled, Some(false));
    }

    #[test]
    fn test_manual_vad() {
        let config = RealtimeInputConfig::manual();
        let vad = config.automatic_activity_detection.unwrap();
        assert_eq!(vad.disabled, Some(true));
    }

    #[test]
    fn test_vad_builder() {
        let config = VadConfigBuilder::new()
            .low_start_sensitivity()
            .silence_duration(500)
            .no_interruption()
            .build();

        let vad = config.automatic_activity_detection.unwrap();
        assert_eq!(
            vad.start_of_speech_sensitivity,
            Some(StartSensitivity::StartSensitivityLow)
        );
        assert_eq!(vad.silence_duration_ms, Some(500));
        assert_eq!(
            config.activity_handling,
            Some(ActivityHandling::NoInterruption)
        );
    }
}

