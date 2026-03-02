//! Gemini API client with rate limiting and retry logic.
//!
//! This module provides the main `GeminiClient` for interacting with
//! Google's Gemini API. It includes:
//!
//! - Automatic rate limiting to stay within API quotas
//! - Exponential backoff retry for transient failures
//! - Cost tracking for budget management
//! - Support for text and multimodal requests
//!
//! # Example
//!
//! ```rust,ignore
//! use cc_gemini::{GeminiClient, GeminiConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = GeminiClient::from_env()?;
//!
//!     // Text generation
//!     let response = client.generate_text("What is the capital of France?").await?;
//!     println!("Response: {}", response);
//!
//!     // Image analysis
//!     let image_bytes = std::fs::read("image.jpg")?;
//!     let result = client.analyze_frame(&image_bytes, "image/jpeg", "Describe this image").await?;
//!     println!("Description: {}", result.description);
//!
//!     // Check costs
//!     println!("Total cost: ${:.4}", client.total_cost());
//!
//!     Ok(())
//! }
//! ```

use crate::config::GeminiConfig;
use crate::cost::{Cost, CostTracker};
use crate::error::{GeminiError, Result};
use crate::rate_limiter::RateLimiter;
use crate::types::*;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, trace, warn};

/// Production-grade Gemini API client.
///
/// The client handles:
/// - Rate limiting (RPM and TPM)
/// - Automatic retries with exponential backoff
/// - Cost tracking and budget enforcement
/// - Request/response serialization
///
/// # Thread Safety
///
/// The client is designed for concurrent use. All internal state is
/// protected by appropriate synchronization primitives.
///
/// # Rate Limiting
///
/// The client enforces both requests-per-minute (RPM) and tokens-per-minute
/// (TPM) limits. When limits are reached, requests will wait until capacity
/// is available.
///
/// # Cost Tracking
///
/// Every request's token usage is tracked. You can query `total_cost()` at
/// any time to see accumulated costs. Set a cost limit in the config to
/// prevent runaway spending.
pub struct GeminiClient {
    /// Configuration.
    config: GeminiConfig,

    /// HTTP client.
    http_client: reqwest::Client,

    /// Rate limiter.
    rate_limiter: RateLimiter,

    /// Cost tracker.
    cost_tracker: Arc<Mutex<CostTracker>>,

    /// Total requests made.
    total_requests: AtomicU64,
}

impl GeminiClient {
    /// Create a new client with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration
    ///
    /// # Errors
    ///
    /// Returns `GeminiError::ConfigError` if the configuration is invalid.
    pub fn new(config: GeminiConfig) -> Result<Self> {
        config.validate()?;

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // Add custom headers
        for (key, value) in &config.custom_headers {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::try_from(key),
                HeaderValue::try_from(value),
            ) {
                headers.insert(name, val);
            }
        }

        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .default_headers(headers)
            .build()
            .map_err(|e| {
                GeminiError::config_error(format!("Failed to create HTTP client: {}", e))
            })?;

        let rate_limiter = RateLimiter::new(config.rate_limit.clone());

        let mut cost_tracker = CostTracker::new(config.model);
        if let Some(max_cost) = config.max_cost {
            cost_tracker.set_limit(max_cost);
        }

        info!(
            model = %config.model,
            max_cost = ?config.max_cost,
            timeout_secs = config.timeout.as_secs(),
            "Gemini client initialized"
        );

