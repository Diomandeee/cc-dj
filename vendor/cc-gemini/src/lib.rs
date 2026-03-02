//! # cc-gemini
//!
//! Production-grade Gemini API client with rate limiting, cost tracking, and retry logic.
//!
//! This crate provides a robust, thread-safe interface to Google's Gemini API,
//! designed for high-throughput multimodal analysis workloads.
//!
//! ## Features
//!
//! - **Rate Limiting**: Dual token bucket algorithm respecting both RPM and TPM limits
//! - **Cost Tracking**: Real-time cost estimation with budget enforcement
//! - **Retry Logic**: Exponential backoff with jitter for transient failures
//! - **Multimodal**: Support for text, images, and video frame analysis
//! - **Thread-Safe**: Designed for concurrent usage across async tasks
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use cc_gemini::{GeminiClient, GeminiConfig};
//!
//! // Create client from environment
//! let client = GeminiClient::from_env()?;
//!
//! // Simple text generation
//! let response = client.generate_text("Explain quantum computing").await?;
//! println!("{}", response);
//!
//! // Image analysis
//! let image_data = std::fs::read("image.jpg")?;
//! let result = client.analyze_image(&image_data, "image/jpeg", "Describe this image").await?;
//! println!("{}", result.description);
//!
//! // Check costs
//! println!("Total cost: ${:.4}", client.total_cost());
//! ```
//!
//! ## Configuration
//!
//! The client can be configured via environment variables or programmatically:
//!
//! ```bash
//! export GEMINI_API_KEY=your_api_key
//! export GEMINI_MODEL=gemini-2.0-flash          # Optional
//! export GEMINI_TIMEOUT_SECS=30                 # Optional
//! export GEMINI_RPM_LIMIT=4000                  # Optional
//! export GEMINI_TPM_LIMIT=4000000               # Optional
//! export GEMINI_MAX_COST=10.0                   # Optional cost limit
//! ```
//!
//! Or programmatically:
//!
//! ```rust,ignore
//! use cc_gemini::{GeminiConfig, GeminiModel};
//!
//! let config = GeminiConfig::new("your_api_key")
//!     .with_model(GeminiModel::Flash2_0)
//!     .with_timeout(30)
//!     .with_max_cost(10.0);
//!
//! let client = GeminiClient::new(config)?;
//! ```
//!
//! ## Rate Limiting
//!
//! The client automatically respects Gemini API rate limits:
//!
//! - **RPM (Requests Per Minute)**: Default 4000 for Flash models
//! - **TPM (Tokens Per Minute)**: Default 4,000,000 for Flash models
//!
//! When limits are approached, the client will automatically wait before proceeding.
//!
//! ```rust,ignore
//! // Check if a request can proceed immediately
//! let wait_time = client.time_until_available(1000).await;
//! if wait_time.is_zero() {
//!     println!("Request can proceed immediately");
//! } else {
//!     println!("Must wait {:?}", wait_time);
//! }
//! ```
//!
//! ## Cost Tracking
//!
//! Track and limit API costs in real-time:
//!
//! ```rust,ignore
//! let config = GeminiConfig::new("key")
//!     .with_max_cost(5.0)  // $5.00 limit
//!     .with_cost_tracking(true);
//!
//! let client = GeminiClient::new(config)?;
//!
//! // This will fail if cost limit exceeded
//! match client.generate_text("prompt").await {
//!     Err(GeminiError::CostLimitExceeded { current, limit }) => {
//!         println!("Cost ${:.2} exceeded limit ${:.2}", current, limit);
//!     }
//!     Ok(response) => println!("{}", response),
//!     Err(e) => eprintln!("Other error: {}", e),
//! }
//! ```
//!
//! ## Error Handling
//!
//! The crate provides comprehensive error types for precise handling:
//!
//! ```rust,ignore
//! use cc_gemini::GeminiError;
//!
//! match client.generate_text("prompt").await {
//!     Ok(text) => println!("{}", text),
//!     Err(GeminiError::RateLimitExceeded { retry_after }) => {
//!         // Wait and retry
//!         if let Some(duration) = retry_after {
//!             tokio::time::sleep(duration).await;
//!         }
//!     }
//!     Err(GeminiError::ContentBlocked { reason }) => {
//!         // Handle safety filter
//!         eprintln!("Content blocked: {}", reason);
//!     }
//!     Err(e) if e.is_retryable() => {
//!         // Transient error, retry logic
//!         eprintln!("Retryable error: {}", e);
//!     }
//!     Err(e) => {
//!         // Non-retryable error
//!         eprintln!("Fatal error: {}", e);
//!     }
//! }
//! ```
//!
//! ## Models
//!
//! Supported Gemini models with their pricing:
//!
//! | Model | Input ($/M tokens) | Output ($/M tokens) | Best For |
//! |-------|-------------------|---------------------|----------|
//! | `Flash2_0` | $0.075 | $0.30 | High-throughput, balanced |
//! | `Flash2_0Lite` | $0.0375 | $0.15 | Cost-sensitive workloads |
//! | `Flash1_5` | $0.075 | $0.30 | Stable, proven performance |
//! | `Pro1_5` | $1.25 | $5.00 | Maximum capability |
//!
//! ## Thread Safety
//!
//! The `GeminiClient` is designed for concurrent usage:
//!
//! ```rust,ignore
//! use std::sync::Arc;
//!
//! let client = Arc::new(GeminiClient::from_env()?);
//!
//! let handles: Vec<_> = (0..10).map(|i| {
//!     let client = client.clone();
//!     tokio::spawn(async move {
//!         client.generate_text(&format!("Prompt {}", i)).await
//!     })
//! }).collect();
//!
//! for handle in handles {
//!     let result = handle.await?;
//!     // Process results...
//! }
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![deny(unsafe_code)]

