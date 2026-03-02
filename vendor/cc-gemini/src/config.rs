//! Configuration types for the Gemini API client.
//!
//! This module provides comprehensive configuration for the Gemini client,
//! including model selection, rate limiting parameters, retry policies,
//! and cost control settings.
//!
//! # Configuration Sources
//!
//! Configuration can be loaded from:
//! - Environment variables (recommended for production)
//! - Programmatic construction (useful for testing)
//! - Default values for development
//!
//! # Example
//!
//! ```rust,ignore
//! use cc_gemini::{GeminiConfig, GeminiModel};
//!
//! // Load from environment
//! let config = GeminiConfig::from_env()?;
//!
//! // Or construct programmatically
//! let config = GeminiConfig::new("your-api-key")
//!     .with_model(GeminiModel::Flash2_0)
//!     .with_max_cost(10.0)
//!     .with_timeout(Duration::from_secs(60));
//! ```

use crate::error::{GeminiError, Result};
use std::time::Duration;

/// Schema version for configuration compatibility tracking.
pub const CONFIG_SCHEMA_VERSION: u32 = 1;

/// Default Gemini API base URL.
pub const DEFAULT_API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Environment variable name for the API key.
pub const ENV_API_KEY: &str = "GEMINI_API_KEY";

/// Environment variable name for the model.
pub const ENV_MODEL: &str = "GEMINI_MODEL";

/// Environment variable name for max cost.
pub const ENV_MAX_COST: &str = "GEMINI_MAX_COST";

/// Available Gemini models with their capabilities and pricing.
///
/// Each model variant includes metadata about its capabilities,
/// context window, and pricing tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum GeminiModel {
    /// Gemini 2.0 Flash - Fast multimodal model optimized for speed.
    ///
    /// - Context: 1M tokens
    /// - Pricing: $0.075/M input, $0.30/M output
    /// - Best for: Real-time applications, high-volume analysis
    #[serde(rename = "gemini-2.0-flash")]
    #[default]
    Flash2_0,

    /// Gemini 2.0 Flash Lite - Lightweight version for simple tasks.
    ///
    /// - Context: 1M tokens
    /// - Pricing: $0.02/M input, $0.05/M output
    /// - Best for: Simple classification, basic OCR
    #[serde(rename = "gemini-2.0-flash-lite")]
    Flash2_0Lite,

    /// Gemini 1.5 Flash - Previous generation fast model.
    ///
    /// - Context: 1M tokens
    /// - Pricing: $0.075/M input, $0.30/M output
    /// - Best for: Legacy compatibility
    #[serde(rename = "gemini-1.5-flash")]
    Flash1_5,

    /// Gemini 1.5 Pro - Higher quality, larger context.
    ///
    /// - Context: 2M tokens
    /// - Pricing: $1.25/M input, $5.00/M output
    /// - Best for: Complex reasoning, long-form content
    #[serde(rename = "gemini-1.5-pro")]
    Pro1_5,
}

impl GeminiModel {
    /// Returns the API model identifier string.
    pub fn as_str(&self) -> &'static str {
        match self {
            GeminiModel::Flash2_0 => "gemini-2.0-flash",
            GeminiModel::Flash2_0Lite => "gemini-2.0-flash-lite",
            GeminiModel::Flash1_5 => "gemini-1.5-flash",
            GeminiModel::Pro1_5 => "gemini-1.5-pro",
        }
    }

    /// Returns the full API path for content generation.
    pub fn generate_content_path(&self) -> String {
        format!("models/{}:generateContent", self.as_str())
    }

    /// Returns the input token cost per million tokens in USD.
    pub fn input_cost_per_million(&self) -> f64 {
        match self {
            GeminiModel::Flash2_0 => 0.075,
            GeminiModel::Flash2_0Lite => 0.02,
            GeminiModel::Flash1_5 => 0.075,
            GeminiModel::Pro1_5 => 1.25,
        }
    }

    /// Returns the output token cost per million tokens in USD.
    pub fn output_cost_per_million(&self) -> f64 {
        match self {
            GeminiModel::Flash2_0 => 0.30,
            GeminiModel::Flash2_0Lite => 0.05,
            GeminiModel::Flash1_5 => 0.30,
            GeminiModel::Pro1_5 => 5.00,
        }
    }

    /// Returns the maximum context window in tokens.
    pub fn context_window(&self) -> u64 {
        match self {
            GeminiModel::Flash2_0 => 1_000_000,
            GeminiModel::Flash2_0Lite => 1_000_000,
            GeminiModel::Flash1_5 => 1_000_000,
            GeminiModel::Pro1_5 => 2_000_000,
        }
    }

    /// Returns whether this model supports image input.
    pub fn supports_images(&self) -> bool {
        // All current Gemini models support images
        true
    }

    /// Returns whether this model supports audio input.
    pub fn supports_audio(&self) -> bool {
        matches!(self, GeminiModel::Flash2_0 | GeminiModel::Pro1_5)
    }

    /// Returns whether this model supports video input.
    pub fn supports_video(&self) -> bool {
        matches!(self, GeminiModel::Flash2_0 | GeminiModel::Pro1_5)
    }

    /// Parse a model from string, with flexible matching.
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "gemini-2.0-flash" | "flash-2.0" | "flash2" => Some(GeminiModel::Flash2_0),
            "gemini-2.0-flash-lite" | "flash-2.0-lite" | "flash2-lite" => {
                Some(GeminiModel::Flash2_0Lite)
            }
            "gemini-1.5-flash" | "flash-1.5" | "flash" => Some(GeminiModel::Flash1_5),
            "gemini-1.5-pro" | "pro-1.5" | "pro" => Some(GeminiModel::Pro1_5),
            _ => None,
        }
    }
}

