//! # cc-dj-control: DJ Software Control
//!
//! This crate provides the bridge layer between the DJ agent and
//! DJ software like Rekordbox and Serato.
//!
//! ## Components
//!
//! - [`DeckController`] - High-level deck control
//! - [`DJBridge`] - Trait for software-specific bridges
//! - [`ActionScheduler`] - Beat-quantized action scheduling
//! - [`ChainExecutor`] - Executes action sequences

#![warn(missing_docs)]

pub mod bridge;
pub mod deck;
pub mod executor;
pub mod scheduler;

pub use bridge::{DJBridge, RekordboxBridge, SeratoBridge};
pub use deck::DeckController;
pub use executor::ChainExecutor;
pub use scheduler::ActionScheduler;
