//! Cost tracking and estimation for Gemini API usage.
//!
//! This module provides precise cost tracking for Gemini API requests,
//! enabling budget management and cost optimization. It supports all
//! Gemini models with their respective pricing tiers.
//!
//! # Pricing Model
//!
//! Gemini pricing is based on three token categories:
//! - **Input tokens**: Text and structured data sent to the model
//! - **Output tokens**: Generated text responses
//! - **Image tokens**: Visual content (images, video frames)
//!
//! Each model has different per-million-token rates for these categories.
//!
//! # Example
//!
//! ```rust,ignore
//! use cc_gemini::{Cost, CostTracker, GeminiModel};
//!
//! let mut tracker = CostTracker::new(GeminiModel::Flash2_0);
//!
//! // After each API call, record the cost
//! let cost = Cost::new(150, 50, 1000); // 150 input, 50 output, 1000 image
//! tracker.add(&cost);
//!
//! println!("Total cost: ${:.4}", tracker.total_usd());
//! println!("Average per request: ${:.6}", tracker.average_cost());
//! ```

use crate::config::GeminiModel;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, info};

/// Token counts for a single API request.
///
/// This struct captures the three categories of tokens that Gemini
/// charges for, enabling precise cost calculation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Cost {
    /// Number of input tokens (prompt text).
    pub input_tokens: u64,

    /// Number of output tokens (generated response).
    pub output_tokens: u64,

    /// Number of image tokens (visual content).
    ///
    /// Image tokens are calculated based on image dimensions.
    /// Typical values:
    /// - Small image (~256x256): ~250 tokens
    /// - Medium image (~512x512): ~500 tokens
    /// - Large image (~1024x1024): ~1000 tokens
    pub image_tokens: u64,
}

impl Cost {
    /// Create a new cost record.
    ///
    /// # Arguments
    ///
    /// * `input_tokens` - Number of input/prompt tokens
    /// * `output_tokens` - Number of output/response tokens
    /// * `image_tokens` - Number of image tokens
    pub fn new(input_tokens: u64, output_tokens: u64, image_tokens: u64) -> Self {
        Self {
            input_tokens,
            output_tokens,
            image_tokens,
        }
    }

    /// Create a cost record for text-only request (no images).
    pub fn text_only(input_tokens: u64, output_tokens: u64) -> Self {
        Self::new(input_tokens, output_tokens, 0)
    }

    /// Create a cost record for image analysis.
    pub fn image_analysis(image_tokens: u64, prompt_tokens: u64, output_tokens: u64) -> Self {
        Self::new(prompt_tokens, output_tokens, image_tokens)
    }

    /// Total tokens across all categories.
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.image_tokens
    }

    /// Calculate cost in USD for a given model.
    ///
    /// # Arguments
    ///
    /// * `model` - The Gemini model to use for pricing
    ///
    /// # Returns
    ///
    /// Cost in USD as a floating-point value.
    pub fn calculate_usd(&self, model: GeminiModel) -> f64 {
        let input_cost = self.input_tokens as f64 * model.input_cost_per_million() / 1_000_000.0;
        let output_cost = self.output_tokens as f64 * model.output_cost_per_million() / 1_000_000.0;
        // Image tokens are charged at input rate
        let image_cost = self.image_tokens as f64 * model.input_cost_per_million() / 1_000_000.0;

        input_cost + output_cost + image_cost
    }

    /// Add another cost to this one.
    pub fn add(&mut self, other: &Cost) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.image_tokens += other.image_tokens;
    }

    /// Check if this cost is zero (no tokens used).
    pub fn is_zero(&self) -> bool {
        self.input_tokens == 0 && self.output_tokens == 0 && self.image_tokens == 0
    }
}

impl std::ops::Add for Cost {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            input_tokens: self.input_tokens + rhs.input_tokens,
            output_tokens: self.output_tokens + rhs.output_tokens,
            image_tokens: self.image_tokens + rhs.image_tokens,
        }
    }
}

impl std::ops::AddAssign for Cost {
    fn add_assign(&mut self, rhs: Self) {
        self.add(&rhs);
    }
}

impl std::fmt::Display for Cost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Cost(input={}, output={}, image={}, total={})",
            self.input_tokens,
            self.output_tokens,
            self.image_tokens,
            self.total_tokens()
        )
    }
}

/// Thread-safe cost tracker for cumulative API usage.
///
/// Tracks total token usage across multiple requests and provides
/// cost calculations, statistics, and budget enforcement.
///
/// # Thread Safety
///
/// All operations use atomic counters, making this safe to share
/// across async tasks without explicit locking.
///
/// # Budget Enforcement
///
/// When a cost limit is configured, the tracker can be queried to
/// check if the limit has been exceeded before making requests.
#[derive(Debug)]
pub struct CostTracker {
    /// Model for cost calculations.
    model: GeminiModel,