impl std::fmt::Display for GeminiModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Rate limit configuration for API request throttling.
///
/// Gemini enforces both requests-per-minute (RPM) and tokens-per-minute (TPM)
/// limits. This configuration allows fine-tuning the client's rate limiting
/// behavior to stay within quotas while maximizing throughput.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per minute.
    ///
    /// Default: 4000 (Gemini Flash tier 1 limit)
    pub rpm_limit: u32,

    /// Maximum tokens per minute.
    ///
    /// Default: 4,000,000 (Gemini Flash tier 1 limit)
    pub tpm_limit: u32,

    /// Burst allowance as a fraction (0.0 - 1.0).
    ///
    /// Allows temporary bursts above the sustained rate.
    /// Default: 0.1 (10% burst)
    pub burst_allowance: f32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            rpm_limit: 4000,
            tpm_limit: 4_000_000,
            burst_allowance: 0.1,
        }
    }
}

impl RateLimitConfig {
    /// Create rate limit config for Gemini Flash (default tier).
    pub fn flash_default() -> Self {
        Self::default()
    }

    /// Create rate limit config for Gemini Pro.
    pub fn pro_default() -> Self {
        Self {
            rpm_limit: 1000,
            tpm_limit: 4_000_000,
            burst_allowance: 0.1,
        }
    }

    /// Create a conservative rate limit for shared API keys.
    pub fn conservative() -> Self {
        Self {
            rpm_limit: 100,
            tpm_limit: 100_000,
            burst_allowance: 0.0,
        }
    }

    /// Set the RPM limit.
    pub fn with_rpm(mut self, rpm: u32) -> Self {
        self.rpm_limit = rpm;
        self
    }

    /// Set the TPM limit.
    pub fn with_tpm(mut self, tpm: u32) -> Self {
        self.tpm_limit = tpm;
        self
    }

    /// Set the burst allowance.
    pub fn with_burst(mut self, burst: f32) -> Self {
        self.burst_allowance = burst.clamp(0.0, 1.0);
        self
    }
}

/// Retry policy configuration for handling transient failures.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    ///
    /// Default: 3
    pub max_retries: u32,

    /// Initial delay before first retry.
    ///
    /// Default: 1 second
    pub initial_delay: Duration,

    /// Maximum delay between retries (caps exponential backoff).
    ///
    /// Default: 60 seconds
    pub max_delay: Duration,

    /// Backoff multiplier for exponential backoff.
    ///
    /// Default: 2.0
    pub backoff_multiplier: f64,

    /// Jitter factor to add randomness to retry delays (0.0 - 1.0).
    ///
    /// Helps prevent thundering herd when many clients retry simultaneously.
    /// Default: 0.1 (10% jitter)
    pub jitter: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: 0.1,
        }
    }
}

impl RetryConfig {
    /// Calculate the delay for a given retry attempt (0-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base_delay =
            self.initial_delay.as_secs_f64() * self.backoff_multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max_delay.as_secs_f64());

        // Apply jitter
        let jitter_range = capped_delay * self.jitter;
        let jitter = if jitter_range > 0.0 {
            (rand::random::<f64>() - 0.5) * 2.0 * jitter_range
        } else {
            0.0
        };

        Duration::from_secs_f64((capped_delay + jitter).max(0.0))
    }

    /// Set the maximum number of retries.
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    /// Set the initial delay.
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Disable retries.
    pub fn no_retries() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }
}

