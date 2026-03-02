//! Dual token bucket rate limiter for Gemini API.
//!
//! This module implements a production-grade rate limiter that enforces both
//! requests-per-minute (RPM) and tokens-per-minute (TPM) limits simultaneously.
//! The implementation uses the token bucket algorithm with automatic refilling.
//!
//! # Algorithm
//!
//! The token bucket algorithm allows for smooth rate limiting with burst tolerance:
//!
//! 1. Each bucket starts full with `max_tokens` tokens
//! 2. Tokens are consumed when requests are made
//! 3. Tokens are continuously refilled at a rate of `refill_rate` per second
//! 4. If not enough tokens are available, the request waits until tokens refill
//!
//! # Thread Safety
//!
//! The rate limiter is designed for concurrent access from multiple async tasks.
//! All operations are protected by async mutexes to ensure correctness.
//!
//! # Example
//!
//! ```rust,ignore
//! use cc_gemini::{RateLimiter, RateLimitConfig};
//!
//! let limiter = RateLimiter::new(RateLimitConfig::default());
//!
//! // Acquire permission for a request with ~1000 estimated tokens
//! let wait_time = limiter.acquire(1000).await;
//! println!("Waited {:?} for rate limit", wait_time);
//!
//! // Make API request...
//!
//! // Report actual usage for accuracy
//! limiter.report_usage(1200, 1000);
//! ```

use crate::config::RateLimitConfig;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, trace, warn};

/// Token bucket implementation for rate limiting.
///
/// A token bucket allows smooth rate limiting by:
/// - Starting with a full bucket of tokens
/// - Consuming tokens on each request
/// - Continuously refilling tokens at a fixed rate
/// - Blocking when tokens are exhausted
#[derive(Debug)]
struct TokenBucket {
    /// Current token count (can be fractional during calculations).
    tokens: f64,

    /// Maximum token capacity (includes burst allowance).
    max_tokens: f64,

    /// Tokens added per second.
    refill_rate: f64,

    /// Timestamp of last token refill.
    last_refill: Instant,

    /// Name for logging purposes.
    name: &'static str,
}

impl TokenBucket {
    /// Create a new token bucket.
    ///
    /// # Arguments
    ///
    /// * `max_tokens` - Maximum token capacity
    /// * `refill_rate` - Tokens added per second
    /// * `name` - Identifier for logging
    fn new(max_tokens: f64, refill_rate: f64, name: &'static str) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
            name,
        }
    }

    /// Refill tokens based on elapsed time.
    ///
    /// This method should be called before checking or consuming tokens
    /// to ensure the bucket reflects the current state.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let refilled = elapsed * self.refill_rate;

        self.tokens = (self.tokens + refilled).min(self.max_tokens);
        self.last_refill = now;

        trace!(
            bucket = self.name,
            tokens = self.tokens,
            refilled = refilled,
            "Token bucket refilled"
        );
    }

    /// Attempt to acquire tokens without blocking.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens to acquire
    ///
    /// # Returns
    ///
    /// `true` if tokens were acquired, `false` if insufficient tokens.
    fn try_acquire(&mut self, tokens: f64) -> bool {
        self.refill();

        if self.tokens >= tokens {
            self.tokens -= tokens;
            trace!(
                bucket = self.name,
                requested = tokens,
                remaining = self.tokens,
                "Tokens acquired"
            );
            true
        } else {
            trace!(
                bucket = self.name,
                requested = tokens,
                available = self.tokens,
                "Insufficient tokens"
            );
            false
        }
    }

    /// Calculate time until the requested tokens become available.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens needed
    ///
    /// # Returns
    ///
    /// Duration until tokens are available. Returns `Duration::ZERO` if tokens
    /// are already available.
    fn time_until_available(&mut self, tokens: f64) -> Duration {
        self.refill();

        if self.tokens >= tokens {
            return Duration::ZERO;
        }

        let needed = tokens - self.tokens;
        let wait_secs = needed / self.refill_rate;

        Duration::from_secs_f64(wait_secs)
    }

    /// Get current token count (for monitoring).
    fn available_tokens(&mut self) -> f64 {
        self.refill();
        self.tokens
    }
}

/// Dual token bucket rate limiter for Gemini API.
///
/// Enforces both requests-per-minute (RPM) and tokens-per-minute (TPM) limits
/// using two independent token buckets. Both limits must be satisfied for a
/// request to proceed.
///
/// # Thread Safety
///
/// This struct is safe to share across async tasks via `Arc`. All bucket
/// operations are protected by async mutexes.
///
/// # Metrics
///
/// The limiter tracks cumulative statistics:
/// - Total requests made
/// - Total tokens consumed
/// - Total time spent waiting
///
/// These can be queried for monitoring and debugging.
pub struct RateLimiter {
    /// Request bucket (RPM).
    rpm_bucket: Arc<Mutex<TokenBucket>>,

