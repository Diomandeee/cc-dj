//! Transition advisor for recommending optimal mix points.

use crate::analyzer::{SectionType, TrackAnalysis};
use cc_dj_types::{DeckState, Result};
use serde::{Deserialize, Serialize};

/// A recommended transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionRecommendation {
    /// When to start the transition (beats from now).
    pub start_in_beats: f64,
    /// Duration of the transition (beats).
    pub duration_beats: f64,
    /// Transition style.
    pub style: TransitionStyle,
    /// Confidence score (0.0-1.0).
    pub confidence: f32,
    /// Reason for this recommendation.
    pub reason: String,
}

/// Style of transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionStyle {
    /// Quick cut on the beat.
    Cut,
    /// Gradual crossfade.
    Fade,
    /// EQ swap (kill lows on outgoing, bring in on incoming).
    EqSwap,
    /// Echo out effect.
    EchoOut,
    /// Backspin transition.
    Backspin,
    /// Loop and fade.
    LoopFade,
}

/// Advisor for transition timing and style.
pub struct TransitionAdvisor {
    /// Minimum beats before transition.
    min_lead_time_beats: f64,
    /// Preferred transition duration.
    preferred_duration_beats: f64,
}

impl TransitionAdvisor {
    /// Creates a new transition advisor.
    pub fn new() -> Self {
        Self {
            min_lead_time_beats: 8.0,
            preferred_duration_beats: 16.0,
        }
    }

    /// Sets the minimum lead time.
    pub fn with_min_lead_time(mut self, beats: f64) -> Self {
        self.min_lead_time_beats = beats;
        self
    }

    /// Recommends a transition based on current state and track analysis.
    pub fn recommend(
        &self,
        outgoing_state: &DeckState,
        outgoing_analysis: &TrackAnalysis,
        incoming_analysis: &TrackAnalysis,
    ) -> Result<TransitionRecommendation> {
        // Find the next good transition point
        let current_pos = outgoing_state.position_secs;
        let remaining = outgoing_state.remaining_secs();
        let bpm = outgoing_state.effective_bpm();

        // Calculate beats remaining
        let beats_remaining = remaining * bpm / 60.0;

        // Find next section boundary
        let next_boundary = outgoing_analysis
            .sections
            .iter()
            .find(|s| s.end_secs > current_pos)
            .map(|s| s.end_secs);

        // Determine transition style based on sections
        let outgoing_section = outgoing_analysis
            .sections
            .iter()
            .find(|s| s.start_secs <= current_pos && s.end_secs > current_pos);

        let style = match outgoing_section.map(|s| s.section_type) {
            Some(SectionType::Drop) => TransitionStyle::EqSwap,
            Some(SectionType::Breakdown) => TransitionStyle::Fade,
            Some(SectionType::Outro) => TransitionStyle::Fade,
            _ => TransitionStyle::Cut,
        };

        // Calculate when to start
        let start_in_beats = if let Some(boundary) = next_boundary {
            let beats_to_boundary = (boundary - current_pos) * bpm / 60.0;
            beats_to_boundary - self.preferred_duration_beats
        } else {
            beats_remaining - self.preferred_duration_beats - self.min_lead_time_beats
        };

        let start_in_beats = start_in_beats.max(self.min_lead_time_beats);

        // Calculate confidence based on available info
        let mut confidence = 0.5;
        if outgoing_analysis.key.is_some() && incoming_analysis.key.is_some() {
            confidence += 0.2;
        }
        if !outgoing_analysis.sections.is_empty() {
            confidence += 0.2;
        }
        if outgoing_analysis.mix_points.mix_out.is_some() {
            confidence += 0.1;
        }

        let reason = format!(
            "Transition at {} section using {} style",
            outgoing_section
                .map(|s| format!("{:?}", s.section_type))
                .unwrap_or_else(|| "unknown".to_string()),
            format!("{:?}", style).to_lowercase()
        );

        Ok(TransitionRecommendation {
            start_in_beats,
            duration_beats: self.preferred_duration_beats,
            style,
            confidence,
            reason,
        })
    }

    /// Checks if it's time to start a transition.
    pub fn should_transition(
        &self,
        outgoing_state: &DeckState,
        threshold_secs: f64,
    ) -> bool {
        outgoing_state.remaining_secs() <= threshold_secs
    }
}

impl Default for TransitionAdvisor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_advisor() {
        let advisor = TransitionAdvisor::new();

        let mut outgoing_state = DeckState::default();
        outgoing_state.position_secs = 180.0;
        outgoing_state.duration_secs = 240.0;
        outgoing_state.bpm = 128.0;
        outgoing_state.is_playing = true;

        let outgoing_analysis = TrackAnalysis::default();
        let incoming_analysis = TrackAnalysis::default();

        let rec = advisor
            .recommend(&outgoing_state, &outgoing_analysis, &incoming_analysis)
            .unwrap();

        assert!(rec.start_in_beats > 0.0);
        assert!(rec.confidence > 0.0);
    }

    #[test]
    fn test_should_transition() {
        let advisor = TransitionAdvisor::new();

        let mut state = DeckState::default();
        state.position_secs = 170.0;
        state.duration_secs = 180.0;

        assert!(advisor.should_transition(&state, 15.0));
        assert!(!advisor.should_transition(&state, 5.0));
    }
}

