//! State types for DJ deck and mixer tracking.
//!
//! These types represent the current state of DJ software,
//! enabling the agent to make informed decisions.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Current state of a DJ deck.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeckState {
    /// Whether the deck is currently playing.
    pub is_playing: bool,

    /// Current BPM.
    pub bpm: f64,

    /// Current position in the track (seconds).
    pub position_secs: f64,

    /// Track duration (seconds).
    pub duration_secs: f64,

    /// Current beat position (fractional).
    pub beat_position: f64,

    /// Phase relative to master (0.0-1.0).
    pub phase: f64,

    /// Whether loop is active.
    pub loop_active: bool,

    /// Loop length in beats (if active).
    pub loop_beats: Option<f64>,

    /// Currently loaded track info.
    pub track: Option<TrackInfo>,

    /// Whether this deck is the sync master.
    pub is_master: bool,

    /// Pitch/tempo adjustment (percentage, 0.0 = no change).
    pub pitch_percent: f64,

    /// Key of the track.
    pub key: Option<String>,

    /// Last update timestamp.
    #[serde(skip)]
    pub last_update: Option<std::time::Instant>,
}

impl DeckState {
    /// Creates a new empty deck state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the remaining time in the track.
    pub fn remaining_secs(&self) -> f64 {
        (self.duration_secs - self.position_secs).max(0.0)
    }

    /// Returns the progress through the track (0.0-1.0).
    pub fn progress(&self) -> f64 {
        if self.duration_secs > 0.0 {
            (self.position_secs / self.duration_secs).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Returns true if the track is near the end (last 30 seconds).
    pub fn is_near_end(&self) -> bool {
        self.remaining_secs() < 30.0
    }

    /// Returns the phase error from the beat grid (degrees).
    pub fn phase_error_deg(&self) -> f64 {
        let phase_fraction = self.beat_position.fract();
        if phase_fraction <= 0.5 {
            phase_fraction * 360.0
        } else {
            (phase_fraction - 1.0) * 360.0
        }
    }

    /// Returns true if we're on the beat (within tolerance).
    pub fn is_on_beat(&self, tolerance_deg: f64) -> bool {
        self.phase_error_deg().abs() <= tolerance_deg
    }

    /// Returns the effective BPM (accounting for pitch adjustment).
    pub fn effective_bpm(&self) -> f64 {
        self.bpm * (1.0 + self.pitch_percent / 100.0)
    }
}

/// Information about a loaded track.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackInfo {
    /// Track title.
    pub title: Option<String>,

    /// Artist name.
    pub artist: Option<String>,

    /// Album name.
    pub album: Option<String>,

    /// Original BPM (before pitch adjustment).
    pub bpm: f64,

    /// Key (Camelot or standard notation).
    pub key: Option<String>,

    /// Genre.
    pub genre: Option<String>,

    /// Energy level (1-10).
    pub energy: Option<u8>,

    /// File path.
    pub path: Option<String>,
}

/// State of the mixer section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MixerState {
    /// Crossfader position (-1.0 = full left, 1.0 = full right).
    pub crossfader: f64,

    /// Master volume (0.0-1.0).
    pub master_volume: f64,

    /// Headphone cue mix (0.0 = cue only, 1.0 = master only).
    pub cue_mix: f64,

    /// Per-deck channel state.
    pub channels: Vec<ChannelState>,
}

impl MixerState {
    /// Creates a new mixer state with default values.
    pub fn new(num_decks: usize) -> Self {
        Self {
            crossfader: 0.0,
            master_volume: 1.0,
            cue_mix: 0.5,
            channels: (0..num_decks).map(|_| ChannelState::default()).collect(),
        }
    }

    /// Gets the channel state for a deck (0-indexed).
    pub fn channel(&self, deck: usize) -> Option<&ChannelState> {
        self.channels.get(deck)
    }

    /// Gets mutable channel state for a deck.
    pub fn channel_mut(&mut self, deck: usize) -> Option<&mut ChannelState> {
        self.channels.get_mut(deck)
    }
}

/// State of a single mixer channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelState {
    /// Volume fader (0.0-1.0).
    pub volume: f64,

    /// Low EQ (-1.0 to 1.0, 0.0 = unity).
    pub eq_low: f64,

    /// Mid EQ (-1.0 to 1.0, 0.0 = unity).
    pub eq_mid: f64,

    /// High EQ (-1.0 to 1.0, 0.0 = unity).
    pub eq_high: f64,

    /// Filter cutoff (0.0-1.0, 0.5 = bypass).
    pub filter: f64,

    /// Whether cue/PFL is enabled.
    pub cue_enabled: bool,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self {
            volume: 1.0,
            eq_low: 0.0,
            eq_mid: 0.0,
            eq_high: 0.0,
            filter: 0.5,
            cue_enabled: false,
        }
    }
}

/// Overall session state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    /// States of all decks.
    pub decks: Vec<DeckState>,

    /// Mixer state.
    pub mixer: MixerState,

    /// Current session duration.
    #[serde(with = "duration_serde")]
    pub session_duration: Duration,

    /// Number of transitions performed.
    pub transition_count: u32,

    /// Current energy level (subjective 1-10).
    pub energy_level: u8,

    /// Whether auto-DJ is active.
    pub auto_dj_active: bool,
}

mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs_f64().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = f64::deserialize(deserializer)?;
        Ok(Duration::from_secs_f64(secs))
    }
}

impl SessionState {
    /// Creates a new session state with the given number of decks.
    pub fn new(num_decks: usize) -> Self {
        Self {
            decks: (0..num_decks).map(|_| DeckState::default()).collect(),
            mixer: MixerState::new(num_decks),
            session_duration: Duration::ZERO,
            transition_count: 0,
            energy_level: 5,
            auto_dj_active: false,
        }
    }

    /// Gets the state of a specific deck.
    pub fn deck(&self, index: usize) -> Option<&DeckState> {
        self.decks.get(index)
    }

    /// Gets mutable deck state.
    pub fn deck_mut(&mut self, index: usize) -> Option<&mut DeckState> {
        self.decks.get_mut(index)
    }

    /// Returns the master deck (the one with is_master = true).
    pub fn master_deck(&self) -> Option<&DeckState> {
        self.decks.iter().find(|d| d.is_master)
    }

    /// Returns the current master BPM.
    pub fn master_bpm(&self) -> Option<f64> {
        self.master_deck().map(|d| d.effective_bpm())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck_state() {
        let mut deck = DeckState::new();
        deck.position_secs = 120.0;
        deck.duration_secs = 300.0;
        deck.bpm = 128.0;
        deck.is_playing = true;

        assert_eq!(deck.remaining_secs(), 180.0);
        assert!((deck.progress() - 0.4).abs() < 0.001);
        assert!(!deck.is_near_end());
    }

    #[test]
    fn test_phase_error() {
        let mut deck = DeckState::new();

        deck.beat_position = 0.0;
        assert!(deck.phase_error_deg().abs() < 0.001);

        deck.beat_position = 0.5;
        assert!((deck.phase_error_deg() - 180.0).abs() < 0.001);

        deck.beat_position = 0.9;
        assert!((deck.phase_error_deg() - (-36.0)).abs() < 0.001);
    }

    #[test]
    fn test_session_state() {
        let session = SessionState::new(2);
        assert_eq!(session.decks.len(), 2);
        assert_eq!(session.mixer.channels.len(), 2);
    }
}
