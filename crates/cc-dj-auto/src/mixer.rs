//! Auto mixer for automated DJ mixing.

use cc_dj_types::{DeckState, Result, SessionState};
use cc_dj_control::DeckController;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::analyzer::{TrackAnalysis, TrackAnalyzer};
use crate::strategy::MixStrategy;
use crate::transition::{TransitionAdvisor, TransitionRecommendation};

/// State of the auto mixer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoMixerState {
    /// Idle, not mixing.
    Idle,
    /// Playing, monitoring for transition.
    Playing,
    /// Preparing for transition.
    Preparing,
    /// Actively transitioning.
    Transitioning,
    /// Paused.
    Paused,
}

/// Automated DJ mixer.
pub struct AutoMixer {
    /// Current state.
    state: AutoMixerState,
    /// Mixing strategy.
    strategy: MixStrategy,
    /// Track analyzer.
    analyzer: TrackAnalyzer,
    /// Transition advisor.
    advisor: TransitionAdvisor,
    /// Current transition recommendation.
    current_recommendation: Option<TransitionRecommendation>,
    /// Track queue.
    queue: Vec<String>,
    /// Analysis cache.
    analysis_cache: std::collections::HashMap<String, TrackAnalysis>,
}

impl AutoMixer {
    /// Creates a new auto mixer.
    pub fn new(strategy: MixStrategy) -> Self {
        Self {
            state: AutoMixerState::Idle,
            strategy,
            analyzer: TrackAnalyzer::new(),
            advisor: TransitionAdvisor::new(),
            current_recommendation: None,
            queue: Vec::new(),
            analysis_cache: std::collections::HashMap::new(),
        }
    }

    /// Creates an auto mixer with default strategy.
    pub fn default_strategy() -> Self {
        Self::new(MixStrategy::default())
    }

    /// Sets the mixing strategy.
    pub fn with_strategy(mut self, strategy: MixStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Adds a track to the queue.
    pub fn add_to_queue(&mut self, track_path: impl Into<String>) {
        self.queue.push(track_path.into());
    }

    /// Clears the queue.
    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    /// Returns the queue.
    pub fn queue(&self) -> &[String] {
        &self.queue
    }

    /// Starts auto mixing.
    pub fn start(&mut self) {
        info!("Starting auto mixer with strategy: {}", self.strategy.name);
        self.state = AutoMixerState::Playing;
    }

    /// Stops auto mixing.
    pub fn stop(&mut self) {
        info!("Stopping auto mixer");
        self.state = AutoMixerState::Idle;
        self.current_recommendation = None;
    }

    /// Pauses auto mixing.
    pub fn pause(&mut self) {
        if self.state != AutoMixerState::Idle {
            self.state = AutoMixerState::Paused;
        }
    }

    /// Resumes auto mixing.
    pub fn resume(&mut self) {
        if self.state == AutoMixerState::Paused {
            self.state = AutoMixerState::Playing;
        }
    }

    /// Returns the current state.
    pub fn state(&self) -> AutoMixerState {
        self.state
    }

    /// Updates the mixer with current session state.
    pub async fn update(&mut self, session: &SessionState) -> Result<Option<TransitionRecommendation>> {
        match self.state {
            AutoMixerState::Idle | AutoMixerState::Paused => return Ok(None),
            AutoMixerState::Playing => {
                // Check if we should prepare for transition
                if let Some(deck) = session.decks.first() {
                    if self.advisor.should_transition(deck, 60.0) {
                        self.state = AutoMixerState::Preparing;
                        debug!("Preparing for transition");
                    }
                }
            }
            AutoMixerState::Preparing => {
                // Generate recommendation if we don't have one
                if self.current_recommendation.is_none() {
                    if let (Some(outgoing), Some(incoming)) = (
                        session.decks.first(),
                        session.decks.get(1),
                    ) {
                        let outgoing_analysis = self.get_or_analyze_track(outgoing).await?;
                        let incoming_analysis = self.get_or_analyze_track(incoming).await?;

                        let rec = self.advisor.recommend(
                            outgoing,
                            &outgoing_analysis,
                            &incoming_analysis,
                        )?;

                        info!("Transition recommendation: {:?}", rec.style);
                        self.current_recommendation = Some(rec.clone());
                        return Ok(Some(rec));
                    }
                }

                // Check if it's time to start transitioning
                if let Some(rec) = &self.current_recommendation {
                    if let Some(deck) = session.decks.first() {
                        let beats_remaining = deck.remaining_secs() * deck.effective_bpm() / 60.0;
                        if beats_remaining <= rec.start_in_beats {
                            self.state = AutoMixerState::Transitioning;
                            debug!("Starting transition");
                        }
                    }
                }
            }
            AutoMixerState::Transitioning => {
                // Monitor transition progress
                // When complete, reset to Playing
                if let Some(rec) = &self.current_recommendation {
                    if let Some(deck) = session.decks.first() {
                        let beats_remaining = deck.remaining_secs() * deck.effective_bpm() / 60.0;
                        if beats_remaining <= 0.0 {
                            self.state = AutoMixerState::Playing;
                            self.current_recommendation = None;
                            info!("Transition complete");
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Gets or analyzes a track.
    async fn get_or_analyze_track(&mut self, deck: &DeckState) -> Result<TrackAnalysis> {
        if let Some(track) = &deck.track {
            if let Some(path) = &track.path {
                if let Some(analysis) = self.analysis_cache.get(path) {
                    return Ok(analysis.clone());
                }

                let analysis = self.analyzer.analyze(path).await?;
                self.analysis_cache.insert(path.clone(), analysis.clone());
                return Ok(analysis);
            }
        }

        // Return default analysis if no track info
        Ok(TrackAnalysis::default())
    }

    /// Returns the current recommendation.
    pub fn current_recommendation(&self) -> Option<&TransitionRecommendation> {
        self.current_recommendation.as_ref()
    }
}

impl Default for AutoMixer {
    fn default() -> Self {
        Self::default_strategy()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_mixer_lifecycle() {
        let mut mixer = AutoMixer::default_strategy();
        
        assert_eq!(mixer.state(), AutoMixerState::Idle);
        
        mixer.start();
        assert_eq!(mixer.state(), AutoMixerState::Playing);
        
        mixer.pause();
        assert_eq!(mixer.state(), AutoMixerState::Paused);
        
        mixer.resume();
        assert_eq!(mixer.state(), AutoMixerState::Playing);
        
        mixer.stop();
        assert_eq!(mixer.state(), AutoMixerState::Idle);
    }

    #[test]
    fn test_queue_management() {
        let mut mixer = AutoMixer::default_strategy();
        
        mixer.add_to_queue("/path/to/track1.mp3");
        mixer.add_to_queue("/path/to/track2.mp3");
        
        assert_eq!(mixer.queue().len(), 2);
        
        mixer.clear_queue();
        assert!(mixer.queue().is_empty());
    }
}

