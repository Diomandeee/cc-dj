//! DJ software bridge implementations.

mod rekordbox;
mod serato;

pub use rekordbox::RekordboxBridge;
pub use serato::SeratoBridge;

use std::sync::Arc;

use async_trait::async_trait;
use cc_dj_types::{Action, DJConfig, DJSoftware, Result};

/// Trait for DJ software bridges.
///
/// Bridges handle the actual execution of actions in DJ software,
/// translating abstract actions into keyboard shortcuts, MIDI messages,
/// or other platform-specific commands.
#[async_trait]
pub trait DJBridge: Send + Sync {
    /// Returns the name of the DJ software.
    fn name(&self) -> &'static str;

    /// Executes an action.
    async fn execute(&self, action: &Action) -> Result<()>;

    /// Checks if the bridge is available/connected.
    async fn is_available(&self) -> bool;

    /// Sends a keyboard shortcut.
    async fn send_key(&self, key: &str, modifiers: &[&str]) -> Result<()>;

    /// Sends a MIDI message.
    async fn send_midi(&self, channel: u8, note: u8, velocity: u8) -> Result<()>;
}

/// Creates a bridge for the configured DJ software.
pub fn create_bridge(config: &DJConfig) -> Arc<dyn DJBridge> {
    match config.software {
        DJSoftware::Serato => Arc::new(SeratoBridge::new(config.serato.clone())),
        DJSoftware::Rekordbox => Arc::new(RekordboxBridge::new(config.rekordbox.clone())),
    }
}