    /// Total input tokens.
    input_tokens: AtomicU64,

    /// Total output tokens.
    output_tokens: AtomicU64,

    /// Total image tokens.
    image_tokens: AtomicU64,

    /// Number of requests tracked.
    request_count: AtomicU64,

    /// Optional cost limit in USD (stored as micro-dollars for precision).
    cost_limit_micros: Option<u64>,
}

impl CostTracker {
    /// Create a new cost tracker for the specified model.
    ///
    /// # Arguments
    ///
    /// * `model` - The Gemini model to use for cost calculations
    pub fn new(model: GeminiModel) -> Self {
        Self {
            model,
            input_tokens: AtomicU64::new(0),
            output_tokens: AtomicU64::new(0),
            image_tokens: AtomicU64::new(0),
            request_count: AtomicU64::new(0),
            cost_limit_micros: None,
        }
    }

    /// Create a cost tracker with a cost limit.
    ///
    /// # Arguments
    ///
    /// * `model` - The Gemini model to use for cost calculations
    /// * `limit_usd` - Maximum allowed cost in USD
    pub fn with_limit(model: GeminiModel, limit_usd: f64) -> Self {
        let mut tracker = Self::new(model);
        tracker.set_limit(limit_usd);
        tracker
    }

    /// Set the cost limit.
    ///
    /// # Arguments
    ///
    /// * `limit_usd` - Maximum allowed cost in USD
    pub fn set_limit(&mut self, limit_usd: f64) {
        // Store as micro-dollars for precision
        self.cost_limit_micros = Some((limit_usd * 1_000_000.0) as u64);
    }

    /// Remove the cost limit.
    pub fn clear_limit(&mut self) {
        self.cost_limit_micros = None;
    }

    /// Get the cost limit in USD, if set.
    pub fn limit(&self) -> Option<f64> {
        self.cost_limit_micros
            .map(|micros| micros as f64 / 1_000_000.0)
    }

    /// Add a cost to the tracker.
    ///
    /// # Arguments
    ///
    /// * `cost` - The cost to add
    pub fn add(&self, cost: &Cost) {
        self.input_tokens
            .fetch_add(cost.input_tokens, Ordering::Relaxed);
        self.output_tokens
            .fetch_add(cost.output_tokens, Ordering::Relaxed);
        self.image_tokens
            .fetch_add(cost.image_tokens, Ordering::Relaxed);
        self.request_count.fetch_add(1, Ordering::Relaxed);

        debug!(
            input = cost.input_tokens,
            output = cost.output_tokens,
            image = cost.image_tokens,
            total_usd = self.total_usd(),
            "Cost recorded"
        );
    }

    /// Get total accumulated cost.
    pub fn total(&self) -> Cost {
        Cost {
            input_tokens: self.input_tokens.load(Ordering::Relaxed),
            output_tokens: self.output_tokens.load(Ordering::Relaxed),
            image_tokens: self.image_tokens.load(Ordering::Relaxed),
        }
    }

    /// Get total cost in USD.
    pub fn total_usd(&self) -> f64 {
        self.total().calculate_usd(self.model)
    }