        Ok(Self {
            config,
            http_client,
            rate_limiter,
            cost_tracker: Arc::new(Mutex::new(cost_tracker)),
            total_requests: AtomicU64::new(0),
        })
    }

    /// Create a client from environment variables.
    ///
    /// # Environment Variables
    ///
    /// - `GEMINI_API_KEY` (required): API key
    /// - `GEMINI_MODEL`: Model to use (default: gemini-2.0-flash)
    /// - `GEMINI_MAX_COST`: Maximum cost in USD
    /// - `GEMINI_TIMEOUT_SECS`: Request timeout
    ///
    /// # Errors
    ///
    /// Returns `GeminiError::MissingEnvVar` if `GEMINI_API_KEY` is not set.
    pub fn from_env() -> Result<Self> {
        let config = GeminiConfig::from_env()?;
        Self::new(config)
    }

    /// Generate text from a prompt.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The text prompt
    ///
    /// # Returns
    ///
    /// The generated text response.
    ///
    /// # Errors
    ///
    /// - `GeminiError::RateLimitExceeded`: Rate limit hit
    /// - `GeminiError::CostLimitExceeded`: Budget exceeded
    /// - `GeminiError::ContentBlocked`: Content filtered
    /// - `GeminiError::ApiError`: API returned an error
    pub async fn generate_text(&self, prompt: &str) -> Result<String> {
        let request = GenerateContentRequest::text(prompt);
        let response = self.generate_content(request).await?;

        response
            .text()
            .map(|s| s.to_string())
            .ok_or_else(|| GeminiError::MalformedResponse {
                message: "No text in response".to_string(),
                raw_response: None,
            })
    }

    /// Analyze an image with a prompt.
    ///
    /// # Arguments
    ///
    /// * `image_data` - Raw image bytes
    /// * `mime_type` - MIME type (e.g., "image/jpeg", "image/png")
    /// * `prompt` - Analysis prompt
    ///
    /// # Returns
    ///
    /// Analysis result including description and detected content.
    pub async fn analyze_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
        prompt: &str,
    ) -> Result<AnalysisResult> {
        let request = GenerateContentRequest::image(image_data, mime_type, prompt);
        let response = self.generate_content(request).await?;

        let text = response.full_text();
        let usage = response.usage().cloned().unwrap_or_default();

        Ok(AnalysisResult {
            text: text.clone(),
            description: text.clone(),
            detected_text: Vec::new(), // Could be parsed from structured response
            detected_objects: Vec::new(),
            tokens_used: TokenUsage::from(&usage),
            estimated_cost: response.cost().calculate_usd(self.config.model),
            raw_response: serde_json::to_string(&response).unwrap_or_default(),
        })
    }

    /// Analyze a video frame (alias for analyze_image).
    ///
    /// This is a convenience method for video processing pipelines.
    pub async fn analyze_frame(
        &self,
        frame_data: &[u8],
        mime_type: &str,
        prompt: &str,
    ) -> Result<AnalysisResult> {
        self.analyze_image(frame_data, mime_type, prompt).await
    }

    /// Send a raw generate content request.
    ///
    /// This is the low-level method that handles rate limiting, retries,
    /// and cost tracking.
    ///
    /// # Arguments
    ///
    /// * `request` - The request to send
    ///
    /// # Returns
    ///
    /// The parsed API response.
    pub async fn generate_content(
        &self,
        request: GenerateContentRequest,
    ) -> Result<GenerateContentResponse> {
        // Check cost limit before making request
        self.check_cost_limit(&request).await?;

        // Estimate tokens for rate limiting
        let estimated_tokens = self.estimate_request_tokens(&request);

        // Acquire rate limit
        let wait_time = self.rate_limiter.acquire(estimated_tokens).await;
        if wait_time > Duration::from_secs(1) {
            debug!(
                wait_secs = wait_time.as_secs_f64(),
                "Rate limit wait completed"
            );
        }

        // Execute with retries
        let response = self.execute_with_retry(&request).await?;

        // Track cost
        self.track_cost(&response).await;

        // Update request counter
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        Ok(response)
    }

    /// Execute a request with retry logic.
    async fn execute_with_retry(
        &self,
        request: &GenerateContentRequest,
    ) -> Result<GenerateContentResponse> {
        let mut last_error = None;

        for attempt in 0..=self.config.retry.max_retries {
            if attempt > 0 {
                let delay = self.config.retry.delay_for_attempt(attempt - 1);
                debug!(attempt, delay_ms = delay.as_millis(), "Retrying request");
                tokio::time::sleep(delay).await;
            }

            match self.execute_request(request).await {
                Ok(response) => {
                    // Check for blocked content
                    if response.is_blocked() {
                        return Err(GeminiError::ContentBlocked {
                            reason: response
                                .block_reason()
                                .unwrap_or("Unknown reason")
                                .to_string(),
                            category: None,
                        });
                    }
                    return Ok(response);
                }
                Err(e) => {
                    if e.is_retryable() && attempt < self.config.retry.max_retries {
                        warn!(
                            attempt,
                            error = %e,
                            "Request failed, will retry"
                        );
                        last_error = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| GeminiError::internal("Retry loop exited without result")))
    }

    /// Execute a single request without retry.
    async fn execute_request(
        &self,
        request: &GenerateContentRequest,
    ) -> Result<GenerateContentResponse> {
        let url = format!(
            "{}?key={}",
            self.config.generate_content_endpoint(),
            self.config.api_key
        );

        trace!(url = %self.config.generate_content_endpoint(), "Sending request");

        let response = self.http_client.post(&url).json(request).send().await?;

        let status = response.status();
        let body = response.text().await?;

        trace!(status = %status, body_len = body.len(), "Received response");

        if status.is_success() {
            serde_json::from_str(&body).map_err(|e| GeminiError::MalformedResponse {
                message: format!("Failed to parse response: {}", e),
                raw_response: Some(body),
            })
        } else {
            // Try to parse error response
            if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(&body) {
                let details = &error_response.error;

                if details.is_rate_limit() {
                    return Err(GeminiError::RateLimitExceeded { retry_after: None });
                }

                if details.is_auth_error() {
                    return Err(GeminiError::AuthenticationError {
                        message: details.message.clone(),
                    });
                }

                return Err(GeminiError::ApiError {
                    status: status.as_u16(),
                    message: details.message.clone(),
                    raw_response: Some(body),
                });
            }

            Err(GeminiError::ApiError {
                status: status.as_u16(),
                message: format!("HTTP {}", status),
                raw_response: Some(body),
            })
        }
    }

    /// Check if making a request would exceed the cost limit.
    async fn check_cost_limit(&self, request: &GenerateContentRequest) -> Result<()> {
        let tracker = self.cost_tracker.lock().await;

        if let Some(limit) = tracker.limit() {
            let current = tracker.total_usd();
            if current >= limit {
                return Err(GeminiError::CostLimitExceeded { current, limit });
            }

            // Estimate cost of this request
            let estimated_tokens = self.estimate_request_tokens(request);
            let estimated_cost = Cost::new(estimated_tokens as u64, 100, 0);

            if tracker.would_exceed_limit(&estimated_cost) {
                warn!(
                    current = current,
                    limit = limit,
                    "Request may exceed cost limit"
                );
            }
        }

        Ok(())
    }

    /// Track the cost of a completed request.
    async fn track_cost(&self, response: &GenerateContentResponse) {
        if self.config.track_costs {
            let cost = response.cost();
            let tracker = self.cost_tracker.lock().await;
            tracker.add(&cost);

            debug!(
                input_tokens = cost.input_tokens,
                output_tokens = cost.output_tokens,
                total_cost = tracker.total_usd(),
                "Request cost tracked"
            );
        }
    }

    /// Estimate tokens for a request (for rate limiting).
    fn estimate_request_tokens(&self, request: &GenerateContentRequest) -> u32 {
        let mut tokens = 0u32;

        for content in &request.contents {
            for part in &content.parts {
                match part {
                    Part::Text { text } => {
                        // ~4 characters per token
                        tokens += (text.len() / 4 + 1) as u32;
                    }
                    Part::InlineData { inline_data } => {
                        tokens += inline_data.estimated_tokens() as u32;
                    }
                    Part::FileData { .. } => {
                        tokens += 1000; // Conservative estimate for file refs
                    }
                }
            }
        }

        // Add estimate for output
        tokens += 500;

        tokens
    }

    /// Get total requests made.
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Get total cost in USD.
    pub fn total_cost(&self) -> f64 {
        // Use try_lock to avoid blocking
        if let Ok(tracker) = self.cost_tracker.try_lock() {
            tracker.total_usd()
        } else {
            0.0
        }
    }

    /// Get the cost tracker for detailed metrics.
    pub async fn cost_tracker(&self) -> tokio::sync::MutexGuard<'_, CostTracker> {
        self.cost_tracker.lock().await
    }

    /// Get the rate limiter for monitoring.
    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }

    /// Get the configuration.
    pub fn config(&self) -> &GeminiConfig {
        &self.config
    }

    /// Reset cost tracking (start a new session).
    pub async fn reset_costs(&self) {
        let tracker = self.cost_tracker.lock().await;
        tracker.reset();
        info!("Cost tracking reset");
    }

    /// Check if the client is within rate limits.
    pub async fn is_rate_limited(&self) -> bool {
        let wait = self.rate_limiter.time_until_available(1000).await;
        wait > Duration::ZERO
    }
}

