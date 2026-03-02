//! Mixing strategies for Auto-DJ.

use serde::{Deserialize, Serialize};

/// A mixing strategy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixStrategy {
    /// Strategy name.
    pub name: String,
    /// Preferred transition duration (beats).
    pub transition_duration_beats: f64,
    /// Whether to match BPM automatically.
    pub auto_sync: bool,
    /// Whether to consider key compatibility.
    pub harmonic_mixing: bool,
    /// Energy curve preference.
    pub energy_curve: EnergyCurve,
    /// Transition style preferences.
    pub preferred_styles: Vec<super::transition::TransitionStyle>,
}

/// Energy curve preference for a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnergyCurve {
    /// Gradually increase energy throughout.
    Build,
    /// Maintain consistent energy.
    Steady,
    /// Wave pattern (build and release).
    Wave,
    /// Peak early, then maintain.
    FrontLoad,
    /// Build to peak at end.
    Climax,
}

impl Default for MixStrategy {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            transition_duration_beats: 16.0,
            auto_sync: true,
            harmonic_mixing: true,
            energy_curve: EnergyCurve::Build,
            preferred_styles: vec![
                super::transition::TransitionStyle::Fade,
                super::transition::TransitionStyle::EqSwap,
            ],
        }
    }
}

impl MixStrategy {
    /// Creates a new strategy with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Creates a minimal "just play" strategy.
    pub fn minimal() -> Self {
        Self {
            name: "minimal".to_string(),
            transition_duration_beats: 4.0,
            auto_sync: false,
            harmonic_mixing: false,
            energy_curve: EnergyCurve::Steady,
            preferred_styles: vec![super::transition::TransitionStyle::Cut],
        }
    }

    /// Creates a club/peak-time strategy.
    pub fn club() -> Self {
        Self {
            name: "club".to_string(),
            transition_duration_beats: 32.0,
            auto_sync: true,
            harmonic_mixing: true,
            energy_curve: EnergyCurve::Wave,
            preferred_styles: vec![
                super::transition::TransitionStyle::EqSwap,
                super::transition::TransitionStyle::Fade,
            ],
        }
    }

    /// Creates a lounge/chill strategy.
    pub fn lounge() -> Self {
        Self {
            name: "lounge".to_string(),
            transition_duration_beats: 64.0,
            auto_sync: true,
            harmonic_mixing: true,
            energy_curve: EnergyCurve::Steady,
            preferred_styles: vec![
                super::transition::TransitionStyle::Fade,
                super::transition::TransitionStyle::LoopFade,
            ],
        }
    }

    /// Sets the transition duration.
    pub fn with_transition_duration(mut self, beats: f64) -> Self {
        self.transition_duration_beats = beats;
        self
    }

    /// Sets auto-sync preference.
    pub fn with_auto_sync(mut self, enabled: bool) -> Self {
        self.auto_sync = enabled;
        self
    }

    /// Sets harmonic mixing preference.
    pub fn with_harmonic_mixing(mut self, enabled: bool) -> Self {
        self.harmonic_mixing = enabled;
        self
    }

    /// Sets the energy curve.
    pub fn with_energy_curve(mut self, curve: EnergyCurve) -> Self {
        self.energy_curve = curve;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_strategy() {
        let strategy = MixStrategy::default();
        assert_eq!(strategy.transition_duration_beats, 16.0);
        assert!(strategy.auto_sync);
        assert!(strategy.harmonic_mixing);
    }

    #[test]
    fn test_preset_strategies() {
        let minimal = MixStrategy::minimal();
        assert_eq!(minimal.transition_duration_beats, 4.0);
        assert!(!minimal.auto_sync);

        let club = MixStrategy::club();
        assert_eq!(club.transition_duration_beats, 32.0);
        assert_eq!(club.energy_curve, EnergyCurve::Wave);

        let lounge = MixStrategy::lounge();
        assert_eq!(lounge.transition_duration_beats, 64.0);
    }
}

