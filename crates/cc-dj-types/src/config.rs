//! Configuration types for the DJ agent.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Supported DJ software backends.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum DJSoftware {
    /// Pioneer Rekordbox.
    #[default]
    Rekordbox,
    /// Serato DJ.
    Serato,
}

impl std::fmt::Display for DJSoftware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DJSoftware::Rekordbox => write!(f, "rekordbox"),
            DJSoftware::Serato => write!(f, "serato"),
        }
    }
}

/// Main DJ agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DJConfig {
    /// DJ software selection.
    #[serde(default)]
    pub software: DJSoftware,

    /// Quantization window in degrees.
    #[serde(default = "default_quant_window")]
    pub quant_window_deg: f64,

    /// Cooldown periods per action type (in beats).
    #[serde(default)]
    pub cooldown_beats: HashMap<String, u32>,

    /// Which tiers are enabled (0-5).
    #[serde(default = "default_tiers")]
    pub tiers_enabled: Vec<u8>,

    /// Safety constraints.
    #[serde(default)]
    pub safety: SafetyConfig,

    /// Voice control configuration.
    #[serde(default)]
    pub voice: VoiceConfig,

    /// Reflex policy configuration.
    #[serde(default)]
    pub reflex: ReflexConfig,

    /// Reward weights for RL.
    #[serde(default)]
    pub rewards: RewardConfig,

    /// Software-specific settings.
    #[serde(default)]
    pub rekordbox: Option<SoftwareConfig>,

    /// Serato-specific settings.
    #[serde(default)]
    pub serato: Option<SoftwareConfig>,
}

fn default_quant_window() -> f64 {
    15.0
}

fn default_tiers() -> Vec<u8> {
    vec![0, 1, 2, 3]
}

impl Default for DJConfig {
    fn default() -> Self {
        Self {
            software: DJSoftware::default(),
            quant_window_deg: default_quant_window(),
            cooldown_beats: HashMap::new(),
            tiers_enabled: default_tiers(),
            safety: SafetyConfig::default(),
            voice: VoiceConfig::default(),
            reflex: ReflexConfig::default(),
            rewards: RewardConfig::default(),
            rekordbox: None,
            serato: None,
        }
    }
}

impl DJConfig {
    /// Loads configuration from a YAML file.
    pub fn from_file(path: impl AsRef<Path>) -> crate::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        Self::from_yaml(&contents)
    }

    /// Parses configuration from YAML string.
    pub fn from_yaml(yaml: &str) -> crate::Result<Self> {
        #[derive(Deserialize)]
        struct DJConfigFile {
            dj: DJConfig,
        }

        let file: DJConfigFile = serde_yml::from_str(yaml)?;
        Ok(file.dj)
    }

    /// Returns the active software config.
    pub fn software_config(&self) -> Option<&SoftwareConfig> {
        match self.software {
            DJSoftware::Rekordbox => self.rekordbox.as_ref(),
            DJSoftware::Serato => self.serato.as_ref(),
        }
    }
}

/// Safety constraints for DJ actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    /// Forbid stopping/modifying the playing deck.
    #[serde(default = "default_true")]
    pub lock_playing_deck: bool,

    /// Never load a track on a playing deck.
    #[serde(default = "default_true")]
    pub forbid_load_on_live: bool,

    /// Maximum EQ change per beat (dB).
    #[serde(default = "default_eq_max")]
    pub eq_max_db_per_beat: f64,

    /// Maximum crossfader change per beat (0-1 scale).
    #[serde(default = "default_crossfader_slope")]
    pub crossfader_max_slope: f64,
}

fn default_true() -> bool {
    true
}

fn default_eq_max() -> f64 {
    6.0
}

fn default_crossfader_slope() -> f64 {
    0.25
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            lock_playing_deck: true,
            forbid_load_on_live: true,
            eq_max_db_per_beat: 6.0,
            crossfader_max_slope: 0.25,
        }
    }
}

/// Voice control configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfig {
    /// Whether voice control is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Voice recognition engine.
    #[serde(default = "default_engine")]
    pub engine: String,

    /// Listening timeout in seconds.
    #[serde(default = "default_listen_timeout")]
    pub listen_timeout: f64,

    /// Maximum phrase duration in seconds.
    #[serde(default = "default_phrase_limit")]
    pub phrase_time_limit: f64,

    /// Custom command mappings.
    #[serde(default)]
    pub command_map: HashMap<String, String>,

    /// Whether to speak feedback.
    #[serde(default)]
    pub speak_feedback: bool,

    /// Whether to play audio feedback.
    #[serde(default = "default_true")]
    pub audio_feedback: bool,
}

fn default_engine() -> String {
    "gemini".to_string()
}

fn default_listen_timeout() -> f64 {
    1.0
}

fn default_phrase_limit() -> f64 {
    3.0
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            engine: default_engine(),
            listen_timeout: default_listen_timeout(),
            phrase_time_limit: default_phrase_limit(),
            command_map: HashMap::new(),
            speak_feedback: false,
            audio_feedback: true,
        }
    }
}