impl std::fmt::Debug for GeminiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeminiClient")
            .field("model", &self.config.model)
            .field("total_requests", &self.total_requests())
            .field("total_cost", &self.total_cost())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> GeminiConfig {
        GeminiConfig::new("test-api-key-that-is-long-enough")
            .with_max_cost(10.0)
            .with_timeout(Duration::from_secs(30))
    }

    #[test]
    fn test_client_creation() {
        let config = test_config();
        let client = GeminiClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_token_estimation() {
        let config = test_config();
        let client = GeminiClient::new(config).unwrap();

        // Text request
        let request = GenerateContentRequest::text("Hello, world!");
        let tokens = client.estimate_request_tokens(&request);
        assert!(tokens > 0);
        assert!(tokens < 1000);

        // Image request
        let image_data = vec![0u8; 10000]; // 10KB image
        let request = GenerateContentRequest::image(&image_data, "image/jpeg", "Describe");
        let tokens = client.estimate_request_tokens(&request);
        assert!(tokens > 500); // Should include image tokens
    }

    #[test]
    fn test_metrics() {
        let config = test_config();
        let client = GeminiClient::new(config).unwrap();

        assert_eq!(client.total_requests(), 0);
        assert_eq!(client.total_cost(), 0.0);
    }

    #[tokio::test]
    async fn test_cost_limit_check() {
        let config = GeminiConfig::new("test-api-key-that-is-long-enough").with_max_cost(0.0001); // Very low limit

        let client = GeminiClient::new(config).unwrap();

        // Add some cost manually
        {
            let tracker = client.cost_tracker.lock().await;
            tracker.add(&Cost::new(10000, 5000, 0));
        }

        // Should fail due to cost limit
        let request = GenerateContentRequest::text("Test");
        let result = client.check_cost_limit(&request).await;

        assert!(matches!(result, Err(GeminiError::CostLimitExceeded { .. })));
    }
}