/// Main configuration for the Gemini client.
///
/// # Example
///
/// ```rust,ignore
/// use cc_gemini::GeminiConfig;
/// use std::time::Duration;
///
/// let config = GeminiConfig::new("your-api-key")
///     .with_model(GeminiModel::Flash2_0)
///     .with_timeout(Duration::from_secs(30))
///     .with_max_cost(5.0);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeminiConfig {
    /// Gemini API key.
    ///
    /// Required. Can be loaded from `GEMINI_API_KEY` environment variable.
    #[serde(skip_serializing)]
    pub api_key: String,

    /// API base URL.
    ///
    /// Default: `https://generativelanguage.googleapis.com/v1beta`
    pub api_base_url: String,

    /// Model to use for requests.
    ///
    /// Default: Gemini 2.0 Flash
    pub model: GeminiModel,

    /// Request timeout duration.
    ///
    /// Default: 120 seconds
    pub timeout: Duration,

    /// Rate limiting configuration.
    pub rate_limit: RateLimitConfig,

    /// Retry policy configuration.
    pub retry: RetryConfig,

    /// Maximum cost limit in USD.
    ///
    /// If set, requests will be rejected when accumulated cost exceeds this limit.
    /// Default: None (no limit)
    pub max_cost: Option<f64>,

    /// Whether to track and report detailed cost metrics.
    ///
    /// Default: true
    pub track_costs: bool,

    /// Custom HTTP headers to include in requests.
    #[serde(default)]
    pub custom_headers: std::collections::HashMap<String, String>,
}

