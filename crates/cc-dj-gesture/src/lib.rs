//! # cc-dj-gesture: Gesture Recognition for DJ Agent
//!
//! This crate provides gesture recognition functionality for the DJ agent,
//! enabling motion-based control of DJ software.
//!
//! ## Components
//!
//! - [`GestureRecognizer`] - Real-time gesture recognition (basic/legacy)
//! - [`GestureDatabase`] - Storage for learned gestures
//! - [`GestureTrainer`] - Training system for custom gestures
//! - [`DJGestureRecognizer`] - Basic gesture-to-command recognizer
//! - [`GestureCommandMapping`] - Gesture-to-command mapping definitions

#![warn(missing_docs)]

pub mod database;
pub mod dj_recognizer;
pub mod recognizer;
pub mod trainer;
pub mod types;

pub use database::GestureDatabase;
pub use dj_recognizer::{DJGestureRecognizer, GestureCommandMapping};
pub use recognizer::{GestureRecognizer, RecognitionResult, RecognizerConfig};
pub use trainer::{GestureTrainer, TrainerState};
pub use types::{GestureType, MotionDataPoint, RecordedGesture};