    /// Get total number of requests tracked.
    pub fn request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }

    /// Get average cost per request in USD.
    ///
    /// Returns 0.0 if no requests have been tracked.
    pub fn average_cost(&self) -> f64 {
        let count = self.request_count();
        if count == 0 {
            0.0
        } else {
            self.total_usd() / count as f64
        }
    }

    /// Get average tokens per request.
    pub fn average_tokens(&self) -> f64 {
        let count = self.request_count();
        if count == 0 {
            0.0
        } else {
            self.total().total_tokens() as f64 / count as f64
        }
    }

    /// Check if the cost limit has been exceeded.
    ///
    /// Returns `false` if no limit is set.
    pub fn is_limit_exceeded(&self) -> bool {
        match self.cost_limit_micros {
            Some(limit_micros) => {
                let current_micros = (self.total_usd() * 1_000_000.0) as u64;
                current_micros >= limit_micros
            }
            None => false,
        }
    }

    /// Check if adding an estimated cost would exceed the limit.
    ///
    /// Useful for pre-flight checks before making API calls.
    ///
    /// # Arguments
    ///
    /// * `estimated_cost` - Estimated cost of the next request
    ///
    /// # Returns
    ///
    /// `true` if the total would exceed the limit, `false` otherwise.
    /// Returns `false` if no limit is set.
    pub fn would_exceed_limit(&self, estimated_cost: &Cost) -> bool {
        match self.cost_limit_micros {
            Some(limit_micros) => {
                let current = self.total_usd();
                let estimated = estimated_cost.calculate_usd(self.model);
                let total_micros = ((current + estimated) * 1_000_000.0) as u64;
                total_micros > limit_micros
            }
            None => false,
        }
    }

    /// Get remaining budget in USD.
    ///
    /// Returns `None` if no limit is set.
    /// Returns 0.0 if limit is exceeded.
    pub fn remaining_budget(&self) -> Option<f64> {
        self.cost_limit_micros.map(|limit_micros| {
            let current_micros = (self.total_usd() * 1_000_000.0) as u64;
            if current_micros >= limit_micros {
                0.0
            } else {
                (limit_micros - current_micros) as f64 / 1_000_000.0
            }
        })
    }

    /// Get the model used for cost calculations.
    pub fn model(&self) -> GeminiModel {
        self.model
    }

    /// Reset all counters to zero.
    ///
    /// Useful for starting a new session while keeping the same tracker.
    /// Does not reset the cost limit.
    pub fn reset(&self) {
        self.input_tokens.store(0, Ordering::Relaxed);
        self.output_tokens.store(0, Ordering::Relaxed);
        self.image_tokens.store(0, Ordering::Relaxed);
        self.request_count.store(0, Ordering::Relaxed);

        info!("Cost tracker reset");
    }

    /// Get a detailed cost breakdown.
    pub fn breakdown(&self) -> CostBreakdown {
        let cost = self.total();
        CostBreakdown {
            input_tokens: cost.input_tokens,
            output_tokens: cost.output_tokens,
            image_tokens: cost.image_tokens,
            total_tokens: cost.total_tokens(),
            input_cost_usd: cost.input_tokens as f64 * self.model.input_cost_per_million()
                / 1_000_000.0,
            output_cost_usd: cost.output_tokens as f64 * self.model.output_cost_per_million()
                / 1_000_000.0,
            image_cost_usd: cost.image_tokens as f64 * self.model.input_cost_per_million()
                / 1_000_000.0,
            total_cost_usd: self.total_usd(),
            request_count: self.request_count(),
            model: self.model,
        }
    }
}

impl Clone for CostTracker {
    fn clone(&self) -> Self {
        Self {
            model: self.model,
            input_tokens: AtomicU64::new(self.input_tokens.load(Ordering::Relaxed)),
            output_tokens: AtomicU64::new(self.output_tokens.load(Ordering::Relaxed)),
            image_tokens: AtomicU64::new(self.image_tokens.load(Ordering::Relaxed)),
            request_count: AtomicU64::new(self.request_count.load(Ordering::Relaxed)),
            cost_limit_micros: self.cost_limit_micros,
        }
    }
}

/// Detailed cost breakdown for reporting.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CostBreakdown {
    /// Total input tokens.
    pub input_tokens: u64,

    /// Total output tokens.
    pub output_tokens: u64,

    /// Total image tokens.
    pub image_tokens: u64,

    /// Total tokens across all categories.
    pub total_tokens: u64,

    /// Cost of input tokens in USD.
    pub input_cost_usd: f64,

    /// Cost of output tokens in USD.
    pub output_cost_usd: f64,

    /// Cost of image tokens in USD.
    pub image_cost_usd: f64,

    /// Total cost in USD.
    pub total_cost_usd: f64,

    /// Number of requests.
    pub request_count: u64,

    /// Model used for pricing.
    pub model: GeminiModel,
}

impl std::fmt::Display for CostBreakdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Cost Breakdown ({}):", self.model)?;
        writeln!(
            f,
            "  Input:  {:>10} tokens = ${:.6}",
            self.input_tokens, self.input_cost_usd
        )?;
        writeln!(
            f,
            "  Output: {:>10} tokens = ${:.6}",
            self.output_tokens, self.output_cost_usd
        )?;
        writeln!(
            f,
            "  Image:  {:>10} tokens = ${:.6}",
            self.image_tokens, self.image_cost_usd
        )?;
        writeln!(f, "  ─────────────────────────────")?;
        writeln!(
            f,
            "  Total:  {:>10} tokens = ${:.4}",
            self.total_tokens, self.total_cost_usd
        )?;
        write!(f, "  Requests: {}", self.request_count)
    }
}

/// Estimate tokens for common content types.
///
/// These are approximate values useful for pre-flight cost estimation.
pub mod estimation {
    /// Estimate tokens for a text prompt.
    ///
    /// Rule of thumb: ~4 characters per token for English text.
    pub fn estimate_text_tokens(text: &str) -> u64 {
        // Roughly 4 characters per token
        (text.len() as f64 / 4.0).ceil() as u64
    }

    /// Estimate tokens for an image based on dimensions.
    ///
    /// Gemini charges based on image resolution.
    pub fn estimate_image_tokens(width: u32, height: u32) -> u64 {
        // Approximate: ~1 token per 768 pixels
        let pixels = width as u64 * height as u64;
        (pixels / 768).max(256) // Minimum 256 tokens for any image
    }

