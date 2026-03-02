//! Action definitions and action space management.
//!
//! Actions are the agent's interface to DJ software, organized into
//! progressive tiers with quantization, cooldowns, and safety masks.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::command::{Command, Deck};
use crate::config::SafetyConfig;
use crate::state::DeckState;
use crate::Result;

/// Action tiers (unlock progressively).
///
/// Tiers represent increasing levels of DJ control complexity.
/// Start with basic transport and progress to advanced mixing.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum Tier {
    /// Transport controls: PLAY, SYNC, PITCH_NUDGE.
    #[default]
    Transport = 0,
    /// Looping: SET_LOOP, LOOP_DOUBLE/HALVE, RELOOP/EXIT.
    Looping = 1,
    /// Cues: SET_CUE, JUMP_TO_CUE, CENSOR/SLIP.
    Cues = 2,
    /// Effects: FILTER, FX_TOGGLE, ECHO/REVERB_TAP.
    FX = 3,
    /// Library: LIB_SEARCH, LOAD_TO_DECK, TEMPO_ADJUST.
    Library = 4,
    /// Blending: CROSSFADER, EQ_BAND, SECTION_CUT, SAMPLE_TRIGGER.
    Blend = 5,
}

impl Tier {
    /// Returns the tier number.
    pub fn number(&self) -> u8 {
        *self as u8
    }

    /// Creates a tier from a number.
    pub fn from_number(n: u8) -> Option<Self> {
        match n {
            0 => Some(Self::Transport),
            1 => Some(Self::Looping),
            2 => Some(Self::Cues),
            3 => Some(Self::FX),
            4 => Some(Self::Library),
            5 => Some(Self::Blend),
            _ => None,
        }
    }

    /// Returns the tier name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Transport => "Transport",
            Self::Looping => "Looping",
            Self::Cues => "Cues",
            Self::FX => "FX",
            Self::Library => "Library",
            Self::Blend => "Blend",
        }
    }

    /// Returns all tiers in order.
    pub fn all() -> &'static [Tier] {
        &[
            Tier::Transport,
            Tier::Looping,
            Tier::Cues,
            Tier::FX,
            Tier::Library,
            Tier::Blend,
        ]
    }
}

/// A single action the DJ agent can perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Action identifier (e.g., "PLAY_A", "LOOP_4_A").
    pub name: String,

    /// Which tier this action belongs to.
    pub tier: Tier,

    /// Target deck (if deck-specific).
    pub deck: Option<Deck>,

    /// Optional parameters (e.g., loop_length, cutoff_value).
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,

    /// Whether action must fire at beat boundaries.
    #[serde(default = "default_quantized")]
    pub quantized: bool,

    /// Max phase error for quantized actions (degrees).
    #[serde(default = "default_quant_window")]
    pub quant_window_deg: f64,

    /// Minimum beats between repeated uses.
    #[serde(default = "default_cooldown_beats")]
    pub cooldown_beats: u32,

    /// The command this action executes.
    #[serde(skip)]
    pub command: Option<Command>,
}

fn default_quantized() -> bool {
    true
}

fn default_quant_window() -> f64 {
    15.0
}

fn default_cooldown_beats() -> u32 {
    2
}

impl Action {
    /// Creates a new action.
    pub fn new(name: impl Into<String>, tier: Tier) -> Self {
        Self {
            name: name.into(),
            tier,
            deck: None,
            params: HashMap::new(),
            quantized: true,
            quant_window_deg: 15.0,
            cooldown_beats: 2,
            command: None,
        }
    }

    /// Sets the target deck.
    pub fn with_deck(mut self, deck: Deck) -> Self {
        self.deck = Some(deck);
        self
    }

