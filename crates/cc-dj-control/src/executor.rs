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
    pub async fn execute_chain(
        &self,
        actions: &[Action],
        bridge: &dyn DJBridge,
    ) -> Result<()> {
        info!("Executing chain of {} actions", actions.len());

        for (i, action) in actions.iter().enumerate() {
            debug!("Executing action {} of {}: {}", i + 1, actions.len(), action.name);
            
            bridge.execute(action).await?;

            // Add delay between actions (except after the last one)
            if i < actions.len() - 1 && self.inter_action_delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(self.inter_action_delay_ms)).await;
            }
        }

        info!("Chain execution complete");
        Ok(())
    }

    /// Executes actions in parallel.
    pub async fn execute_parallel(
        &self,
        actions: &[Action],
        bridge: &dyn DJBridge,
    ) -> Result<()> {
        info!("Executing {} actions in parallel", actions.len());

        // For now, execute sequentially without delays
        // True parallel execution would need Arc<dyn DJBridge>
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
    async fn test_chain_executor() {
        let executor = ChainExecutor::new().with_delay(10);
        let bridge = RekordboxBridge::new(None).with_simulation();

        let actions = vec![
            Action::new("PLAY_A", Tier::Transport),
            Action::new("SYNC_A", Tier::Transport),
        ];

        let result = executor.execute_chain(&actions, &bridge).await;
        assert!(result.is_ok());
    }
}