    /// Estimate tokens for a JPEG image from byte size.
    ///
    /// Useful when dimensions aren't known.
    pub fn estimate_image_tokens_from_size(byte_size: usize) -> u64 {
        // Very rough estimate: ~1 token per 100 bytes
        // Minimum 256 tokens for any image
        ((byte_size / 100) as u64).max(256)
    }

    /// Estimate expected output tokens for common tasks.
    pub fn estimate_output_tokens(task: OutputTask) -> u64 {
        match task {
            OutputTask::Classification => 50,
            OutputTask::ShortDescription => 100,
            OutputTask::DetailedDescription => 300,
            OutputTask::OcrTranscription => 500,
            OutputTask::Analysis => 800,
            OutputTask::LongForm => 2000,
        }
    }

    /// Common output task types for estimation.
    #[derive(Debug, Clone, Copy)]
    pub enum OutputTask {
        /// Simple yes/no or category output.
        Classification,
        /// Brief 1-2 sentence description.
        ShortDescription,
        /// Detailed multi-paragraph description.
        DetailedDescription,
        /// Text extraction from images.
        OcrTranscription,
        /// Detailed analysis with multiple aspects.
        Analysis,
        /// Long-form content generation.
        LongForm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_calculation() {
        let cost = Cost::new(1_000_000, 500_000, 1_000_000);

        // Flash 2.0: $0.075/M input, $0.30/M output
        let flash_cost = cost.calculate_usd(GeminiModel::Flash2_0);
        // Input: 1M * 0.075 = 0.075
        // Output: 0.5M * 0.30 = 0.15
        // Image: 1M * 0.075 = 0.075
        // Total: 0.30
        assert!((flash_cost - 0.30).abs() < 0.001);
    }

    #[test]
    fn test_cost_addition() {
        let cost1 = Cost::new(100, 50, 200);
        let cost2 = Cost::new(150, 75, 300);

        let combined = cost1 + cost2;
        assert_eq!(combined.input_tokens, 250);
        assert_eq!(combined.output_tokens, 125);
        assert_eq!(combined.image_tokens, 500);
    }

    #[test]
    fn test_cost_tracker_basic() {
        let tracker = CostTracker::new(GeminiModel::Flash2_0);

        tracker.add(&Cost::new(100, 50, 200));
        tracker.add(&Cost::new(150, 75, 300));

        assert_eq!(tracker.request_count(), 2);
        assert_eq!(tracker.total().input_tokens, 250);
        assert_eq!(tracker.total().output_tokens, 125);
        assert_eq!(tracker.total().image_tokens, 500);
    }

    #[test]
    fn test_cost_limit() {
        let mut tracker = CostTracker::new(GeminiModel::Flash2_0);
        tracker.set_limit(0.001); // $0.001 limit

        // Small cost should be fine
        let small_cost = Cost::new(100, 50, 100);
        assert!(!tracker.would_exceed_limit(&small_cost));

        tracker.add(&small_cost);
        assert!(!tracker.is_limit_exceeded());

        // Large cost should exceed
        let large_cost = Cost::new(1_000_000, 500_000, 1_000_000);
        assert!(tracker.would_exceed_limit(&large_cost));
    }

    #[test]
    fn test_remaining_budget() {
        let mut tracker = CostTracker::new(GeminiModel::Flash2_0);
        tracker.set_limit(1.0); // $1.00 limit

        assert!((tracker.remaining_budget().unwrap() - 1.0).abs() < 0.0001);

        // Add some cost
        tracker.add(&Cost::new(1_000_000, 0, 0)); // ~$0.075
        assert!(tracker.remaining_budget().unwrap() < 1.0);
        assert!(tracker.remaining_budget().unwrap() > 0.9);
    }

    #[test]
    fn test_reset() {
        let tracker = CostTracker::new(GeminiModel::Flash2_0);
        tracker.add(&Cost::new(100, 50, 200));

        assert!(tracker.request_count() > 0);

        tracker.reset();

        assert_eq!(tracker.request_count(), 0);
        assert_eq!(tracker.total().total_tokens(), 0);
    }

    #[test]
    fn test_estimation() {
        // Text estimation
        let text = "Hello, world!"; // 13 characters
        let tokens = estimation::estimate_text_tokens(text);
        assert!((3..=5).contains(&tokens));

        // Image estimation
        let tokens = estimation::estimate_image_tokens(1024, 1024);
        assert!(tokens > 1000);

        // Output estimation
        let tokens = estimation::estimate_output_tokens(estimation::OutputTask::Classification);
        assert_eq!(tokens, 50);
    }
}
