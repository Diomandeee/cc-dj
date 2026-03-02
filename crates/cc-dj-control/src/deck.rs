//! Deck controller for high-level deck operations.

use cc_dj_types::{Action, ActionSpace, DeckState, DJConfig, Result, Tier};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::bridge::{create_bridge, DJBridge};
use crate::scheduler::ActionScheduler;

/// High-level controller for a DJ deck.
pub struct DeckController {
    /// DJ configuration.
    config: Arc<DJConfig>,
    /// Action space.
    action_space: ActionSpace,
    /// DJ software bridge.
    bridge: Box<dyn DJBridge>,
    /// Action scheduler.
    scheduler: ActionScheduler,
    /// Current deck states.
    deck_states: Arc<RwLock<Vec<DeckState>>>,
    /// Current beat position.
    current_beat: f64,
}

impl DeckController {
    /// Creates a new deck controller.
    pub fn new(config: DJConfig) -> Self {
        let config = Arc::new(config);
        let tiers: Vec<Tier> = config
            .tiers_enabled
            .iter()
            .filter_map(|&n| Tier::from_number(n))
            .collect();
        
        let action_space = ActionSpace::new(tiers, config.safety.clone());
        let bridge = create_bridge(&config);
        let scheduler = ActionScheduler::new(config.quant_window_deg);

        Self {
            config,
            action_space,
            bridge,
            scheduler,
            deck_states: Arc::new(RwLock::new(vec![DeckState::default(); 2])),
            current_beat: 0.0,
        }
    }

    /// Executes an action by name.
    pub async fn execute(&mut self, action_name: &str) -> Result<()> {
        info!("Executing action: {}", action_name);

        // Get deck state for validation
        let states = self.deck_states.read().await;
        let deck_state = states.first().unwrap_or(&DeckState::default()).clone();
        drop(states);

        // Check if action is allowed
        self.action_space.can_execute(action_name, &deck_state, self.current_beat)?;

        // Get the action
        let action = self.action_space.get(action_name)
            .ok_or_else(|| cc_dj_types::DJError::CommandNotFound(action_name.to_string()))?
            .clone();

        // Check quantization if needed
        if action.quantized {
            let phase_error = deck_state.phase_error_deg();
            if phase_error.abs() > action.quant_window_deg {
                debug!("Scheduling action for next beat (phase error: {}°)", phase_error);
                self.scheduler.schedule(action.clone(), self.current_beat);
                return Ok(());
            }
        }

        // Execute immediately
        self.bridge.execute(&action).await?;
        
        // Record execution for cooldown tracking
        self.action_space.record_execution(action_name, self.current_beat);

        Ok(())
    }

    /// Updates the current beat position.
    pub fn update_beat(&mut self, beat: f64) {
        self.current_beat = beat;
        
        // Check for scheduled actions
        if let Some(action) = self.scheduler.poll(beat) {
            let bridge = &self.bridge;
            let action_space = &mut self.action_space;
            let action_name = action.name.clone();
            
            tokio::spawn({
                let action = action.clone();
                let bridge_name = bridge.name().to_string();
                async move {
                    debug!("Executing scheduled action: {} via {}", action.name, bridge_name);
                }
            });
            
            action_space.record_execution(&action_name, beat);
        }
    }

    /// Updates a deck's state.
    pub async fn update_deck_state(&self, deck_index: usize, state: DeckState) {
        let mut states = self.deck_states.write().await;
        if deck_index < states.len() {
            states[deck_index] = state;
        }
    }

    /// Returns the action space.
    pub fn action_space(&self) -> &ActionSpace {
        &self.action_space
    }

    /// Returns the bridge name.
    pub fn bridge_name(&self) -> &'static str {
        self.bridge.name()
    }

    /// Returns the current beat.
    pub fn current_beat(&self) -> f64 {
        self.current_beat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_deck_controller() {
        let config = DJConfig::default();
        let mut controller = DeckController::new(config);

        assert_eq!(controller.bridge_name(), "Rekordbox");
        assert_eq!(controller.current_beat(), 0.0);

        controller.update_beat(4.0);
        assert_eq!(controller.current_beat(), 4.0);
    }
}

