//! # cc-dj-types: Core Types for DJ Agent
//!
//! This crate provides the foundational types for the DJ Agent system,
//! including commands, actions, tiers, and state management.
//!
//! ## Schema Version
//!
//! **FROZEN at v0.1.0**. Breaking changes require a major version bump.
//!
//! ## Core Types
//!
//! - [`Command`] - DJ software command (e.g., Play, Loop, Cue)
//! - [`Action`] - Agent action with quantization and safety
//! - [`Tier`] - Progressive unlock tiers for actions
//! - [`DeckState`] - Current state of a DJ deck
//! - [`DJConfig`] - Configuration for the DJ agent

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod action;
pub mod command;
pub mod config;
pub mod error;
pub mod state;

// Re-exports
pub use action::{Action, ActionSpace, Tier};
pub use command::{ActionType, Command, CommandCatalog, CommandCategory, Deck, Shortcut};
pub use config::{
    ActionMapping, DJConfig, DJSoftware, SafetyConfig, SequenceStep, SoftwareConfig, VoiceConfig,
};
pub use error::{DJError, Result};
pub use state::{DeckState, MixerState, SessionState};

/// Schema version for cc-dj-types.
///
/// **FROZEN after v0.1.0 lock**. Changing this requires major version bump.
pub const SCHEMA_VERSION: &str = "0.1.0";

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::action::{Action, ActionSpace, Tier};
    pub use crate::command::{Command, CommandCategory, Deck, Shortcut};
    pub use crate::config::DJConfig;
    pub use crate::error::{DJError, Result};
    pub use crate::state::{DeckState, MixerState};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version() {
        assert_eq!(SCHEMA_VERSION, "0.1.0");
    }
}