// Module declarations
mod client;
mod config;
mod cost;
mod error;
mod rate_limiter;

/// Batch API support for asynchronous bulk processing.
///
/// Provides methods for creating and managing batch jobs that process
/// large volumes of requests at 50% of the standard cost.
pub mod batch;

/// Request and response types for the Gemini API.
pub mod types;

/// Live API support for real-time audio/video streaming.
///
/// Provides WebSocket-based bidirectional communication with Gemini models
/// for low-latency voice and video interactions.
pub mod live;

// Re-exports for convenient access
pub use client::GeminiClient;
pub use config::{GeminiConfig, GeminiModel, RateLimitConfig, RetryConfig, CONFIG_SCHEMA_VERSION};
pub use cost::{Cost, CostBreakdown, CostTracker};
pub use error::GeminiError;
pub use rate_limiter::RateLimiter;

// Batch API re-exports
pub use batch::{
    build_jsonl, BatchClient, BatchConfig, BatchJob, BatchJobState, BatchRequest, BatchResponse,
    BatchResults, BatchStats,
};

// Type re-exports from types module for convenience
pub use types::request::{
    Content, GenerateContentRequest, GenerationConfig, HarmBlockThreshold, HarmCategory,
    InlineData, Part, Role, SafetySetting,
};
pub use types::response::{
    AnalysisResult, ApiErrorDetails, ApiErrorResponse, Candidate, GenerateContentResponse,
    PromptFeedback, ResponseContent, ResponsePart, SafetyRating, TokenUsage, UsageMetadata,
};

/// Result type alias for Gemini operations.
pub type Result<T> = std::result::Result<T, GeminiError>;

/// Prelude module for common imports.
///
/// # Example
///
/// ```rust,ignore
/// use cc_gemini::prelude::*;
///
/// let client = GeminiClient::from_env()?;
/// let response = client.generate_text("Hello").await?;
/// ```
pub mod prelude {
    pub use crate::batch::{BatchClient, BatchConfig, BatchJob, BatchJobState, BatchRequest};
    pub use crate::client::GeminiClient;
    pub use crate::config::{GeminiConfig, GeminiModel};
    pub use crate::cost::Cost;
    pub use crate::error::GeminiError;
    pub use crate::types::response::AnalysisResult;
    pub use crate::Result;
}

/// Cost estimation utilities.
///
/// Provides functions for estimating token counts and costs before making requests.
pub mod estimation {
    pub use crate::cost::estimation::*;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_reexports() {
        // Verify all major types are accessible
        let _: fn() -> GeminiConfig = || GeminiConfig::new("test");
        let _: fn() -> Cost = Cost::default;
        let _: GeminiModel = GeminiModel::Flash2_0;
    }

    #[test]
    fn test_prelude_exports() {
        use prelude::*;

        // Verify prelude provides common types
        let config = GeminiConfig::new("test");
        assert_eq!(config.model, GeminiModel::Flash2_0);
    }

    #[test]
    fn test_estimation_module() {
        use estimation::*;

        // Verify estimation utilities are accessible
        let tokens = estimate_text_tokens("Hello, world!");
        assert!(tokens > 0);
    }
}
