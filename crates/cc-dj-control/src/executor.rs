//! Chain executor for executing action sequences.

use cc_dj_types::{Action, Result};
use std::time::Duration;
use tracing::{debug, info};

use crate::bridge::DJBridge;

/// Executor for action chains/sequences.
pub struct ChainExecutor {
    /// Delay between actions in a chain (ms).
    inter_action_delay_ms: u64,
}

impl ChainExecutor {
    /// Creates a new chain executor.
    pub fn new() -> Self {
        Self {
            inter_action_delay_ms: 50,
        }
    }

    /// Sets the inter-action delay.
    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.inter_action_delay_ms = delay_ms;
        self
    }

    /// Executes a chain of actions.
    pub async fn execute_chain(&self, actions: &[Action], bridge: &dyn DJBridge) -> Result<()> {
        info!("Executing chain of {} actions", actions.len());

        for (i, action) in actions.iter().enumerate() {
            debug!(
                "Executing action {} of {}: {}",
                i + 1,
                actions.len(),
                action.name
            );

            bridge.execute(action).await?;

            // Add delay between actions (except after the last one)
            if i < actions.len() - 1 && self.inter_action_delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(self.inter_action_delay_ms)).await;
            }
        }

        info!("Chain execution complete");
        Ok(())
    }

    /// Executes a batch of actions sequentially without inter-action delays.
    ///
    /// Despite the batch naming, actions are executed one-by-one because the
    /// bridge holds hardware resources (MIDI port, keyboard automation) that
    /// are not safe to drive concurrently.
    pub async fn execute_batch(&self, actions: &[Action], bridge: &dyn DJBridge) -> Result<()> {
        info!("Executing batch of {} actions", actions.len());

        for action in actions {
            bridge.execute(action).await?;
        }

        Ok(())
    }
}

impl Default for ChainExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::RekordboxBridge;
    use cc_dj_types::Tier;

    #[tokio::test]
    async fn test_chain_executes_all_actions_in_order() {
        let executor = ChainExecutor::new().with_delay(10);
        let bridge = RekordboxBridge::new(None).with_simulation();

        let actions = vec![
            Action::new("PLAY_A", Tier::Transport),
            Action::new("SYNC_A", Tier::Transport),
            Action::new("CUE_A", Tier::Transport),
        ];

        let result = executor.execute_chain(&actions, &bridge).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chain_empty_actions() {
        let executor = ChainExecutor::new();
        let bridge = RekordboxBridge::new(None).with_simulation();

        // Empty chain should succeed without doing anything
        assert!(executor.execute_chain(&[], &bridge).await.is_ok());
    }

    #[tokio::test]
    async fn test_batch_executes_without_delays() {
        let executor = ChainExecutor::new().with_delay(1000); // large delay
        let bridge = RekordboxBridge::new(None).with_simulation();

        let actions = vec![
            Action::new("PLAY_A", Tier::Transport),
            Action::new("SYNC_A", Tier::Transport),
        ];

        // Batch should complete quickly (no inter-action delays)
        let start = std::time::Instant::now();
        let result = executor.execute_batch(&actions, &bridge).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        // Should take less than 500ms (no 1000ms delays between actions)
        assert!(elapsed.as_millis() < 500);
    }
}