impl GeminiConfig {
    /// Create a new configuration with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_base_url: DEFAULT_API_BASE_URL.to_string(),
            model: GeminiModel::default(),
            timeout: Duration::from_secs(120),
            rate_limit: RateLimitConfig::default(),
            retry: RetryConfig::default(),
            max_cost: None,
            track_costs: true,
            custom_headers: std::collections::HashMap::new(),
        }
    }

    /// Load configuration from environment variables.
    ///
    /// # Environment Variables
    ///
    /// - `GEMINI_API_KEY` (required): API key
    /// - `GEMINI_MODEL` (optional): Model name
    /// - `GEMINI_MAX_COST` (optional): Maximum cost in USD
    /// - `GEMINI_TIMEOUT_SECS` (optional): Timeout in seconds
    /// - `GEMINI_RPM_LIMIT` (optional): Requests per minute limit
    /// - `GEMINI_TPM_LIMIT` (optional): Tokens per minute limit
    ///
    /// # Errors
    ///
    /// Returns `GeminiError::MissingEnvVar` if `GEMINI_API_KEY` is not set.
    pub fn from_env() -> Result<Self> {
        let api_key =
            std::env::var(ENV_API_KEY).map_err(|_| GeminiError::missing_env(ENV_API_KEY))?;

        let mut config = Self::new(api_key);

        // Parse optional model
        if let Ok(model_str) = std::env::var(ENV_MODEL) {
            if let Some(model) = GeminiModel::from_string(&model_str) {
                config.model = model;
            }
        }

        // Parse optional max cost
        if let Ok(max_cost_str) = std::env::var(ENV_MAX_COST) {
            if let Ok(max_cost) = max_cost_str.parse::<f64>() {
                config.max_cost = Some(max_cost);
            }
        }

        // Parse optional timeout
        if let Ok(timeout_str) = std::env::var("GEMINI_TIMEOUT_SECS") {
            if let Ok(timeout_secs) = timeout_str.parse::<u64>() {
                config.timeout = Duration::from_secs(timeout_secs);
            }
        }

        // Parse optional rate limits
        if let Ok(rpm_str) = std::env::var("GEMINI_RPM_LIMIT") {
            if let Ok(rpm) = rpm_str.parse::<u32>() {
                config.rate_limit.rpm_limit = rpm;
            }
        }

        if let Ok(tpm_str) = std::env::var("GEMINI_TPM_LIMIT") {
            if let Ok(tpm) = tpm_str.parse::<u32>() {
                config.rate_limit.tpm_limit = tpm;
            }
        }

        Ok(config)
    }

    /// Set the model to use.
    pub fn with_model(mut self, model: GeminiModel) -> Self {
        self.model = model;
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum cost limit.
    pub fn with_max_cost(mut self, max_cost: f64) -> Self {
        self.max_cost = Some(max_cost);
        self
    }

    /// Set the rate limit configuration.
    pub fn with_rate_limit(mut self, rate_limit: RateLimitConfig) -> Self {
        self.rate_limit = rate_limit;
        self
    }

    /// Set the retry configuration.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    /// Set the API base URL.
    pub fn with_api_base_url(mut self, url: impl Into<String>) -> Self {
        self.api_base_url = url.into();
        self
    }

    /// Add a custom header to all requests.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.insert(key.into(), value.into());
        self
    }

    /// Disable cost tracking.
    pub fn without_cost_tracking(mut self) -> Self {
        self.track_costs = false;
        self
    }

    /// Build the full API endpoint URL for a path.
    pub fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.api_base_url, path)
    }

    /// Build the content generation endpoint URL.
    pub fn generate_content_endpoint(&self) -> String {
        self.endpoint(&self.model.generate_content_path())
    }

    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns `GeminiError::ConfigError` if the configuration is invalid.
    pub fn validate(&self) -> Result<()> {
        if self.api_key.is_empty() {
            return Err(GeminiError::config_error("API key cannot be empty"));
        }

        if self.api_key.len() < 10 {
            return Err(GeminiError::config_error(
                "API key appears too short - please verify",
            ));
        }

        if self.timeout.is_zero() {
            return Err(GeminiError::config_error("Timeout cannot be zero"));
        }

        if let Some(max_cost) = self.max_cost {
            if max_cost <= 0.0 {
                return Err(GeminiError::config_error(
                    "Max cost must be positive if specified",
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_properties() {
        let model = GeminiModel::Flash2_0;
        assert_eq!(model.as_str(), "gemini-2.0-flash");
        assert!(model.supports_images());
        assert!(model.supports_video());
        assert!(model.context_window() > 0);
    }

    #[test]
    fn test_model_pricing() {
        let flash = GeminiModel::Flash2_0;
        let pro = GeminiModel::Pro1_5;

        // Flash should be cheaper than Pro
        assert!(flash.input_cost_per_million() < pro.input_cost_per_million());
        assert!(flash.output_cost_per_million() < pro.output_cost_per_million());
    }

    #[test]
    fn test_model_from_string() {
        assert_eq!(
            GeminiModel::from_string("gemini-2.0-flash"),
            Some(GeminiModel::Flash2_0)
        );
        assert_eq!(
            GeminiModel::from_string("flash2"),
            Some(GeminiModel::Flash2_0)
        );
        assert_eq!(GeminiModel::from_string("pro"), Some(GeminiModel::Pro1_5));
        assert_eq!(GeminiModel::from_string("invalid"), None);
    }

    #[test]
    fn test_config_builder() {
        let config = GeminiConfig::new("test-key")
            .with_model(GeminiModel::Pro1_5)
            .with_timeout(Duration::from_secs(30))
            .with_max_cost(10.0);

        assert_eq!(config.model, GeminiModel::Pro1_5);
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_cost, Some(10.0));
    }

    #[test]
    fn test_config_validation() {
        let valid_config = GeminiConfig::new("valid-api-key-here");
        assert!(valid_config.validate().is_ok());

        let empty_key = GeminiConfig::new("");
        assert!(empty_key.validate().is_err());

        let short_key = GeminiConfig::new("short");
        assert!(short_key.validate().is_err());
    }

    #[test]
    fn test_retry_delay_calculation() {
        let config = RetryConfig::default();

        let delay0 = config.delay_for_attempt(0);
        let delay1 = config.delay_for_attempt(1);
        let delay2 = config.delay_for_attempt(2);

        // Delays should increase (approximately, accounting for jitter)
        assert!(delay1.as_secs_f64() > delay0.as_secs_f64() * 0.5);
        assert!(delay2.as_secs_f64() > delay1.as_secs_f64() * 0.5);

        // Should not exceed max delay
        let delay10 = config.delay_for_attempt(10);
        assert!(delay10 <= config.max_delay + Duration::from_secs(10)); // Allow for jitter
    }

    #[test]
    fn test_endpoint_building() {
        let config = GeminiConfig::new("key").with_model(GeminiModel::Flash2_0);
        let endpoint = config.generate_content_endpoint();

        assert!(endpoint.contains("gemini-2.0-flash"));
        assert!(endpoint.contains("generateContent"));
    }
}