    /// Token bucket (TPM).
    tpm_bucket: Arc<Mutex<TokenBucket>>,

    /// Total requests processed.
    total_requests: AtomicU64,

    /// Total tokens consumed (estimated).
    total_tokens: AtomicU64,

    /// Total time spent waiting for rate limits.
    total_wait_ns: AtomicU64,

    /// Configuration reference.
    config: RateLimitConfig,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Rate limit configuration
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use cc_gemini::{RateLimiter, RateLimitConfig};
    ///
    /// let limiter = RateLimiter::new(RateLimitConfig {
    ///     rpm_limit: 4000,
    ///     tpm_limit: 4_000_000,
    ///     burst_allowance: 0.1,
    /// });
    /// ```
    pub fn new(config: RateLimitConfig) -> Self {
        let burst_factor = 1.0 + config.burst_allowance as f64;

        // RPM bucket: max = rpm_limit * burst, refill = rpm_limit / 60 per second
        let rpm_max = config.rpm_limit as f64 * burst_factor;
        let rpm_rate = config.rpm_limit as f64 / 60.0;

        // TPM bucket: max = tpm_limit * burst, refill = tpm_limit / 60 per second
        let tpm_max = config.tpm_limit as f64 * burst_factor;
        let tpm_rate = config.tpm_limit as f64 / 60.0;

        debug!(
            rpm_limit = config.rpm_limit,
            tpm_limit = config.tpm_limit,
            burst = config.burst_allowance,
            "Rate limiter initialized"
        );

        Self {
            rpm_bucket: Arc::new(Mutex::new(TokenBucket::new(rpm_max, rpm_rate, "RPM"))),
            tpm_bucket: Arc::new(Mutex::new(TokenBucket::new(tpm_max, tpm_rate, "TPM"))),
            total_requests: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
            total_wait_ns: AtomicU64::new(0),
            config,
        }
    }

