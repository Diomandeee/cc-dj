//! # cc-dj-voice: Voice Control for DJ Agent
//!
//! This crate provides voice control functionality for the DJ agent,
//! using the Gemini Live API from `cc-gemini` for real-time speech recognition.
//!
//! ## Components
//!
//! - [`VoiceController`] - Main orchestrator for voice control
//! - [`CommandOrbiter`] - Embedding-based command retrieval
//! - [`IntentProcessor`] - Maps recognized speech to DJ commands

#![warn(missing_docs)]

pub mod controller;
pub mod intent;
pub mod mic;
pub mod orbiter;
pub mod semantic;

pub use controller::VoiceController;
pub use intent::IntentProcessor;
pub use orbiter::CommandOrbiter;