/// Reflex policy configuration (frame-rate continuous controls).
///
/// Reserved for future gesture-driven RL integration. Fields are deserialized
/// from config but not yet consumed by the runtime.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflexConfig {
    /// Filter cutoff frequency range [min, max] Hz.
    #[serde(default = "default_filter_range")]
    pub filter_cutoff_range: [f64; 2],

    /// Filter resonance (0.0-1.0).
    #[serde(default = "default_resonance")]
    pub filter_resonance: f64,

    /// Amplitude curve type ("log" or "linear").
    #[serde(default = "default_amp_curve")]
    pub amp_curve: String,

    /// Use limb asymmetry for stereo pan.
    #[serde(default = "default_true")]
    pub pan_from_limbs: bool,
}

fn default_filter_range() -> [f64; 2] {
    [1200.0, 6000.0]
}

fn default_resonance() -> f64 {
    0.5
}

fn default_amp_curve() -> String {
    "log".to_string()
}

impl Default for ReflexConfig {
    fn default() -> Self {
        Self {
            filter_cutoff_range: default_filter_range(),
            filter_resonance: default_resonance(),
            amp_curve: default_amp_curve(),
            pan_from_limbs: true,
        }
    }
}

/// Reward weights for reinforcement learning.
///
/// Reserved for future RL reward shaping. Fields are deserialized from config
/// but not yet consumed by the training loop.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardConfig {
    /// Phase alignment weight.
    #[serde(default = "default_one")]
    pub w_phase: f64,

    /// On-grid action bonus weight.
    #[serde(default = "default_1_2")]
    pub w_ongrid: f64,

    /// Harmonic compatibility weight.
    #[serde(default = "default_0_6")]
    pub w_key: f64,

    /// Smoothness weight.
    #[serde(default = "default_0_6")]
    pub w_smooth: f64,

    /// Energy contour match weight.
    #[serde(default = "default_0_4")]
    pub w_energy: f64,

    /// Anti-spam penalty weight.
    #[serde(default = "default_one")]
    pub w_spam: f64,

    /// Novelty bonus weight.
    #[serde(default = "default_0_2")]
    pub w_novel: f64,
}

fn default_one() -> f64 {
    1.0
}

fn default_1_2() -> f64 {
    1.2
}

fn default_0_6() -> f64 {
    0.6
}

fn default_0_4() -> f64 {
    0.4
}

fn default_0_2() -> f64 {
    0.2
}

impl Default for RewardConfig {
    fn default() -> Self {
        Self {
            w_phase: 1.0,
            w_ongrid: 1.2,
            w_key: 0.6,
            w_smooth: 0.6,
            w_energy: 0.4,
            w_spam: 1.0,
            w_novel: 0.2,
        }
    }
}

/// Software-specific configuration (Rekordbox or Serato).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SoftwareConfig {
    /// Communication mode ("midi" or "keyboard").
    #[serde(default = "default_mode")]
    pub mode: String,

    /// MIDI port name.
    pub midi_port: Option<String>,

    /// Action mappings.
    #[serde(default)]
    pub map: HashMap<String, ActionMapping>,
}

fn default_mode() -> String {
    "keyboard".to_string()
}

/// Mapping for a single action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionMapping {
    /// Single key press.
    Key {
        /// The key to press.
        key: String,
        /// Optional modifiers.
        #[serde(default)]
        modifiers: Vec<String>,
    },
    /// Sequence of key presses.
    Sequence {
        /// Steps in the sequence.
        steps: Vec<SequenceStep>,
    },
    /// MIDI message.
    Midi {
        /// MIDI channel.
        channel: u8,
        /// Note number.
        note: u8,
        /// Velocity.
        #[serde(default = "default_velocity")]
        velocity: u8,
    },
}

fn default_velocity() -> u8 {
    127
}

/// A step in an action sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceStep {
    /// Key to press.
    pub key: String,
    /// Optional modifiers.
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// Delay after this step (ms).
    #[serde(default)]
    pub delay_ms: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DJConfig::default();
        assert_eq!(config.software, DJSoftware::Rekordbox);
        assert_eq!(config.quant_window_deg, 15.0);
        assert!(config.safety.lock_playing_deck);
    }

    #[test]
    fn test_config_from_yaml() {
        let yaml = r#"
dj:
  software: serato
  quant_window_deg: 20
  tiers_enabled: [0, 1, 2]
  safety:
    lock_playing_deck: true
  voice:
    enabled: true
    engine: gemini
"#;

        let config = DJConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.software, DJSoftware::Serato);
        assert_eq!(config.quant_window_deg, 20.0);
        assert!(config.voice.enabled);
    }

    #[test]
    fn test_safety_defaults() {
        let safety = SafetyConfig::default();
        assert!(safety.lock_playing_deck);
        assert!(safety.forbid_load_on_live);
        assert_eq!(safety.eq_max_db_per_beat, 6.0);
    }
}