    /// Sets a parameter.
    pub fn with_param(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.params.insert(key.into(), v);
        }
        self
    }

    /// Sets quantization.
    pub fn with_quantization(mut self, quantized: bool, window_deg: f64) -> Self {
        self.quantized = quantized;
        self.quant_window_deg = window_deg;
        self
    }

    /// Sets cooldown.
    pub fn with_cooldown(mut self, beats: u32) -> Self {
        self.cooldown_beats = beats;
        self
    }

    /// Links to a command.
    pub fn with_command(mut self, command: Command) -> Self {
        self.command = Some(command);
        self
    }

    /// Checks if this action is allowed given current state and safety config.
    pub fn is_allowed(&self, state: &DeckState, safety: &SafetyConfig) -> Result<()> {
        // Check if deck is playing and we shouldn't modify it
        if let Some(deck) = &self.deck {
            if state.is_playing && safety.lock_playing_deck {
                // Only block destructive actions on playing deck
                if self.is_destructive() {
                    return Err(crate::DJError::action_not_allowed(format!(
                        "Cannot execute {} on playing deck {}",
                        self.name,
                        deck.letter()
                    )));
                }
            }
        }

        // Check for load on live deck
        if self.name.starts_with("LOAD") && state.is_playing && safety.forbid_load_on_live {
            return Err(crate::DJError::action_not_allowed(
                "Cannot load track on live deck",
            ));
        }

        Ok(())
    }

    /// Returns true if this action is destructive.
    fn is_destructive(&self) -> bool {
        self.name.starts_with("LOAD")
            || self.name.starts_with("EJECT")
            || self.name.starts_with("STOP")
    }
}

/// The complete action space with tier management and masking.
#[derive(Debug, Clone, Default)]
pub struct ActionSpace {
    /// All actions indexed by name.
    actions: HashMap<String, Action>,

    /// Which tiers are enabled.
    tiers_enabled: Vec<Tier>,

    /// Safety configuration.
    safety: SafetyConfig,

    /// Cooldown tracking (action name -> last execution beat).
    cooldowns: HashMap<String, f64>,
}

impl ActionSpace {
    /// Creates a new action space.
    pub fn new(tiers_enabled: Vec<Tier>, safety: SafetyConfig) -> Self {
        let mut space = Self {
            actions: HashMap::new(),
            tiers_enabled,
            safety,
            cooldowns: HashMap::new(),
        };
        space.build_default_actions();
        space
    }

    /// Builds the default action set.
    fn build_default_actions(&mut self) {
        // Tier 0: Transport
        self.add(Action::new("PLAY_A", Tier::Transport).with_deck(Deck::Left));
        self.add(Action::new("PLAY_B", Tier::Transport).with_deck(Deck::Right));
        self.add(Action::new("SYNC_A", Tier::Transport).with_deck(Deck::Left));
        self.add(Action::new("SYNC_B", Tier::Transport).with_deck(Deck::Right));
        self.add(
            Action::new("PITCH_NUDGE_UP_A", Tier::Transport)
                .with_deck(Deck::Left)
                .with_cooldown(1),
        );
        self.add(
            Action::new("PITCH_NUDGE_DOWN_A", Tier::Transport)
                .with_deck(Deck::Left)
                .with_cooldown(1),
        );

        // Tier 1: Looping
        self.add(Action::new("LOOP_4_A", Tier::Looping).with_deck(Deck::Left));
        self.add(Action::new("LOOP_4_B", Tier::Looping).with_deck(Deck::Right));
        self.add(Action::new("LOOP_DOUBLE_A", Tier::Looping).with_deck(Deck::Left));
        self.add(Action::new("LOOP_HALVE_A", Tier::Looping).with_deck(Deck::Left));
        self.add(Action::new("RELOOP_A", Tier::Looping).with_deck(Deck::Left));

        // Tier 2: Cues
        self.add(
            Action::new("JUMP_CUE_1_A", Tier::Cues)
                .with_deck(Deck::Left)
                .with_cooldown(4),
        );
        self.add(
            Action::new("JUMP_CUE_1_B", Tier::Cues)
                .with_deck(Deck::Right)
                .with_cooldown(4),
        );

        // Tier 3: FX
        self.add(
            Action::new("FX_TOGGLE_1_A", Tier::FX)
                .with_deck(Deck::Left)
                .with_cooldown(4),
        );
        self.add(
            Action::new("FX_TOGGLE_1_B", Tier::FX)
                .with_deck(Deck::Right)
                .with_cooldown(4),
        );
        self.add(Action::new("ECHO_TAP_A", Tier::FX).with_deck(Deck::Left));

        // Tier 4: Library
        self.add(Action::new("LIB_SEARCH", Tier::Library).with_cooldown(8));
        self.add(
            Action::new("LOAD_A", Tier::Library)
                .with_deck(Deck::Left)
                .with_cooldown(16),
        );
        self.add(
            Action::new("LOAD_B", Tier::Library)
                .with_deck(Deck::Right)
                .with_cooldown(16),
        );

        // Tier 5: Blend
        self.add(
            Action::new("CROSSFADER", Tier::Blend)
                .with_quantization(false, 0.0)
                .with_cooldown(1),
        );
        self.add(
            Action::new("EQ_LOW_A", Tier::Blend)
                .with_deck(Deck::Left)
                .with_quantization(false, 0.0),
        );
    }