    /// Create a rate limiter with default Gemini Flash limits.
    ///
    /// Uses:
    /// - 4000 RPM
    /// - 4,000,000 TPM
    /// - 10% burst allowance
    pub fn default_gemini() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// Acquire permission to make a request with estimated token count.
    ///
    /// This method blocks until both RPM and TPM limits allow the request.
    /// The blocking is cooperative and uses async sleep, so other tasks
    /// can continue while waiting.
    ///
    /// # Arguments
    ///
    /// * `estimated_tokens` - Estimated total tokens for the request
    ///   (input + output + image tokens)
    ///
    /// # Returns
    ///
    /// Duration spent waiting for rate limits. Returns `Duration::ZERO`
    /// if no waiting was required.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Estimate tokens for an image analysis request
    /// let image_tokens = 1000; // ~1K tokens for a typical image
    /// let prompt_tokens = 50;
    /// let expected_output = 200;
    /// let estimated = image_tokens + prompt_tokens + expected_output;
    ///
    /// let wait = limiter.acquire(estimated).await;
    /// // Now safe to make the API request
    /// ```
    pub async fn acquire(&self, estimated_tokens: u32) -> Duration {
        let start = Instant::now();
        let tokens = estimated_tokens as f64;

        loop {
            // Check RPM bucket
            let rpm_wait = {
                let mut bucket = self.rpm_bucket.lock().await;
                bucket.time_until_available(1.0)
            };

            // Check TPM bucket
            let tpm_wait = {
                let mut bucket = self.tpm_bucket.lock().await;
                bucket.time_until_available(tokens)
            };

            // Take the longer wait time
            let wait = rpm_wait.max(tpm_wait);

            if wait.is_zero() {
                // Both buckets have capacity - try to acquire atomically
                let mut rpm = self.rpm_bucket.lock().await;
                let mut tpm = self.tpm_bucket.lock().await;

                // Double-check after acquiring both locks
                if rpm.try_acquire(1.0) && tpm.try_acquire(tokens) {
                    // Successfully acquired - update metrics
                    self.total_requests.fetch_add(1, Ordering::Relaxed);
                    self.total_tokens
                        .fetch_add(estimated_tokens as u64, Ordering::Relaxed);

                    let elapsed = start.elapsed();
                    self.total_wait_ns
                        .fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);

                    if elapsed > Duration::from_millis(100) {
                        debug!(
                            waited_ms = elapsed.as_millis(),
                            estimated_tokens,
                            "Rate limit wait completed"
                        );
                    }

                    return elapsed;
                }

                // Race condition: someone else acquired while we were locking
                // Drop locks and retry
                drop(rpm);
                drop(tpm);
                continue;
            }

            // Need to wait - log if significant
            if wait > Duration::from_secs(1) {
                debug!(
                    wait_secs = wait.as_secs_f64(),
                    rpm_wait_ms = rpm_wait.as_millis(),
                    tpm_wait_ms = tpm_wait.as_millis(),
                    "Rate limiter waiting"
                );
            }

            sleep(wait).await;
        }
    }

    /// Try to acquire permission without blocking.
    ///
    /// # Arguments
    ///
    /// * `estimated_tokens` - Estimated total tokens for the request
    ///
    /// # Returns
    ///
    /// `true` if permission was granted, `false` if it would require waiting.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if limiter.try_acquire(1000).await {
    ///     // Proceed with request
    /// } else {
    ///     // Handle rate limit - maybe skip or queue
    /// }
    /// ```
    pub async fn try_acquire(&self, estimated_tokens: u32) -> bool {
        let tokens = estimated_tokens as f64;

        let mut rpm = self.rpm_bucket.lock().await;
        let mut tpm = self.tpm_bucket.lock().await;

        if rpm.try_acquire(1.0) && tpm.try_acquire(tokens) {
            self.total_requests.fetch_add(1, Ordering::Relaxed);
            self.total_tokens
                .fetch_add(estimated_tokens as u64, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Get time until a request of the given size can be made.
    ///
    /// # Arguments
    ///
    /// * `estimated_tokens` - Estimated total tokens for the request
    ///
    /// # Returns
    ///
    /// Duration until both rate limits would allow the request.
    /// Returns `Duration::ZERO` if the request can proceed immediately.
    pub async fn time_until_available(&self, estimated_tokens: u32) -> Duration {
        let tokens = estimated_tokens as f64;

        let rpm_wait = {
            let mut bucket = self.rpm_bucket.lock().await;
            bucket.time_until_available(1.0)
        };

        let tpm_wait = {
            let mut bucket = self.tpm_bucket.lock().await;
            bucket.time_until_available(tokens)
        };

        rpm_wait.max(tpm_wait)
    }

    /// Report actual token usage after a request completes.
    ///
    /// This helps improve rate limiting accuracy when estimates differ
    /// significantly from actual usage. Large discrepancies are logged
    /// as warnings.
    ///
    /// # Arguments
    ///
    /// * `actual_tokens` - Actual tokens used (from API response)
    /// * `estimated_tokens` - Originally estimated tokens
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let estimated = 1000;
    /// limiter.acquire(estimated).await;
    ///
    /// let response = client.generate_content(...).await?;
    /// let actual = response.usage_metadata.total_token_count;
    ///
    /// limiter.report_usage(actual, estimated);
    /// ```
    pub fn report_usage(&self, actual_tokens: u32, estimated_tokens: u32) {
        let diff = actual_tokens as i64 - estimated_tokens as i64;

        // Warn if estimate was significantly off (>50% error)
        if diff.unsigned_abs() > estimated_tokens as u64 / 2 {
            warn!(
                estimated = estimated_tokens,
                actual = actual_tokens,
                diff = diff,
                "Token estimate significantly off - consider adjusting estimation"
            );
        }

        // Adjust total tokens counter for accurate metrics
        let current = self.total_tokens.load(Ordering::Relaxed);
        let adjusted = (current as i64 + diff).max(0) as u64;
        self.total_tokens.store(adjusted, Ordering::Relaxed);
    }

    /// Get total requests made through this limiter.
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Get total tokens consumed (estimated, adjusted by actual usage).
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens.load(Ordering::Relaxed)
    }

    /// Get total time spent waiting for rate limits.
    pub fn total_wait_time(&self) -> Duration {
        Duration::from_nanos(self.total_wait_ns.load(Ordering::Relaxed))
    }

    /// Get current configuration.
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }

    /// Get current available capacity in both buckets.
    ///
    /// Useful for monitoring and debugging.
    ///
    /// # Returns
    ///
    /// Tuple of (available_rpm, available_tpm)
    pub async fn available_capacity(&self) -> (f64, f64) {
        let rpm = {
            let mut bucket = self.rpm_bucket.lock().await;
            bucket.available_tokens()
        };

        let tpm = {
            let mut bucket = self.tpm_bucket.lock().await;
            bucket.available_tokens()
        };

        (rpm, tpm)
    }

    /// Reset the rate limiter to full capacity.
    ///
    /// This is primarily useful for testing. In production, the buckets
    /// will naturally refill over time.
    pub async fn reset(&self) {
        let burst_factor = 1.0 + self.config.burst_allowance as f64;

        {
            let mut bucket = self.rpm_bucket.lock().await;
            bucket.tokens = self.config.rpm_limit as f64 * burst_factor;
            bucket.last_refill = Instant::now();
        }

        {
            let mut bucket = self.tpm_bucket.lock().await;
            bucket.tokens = self.config.tpm_limit as f64 * burst_factor;
            bucket.last_refill = Instant::now();
        }

        debug!("Rate limiter reset to full capacity");
    }
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("config", &self.config)
            .field("total_requests", &self.total_requests())
            .field("total_tokens", &self.total_tokens())
            .field("total_wait_time", &self.total_wait_time())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let limiter = RateLimiter::new(RateLimitConfig {
            rpm_limit: 100,
            tpm_limit: 10000,
            burst_allowance: 0.1,
        });

        // Should be able to acquire immediately
        let wait = limiter.acquire(100).await;
        assert!(wait < Duration::from_millis(50));

        assert_eq!(limiter.total_requests(), 1);
        assert_eq!(limiter.total_tokens(), 100);
    }

    #[tokio::test]
    async fn test_try_acquire() {
        let limiter = RateLimiter::new(RateLimitConfig {
            rpm_limit: 10,
            tpm_limit: 1000,
            burst_allowance: 0.0, // No burst for predictable testing
        });

        // Exhaust the RPM bucket
        for _ in 0..10 {
            assert!(limiter.try_acquire(50).await);
        }

        // Should fail now (RPM exhausted)
        assert!(!limiter.try_acquire(50).await);
    }

    #[tokio::test]
    async fn test_tpm_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            rpm_limit: 1000,
            tpm_limit: 500, // Low TPM for testing
            burst_allowance: 0.0,
        });

        // First request should succeed (uses 400 of 500 tokens)
        assert!(limiter.try_acquire(400).await);

        // Second request should fail (would need 400 more, only 100 available)
        assert!(!limiter.try_acquire(400).await);

        // Smaller request should succeed
        assert!(limiter.try_acquire(50).await);
    }

    #[tokio::test]
    async fn test_time_until_available() {
        let limiter = RateLimiter::new(RateLimitConfig {
            rpm_limit: 60, // 1 per second
            tpm_limit: 1_000_000,
            burst_allowance: 0.0,
        });

        // Exhaust RPM
        for _ in 0..60 {
            let _ = limiter.try_acquire(100).await;
        }

        // Should need to wait
        let wait = limiter.time_until_available(100).await;
        assert!(wait > Duration::ZERO);
        assert!(wait <= Duration::from_secs(2)); // Should be ~1 second
    }

    #[tokio::test]
    async fn test_report_usage() {
        let limiter = RateLimiter::default_gemini();

        limiter.acquire(1000).await;
        assert_eq!(limiter.total_tokens(), 1000);

        // Report that we actually used more
        limiter.report_usage(1500, 1000);
        assert_eq!(limiter.total_tokens(), 1500);

        // Report that we used less
        limiter.report_usage(800, 1000);
        // 1500 + (800 - 1000) = 1300
        assert_eq!(limiter.total_tokens(), 1300);
    }

    #[tokio::test]
    async fn test_available_capacity() {
        let limiter = RateLimiter::new(RateLimitConfig {
            rpm_limit: 100,
            tpm_limit: 10000,
            burst_allowance: 0.0,
        });

        let (rpm, tpm) = limiter.available_capacity().await;
        assert!((rpm - 100.0).abs() < 1.0);
        assert!((tpm - 10000.0).abs() < 1.0);

        // Use some capacity
        limiter.acquire(1000).await;

        let (rpm_after, tpm_after) = limiter.available_capacity().await;
        assert!((rpm_after - 99.0).abs() < 1.0);
        assert!((tpm_after - 9000.0).abs() < 100.0); // Allow for refill
    }

    #[tokio::test]
    async fn test_reset() {
        let limiter = RateLimiter::new(RateLimitConfig {
            rpm_limit: 10,
            tpm_limit: 1000,
            burst_allowance: 0.0,
        });

        // Exhaust capacity
        for _ in 0..10 {
            limiter.acquire(100).await;
        }

        // Should fail
        assert!(!limiter.try_acquire(100).await);

        // Reset
        limiter.reset().await;

        // Should succeed again
        assert!(limiter.try_acquire(100).await);
    }

    #[tokio::test]
    async fn test_burst_allowance() {
        let limiter = RateLimiter::new(RateLimitConfig {
            rpm_limit: 10,
            tpm_limit: 1000,
            burst_allowance: 0.5, // 50% burst
        });

        // Should be able to burst to 15 requests (10 * 1.5)
        for i in 0..15 {
            assert!(
                limiter.try_acquire(10).await,
                "Failed at request {}",
                i + 1
            );
        }

        // Should fail now
        assert!(!limiter.try_acquire(10).await);
    }
}
