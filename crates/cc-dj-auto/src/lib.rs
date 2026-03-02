//! # cc-dj-auto: Auto-DJ Features
//!
//! This crate provides automatic DJ features including track analysis,
//! transition recommendations, and automated mixing strategies.
//!
//! ## Components
//!
//! - [`AutoMixer`] - Automated mixing controller
//! - [`TransitionAdvisor`] - Recommends optimal transition points
//! - [`TrackAnalyzer`] - Analyzes track characteristics
//! - [`MixStrategy`] - Configurable mixing strategies

#![warn(missing_docs)]

pub mod analyzer;
pub mod mixer;
pub mod strategy;
pub mod transition;

pub use analyzer::{
    AnalysisSource, MixPoints, SectionMarker, SectionType, TrackAnalysis, TrackAnalyzer,
};
pub use mixer::AutoMixer;
pub use strategy::MixStrategy;
pub use transition::TransitionAdvisor;