    /// Adds an action to the space.
    pub fn add(&mut self, action: Action) {
        self.actions.insert(action.name.clone(), action);
    }

    /// Gets an action by name.
    pub fn get(&self, name: &str) -> Option<&Action> {
        self.actions.get(name)
    }

    /// Returns true if the given tier is enabled.
    pub fn is_tier_enabled(&self, tier: Tier) -> bool {
        self.tiers_enabled.contains(&tier)
    }

    /// Gets all available actions (respecting tier locks).
    pub fn available_actions(&self) -> Vec<&Action> {
        self.actions
            .values()
            .filter(|a| self.is_tier_enabled(a.tier))
            .collect()
    }

    /// Gets available actions for a specific tier.
    pub fn actions_in_tier(&self, tier: Tier) -> Vec<&Action> {
        self.actions.values().filter(|a| a.tier == tier).collect()
    }

    /// Checks if an action can be executed.
    pub fn can_execute(
        &self,
        action_name: &str,
        state: &DeckState,
        current_beat: f64,
    ) -> Result<()> {
        let action = self
            .get(action_name)
            .ok_or_else(|| crate::DJError::CommandNotFound(action_name.to_string()))?;

        // Check tier
        if !self.is_tier_enabled(action.tier) {
            return Err(crate::DJError::TierLocked {
                tier: action.tier.number(),
            });
        }

        // Check cooldown
        if let Some(&last_beat) = self.cooldowns.get(action_name) {
            let beats_since = current_beat - last_beat;
            if beats_since < action.cooldown_beats as f64 {
                return Err(crate::DJError::CooldownActive {
                    beats_remaining: action.cooldown_beats as f64 - beats_since,
                });
            }
        }

        // Check safety
        action.is_allowed(state, &self.safety)?;

        Ok(())
    }

    /// Records an action execution for cooldown tracking.
    pub fn record_execution(&mut self, action_name: &str, beat: f64) {
        self.cooldowns.insert(action_name.to_string(), beat);
    }

    /// Returns the number of actions.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_ordering() {
        assert!(Tier::Transport < Tier::Looping);
        assert!(Tier::Looping < Tier::Cues);
        assert!(Tier::FX < Tier::Library);
    }

    #[test]
    fn test_tier_from_number() {
        assert_eq!(Tier::from_number(0), Some(Tier::Transport));
        assert_eq!(Tier::from_number(5), Some(Tier::Blend));
        assert_eq!(Tier::from_number(10), None);
    }

    #[test]
    fn test_action_builder() {
        let action = Action::new("PLAY_A", Tier::Transport)
            .with_deck(Deck::Left)
            .with_cooldown(1)
            .with_param("velocity", 127);

        assert_eq!(action.name, "PLAY_A");
        assert_eq!(action.tier, Tier::Transport);
        assert_eq!(action.deck, Some(Deck::Left));
        assert_eq!(action.cooldown_beats, 1);
    }

    #[test]
    fn test_action_space() {
        let space = ActionSpace::new(
            vec![Tier::Transport, Tier::Looping],
            SafetyConfig::default(),
        );

        assert!(space.is_tier_enabled(Tier::Transport));
        assert!(space.is_tier_enabled(Tier::Looping));
        assert!(!space.is_tier_enabled(Tier::FX));

        let transport_actions = space.actions_in_tier(Tier::Transport);
        assert!(!transport_actions.is_empty());
    }
}
