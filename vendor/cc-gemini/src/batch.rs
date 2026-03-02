//! Gemini Batch API client for asynchronous bulk processing.
//!
//! The Batch API enables processing large volumes of requests at 50% of the
//! standard cost, with a target turnaround time of 24 hours (often much faster).
//!
//! # Features
//!
//! - **50% Cost Savings**: Batch processing at half the standard API cost
//! - **Inline & File Requests**: Support for both small batches and large JSONL files
//! - **File API Integration**: Automatic upload/download for large datasets
//! - **Job Management**: Create, poll, cancel, and delete batch jobs
//! - **Cost Tracking**: Integrated with existing cost tracking at batch pricing
//!
//! # Example
//!
//! ```rust,ignore
//! use cc_gemini::{BatchClient, BatchRequest, BatchJobState};
//!
//! // Create client
//! let client = BatchClient::from_env()?;
//!
//! // Submit inline batch (under 20MB)
//! let requests = vec![
//!     BatchRequest::text("request-1", "Describe photosynthesis"),
//!     BatchRequest::text("request-2", "What is quantum computing?"),
//! ];
//!
//! let job = client.create_inline("my-batch", requests).await?;
//! println!("Created job: {}", job.name);
//!
//! // Poll until complete
//! loop {
//!     let status = client.get_status(&job.name).await?;
//!     match status.state {
//!         BatchJobState::Succeeded => break,
//!         BatchJobState::Failed => panic!("Job failed"),
//!         _ => tokio::time::sleep(Duration::from_secs(30)).await,
//!     }
//! }
//!
//! // Get results
//! let results = client.get_results(&job).await?;
//! for result in results.responses {
//!     println!("{}: {}", result.key, result.text());
//! }
//! ```

use crate::config::GeminiModel;
use crate::cost::{Cost, CostTracker};
use crate::error::{GeminiError, Result};
use crate::types::request::GenerateContentRequest;
use crate::types::response::GenerateContentResponse;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, trace, warn};

/// Batch API base URL.
const BATCH_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// File API base URL for uploads.
const FILE_API_BASE: &str = "https://generativelanguage.googleapis.com/upload/v1beta/files";

/// Maximum size for inline requests (20MB).
const MAX_INLINE_SIZE: usize = 20 * 1024 * 1024;

/// Default polling interval for job status.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Batch job states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BatchJobState {
    /// Job has been created and is waiting to be processed.
    JobStatePending,
    /// Job is currently being processed.
    JobStateRunning,
    /// Job completed successfully.
    JobStateSucceeded,
    /// Job failed. Check error details.
    JobStateFailed,
    /// Job was cancelled by user.
    JobStateCancelled,
    /// Job expired (pending/running for >48 hours).
    JobStateExpired,
    /// Unknown state.
    #[serde(other)]
    Unknown,
}

impl BatchJobState {
    /// Check if the job is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            BatchJobState::JobStateSucceeded
                | BatchJobState::JobStateFailed
                | BatchJobState::JobStateCancelled
                | BatchJobState::JobStateExpired
        )
    }

    /// Check if the job completed successfully.
    pub fn is_success(&self) -> bool {
        matches!(self, BatchJobState::JobStateSucceeded)
    }
}

impl std::fmt::Display for BatchJobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatchJobState::JobStatePending => write!(f, "pending"),
            BatchJobState::JobStateRunning => write!(f, "running"),
            BatchJobState::JobStateSucceeded => write!(f, "succeeded"),
            BatchJobState::JobStateFailed => write!(f, "failed"),
            BatchJobState::JobStateCancelled => write!(f, "cancelled"),
            BatchJobState::JobStateExpired => write!(f, "expired"),
            BatchJobState::Unknown => write!(f, "unknown"),
        }
    }
}

/// Batch job statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchStats {
    /// Total number of requests in the batch.
    #[serde(default)]
    pub total_request_count: u64,
    /// Number of successfully processed requests.
    #[serde(default)]
    pub succeeded_request_count: u64,
    /// Number of failed requests.
    #[serde(default)]
    pub failed_request_count: u64,
}

/// Batch job metadata and status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchJob {
    /// Unique job identifier (e.g., "batches/123456789").
    pub name: String,
    /// User-defined display name.
    #[serde(default)]
    pub display_name: String,
    /// Current job state.
    #[serde(default)]
    pub state: Option<BatchJobState>,
    /// Job creation timestamp.
    #[serde(default)]
    pub create_time: Option<String>,
    /// Job completion timestamp.
    #[serde(default)]
    pub update_time: Option<String>,
    /// Job statistics.
    #[serde(default)]
    pub batch_stats: Option<BatchStats>,
    /// Error details if job failed.
    #[serde(default)]
    pub error: Option<BatchError>,
    /// Source configuration (input file or inline).
    #[serde(default)]
    pub src: Option<BatchSource>,
    /// Destination configuration (output file).
    #[serde(default)]
    pub dest: Option<BatchDest>,
}

impl BatchJob {
    /// Get the job state, defaulting to Pending if not set.
    pub fn state(&self) -> BatchJobState {
        self.state.unwrap_or(BatchJobState::JobStatePending)
    }

    /// Check if the job is complete (terminal state).
    pub fn is_complete(&self) -> bool {
        self.state().is_terminal()
    }

    /// Check if the job succeeded.
    pub fn is_success(&self) -> bool {
        self.state().is_success()
    }
}

/// Batch job error details.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchError {
    /// Error code.
    #[serde(default)]
    pub code: i32,
    /// Error message.
    #[serde(default)]
    pub message: String,
}

/// Batch source configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSource {
    /// File name for file-based input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// Inline requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inlined_requests: Option<Vec<InlineRequestWrapper>>,
}

/// Batch destination configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDest {
    /// Output file name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
}

/// Wrapper for inline request with key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineRequestWrapper {
    /// User-defined key for matching request to response.
    pub key: String,
    /// The actual request.
    pub request: GenerateContentRequest,
}

/// A single batch request with a unique key.
#[derive(Debug, Clone)]
pub struct BatchRequest {
    /// Unique key to identify this request in results.
    pub key: String,
    /// The generate content request.
    pub request: GenerateContentRequest,
    /// Optional metadata.
    pub metadata: HashMap<String, String>,
}

impl BatchRequest {
    /// Create a text-only batch request.
    pub fn text(key: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            request: GenerateContentRequest::text(prompt.into()),
            metadata: HashMap::new(),
        }
    }

    /// Create a batch request with a custom GenerateContentRequest.
    pub fn new(key: impl Into<String>, request: GenerateContentRequest) -> Self {
        Self {
            key: key.into(),
            request,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the request.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Convert to JSONL line format for file-based submission.
    pub fn to_jsonl_line(&self) -> Result<String> {
        let wrapper = serde_json::json!({
            "key": self.key,
            "request": self.request,
        });
        serde_json::to_string(&wrapper).map_err(GeminiError::JsonError)
    }
}

/// Batch job results.
#[derive(Debug, Clone)]
pub struct BatchResults {
    /// Job that produced these results.
    pub job_name: String,
    /// Individual response results.
    pub responses: Vec<BatchResponse>,
    /// Total cost of the batch (at 50% rate).
    pub total_cost: Cost,
}

impl BatchResults {
    /// Get a response by key.
    pub fn get(&self, key: &str) -> Option<&BatchResponse> {
        self.responses.iter().find(|r| r.key == key)
    }

    /// Check if all responses succeeded.
    pub fn all_succeeded(&self) -> bool {
        self.responses.iter().all(|r| r.is_success())
    }

    /// Get successful responses only.
    pub fn successful(&self) -> impl Iterator<Item = &BatchResponse> {
        self.responses.iter().filter(|r| r.is_success())
    }

    /// Get failed responses only.
    pub fn failed(&self) -> impl Iterator<Item = &BatchResponse> {
        self.responses.iter().filter(|r| !r.is_success())
    }
}

/// Individual batch response.
#[derive(Debug, Clone)]
pub struct BatchResponse {
    /// The request key this response corresponds to.
    pub key: String,
    /// The generated content response (if successful).
    pub response: Option<GenerateContentResponse>,
    /// Error message (if failed).
    pub error: Option<String>,
}

impl BatchResponse {
    /// Check if this response was successful.
    pub fn is_success(&self) -> bool {
        self.response.is_some() && self.error.is_none()
    }

    /// Get the text content from the response.
    pub fn text(&self) -> Option<&str> {
        self.response.as_ref().and_then(|r| r.text())
    }

    /// Get the full text content from all candidates.
    pub fn full_text(&self) -> String {
        self.response
            .as_ref()
            .map(|r| r.full_text())
            .unwrap_or_default()
    }
}

/// Configuration for the BatchClient.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// API key for authentication.
    pub api_key: String,
    /// Model to use for batch processing.
    pub model: GeminiModel,
    /// Request timeout.
    pub timeout: Duration,
    /// Polling interval for job status.
    pub poll_interval: Duration,
    /// Maximum cost limit (optional).
    pub max_cost: Option<f64>,
}

impl BatchConfig {
    /// Create a new batch config.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: GeminiModel::Flash2_0,
            timeout: Duration::from_secs(60),
            poll_interval: DEFAULT_POLL_INTERVAL,
            max_cost: None,
        }
    }

    /// Set the model to use.
    pub fn with_model(mut self, model: GeminiModel) -> Self {
        self.model = model;
        self
    }

    /// Set the polling interval.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Set the maximum cost limit.
    pub fn with_max_cost(mut self, max_cost: f64) -> Self {
        self.max_cost = Some(max_cost);
        self
    }

    /// Create config from environment variables.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| GeminiError::missing_env("GEMINI_API_KEY"))?;

        let mut config = Self::new(api_key);

        if let Ok(model_str) = std::env::var("GEMINI_MODEL") {
            config.model = match model_str.as_str() {
                "gemini-2.0-flash" => GeminiModel::Flash2_0,
                "gemini-2.0-flash-lite" => GeminiModel::Flash2_0Lite,
                "gemini-1.5-flash" => GeminiModel::Flash1_5,
                "gemini-1.5-pro" => GeminiModel::Pro1_5,
                _ => config.model,
            };
        }

        if let Ok(interval) = std::env::var("BATCH_POLLING_INTERVAL_SECS") {
            if let Ok(secs) = interval.parse::<u64>() {
                config.poll_interval = Duration::from_secs(secs);
            }
        }

        if let Ok(max_cost) = std::env::var("BATCH_MAX_COST") {
            if let Ok(cost) = max_cost.parse::<f64>() {
                config.max_cost = Some(cost);
            }
        }

        Ok(config)
    }
}

/// Gemini Batch API client.
///
/// Provides methods for creating, monitoring, and retrieving results from
/// batch processing jobs.
pub struct BatchClient {
    /// Configuration.
    config: BatchConfig,
    /// HTTP client.
    http_client: reqwest::Client,
    /// Cost tracker (tracks at 50% rate).
    cost_tracker: Arc<Mutex<CostTracker>>,
}

impl BatchClient {
    /// Create a new batch client with the given configuration.
    pub fn new(config: BatchConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .default_headers(headers)
            .build()
            .map_err(|e| {
                GeminiError::config_error(format!("Failed to create HTTP client: {}", e))
            })?;

        let mut cost_tracker = CostTracker::new(config.model);
        if let Some(max_cost) = config.max_cost {
            cost_tracker.set_limit(max_cost);
        }

        info!(
            model = %config.model,
            poll_interval = ?config.poll_interval,
            "Batch client initialized"
        );

        Ok(Self {
            config,
            http_client,
            cost_tracker: Arc::new(Mutex::new(cost_tracker)),
        })
    }

    /// Create a batch client from environment variables.
    pub fn from_env() -> Result<Self> {
        let config = BatchConfig::from_env()?;
        Self::new(config)
    }

    /// Create a batch job with inline requests.
    ///
    /// Use this for small batches (under 20MB total).
    ///
    /// # Arguments
    ///
    /// * `display_name` - Human-readable name for the batch
    /// * `requests` - List of batch requests
    ///
    /// # Returns
    ///
    /// The created batch job with its unique name.
    pub async fn create_inline(
        &self,
        display_name: &str,
        requests: Vec<BatchRequest>,
    ) -> Result<BatchJob> {
        // Convert to Gemini Batch API format (per v1beta documentation)
        // Each request should have a "key" for identification and a "request" body
        // API expects: { "requests": [{"key": "...", "request": {...}}, ...] }
        let api_requests: Vec<serde_json::Value> = requests
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "key": r.key,
                    "request": r.request
                })
            })
            .collect();

        let payload = serde_json::json!({
            "requests": api_requests
        });

        // Check size
        let payload_str = serde_json::to_string(&payload)?;
        if payload_str.len() > MAX_INLINE_SIZE {
            return Err(GeminiError::invalid_request(format!(
                "Inline batch too large ({} bytes). Use file-based submission for batches over {}MB.",
                payload_str.len(),
                MAX_INLINE_SIZE / 1024 / 1024
            )));
        }

        let url = format!(
            "{}/models/{}:batchGenerateContent?key={}",
            BATCH_API_BASE,
            self.config.model.as_str(),
            self.config.api_key
        );

        debug!(
            display_name,
            request_count = api_requests.len(),
            "Creating inline batch job"
        );

        let response = self.http_client.post(&url).json(&payload).send().await?;

        self.parse_batch_response(response).await
    }

    /// Create a batch job from an uploaded JSONL file.
    ///
    /// Use this for large batches. First upload the file using `upload_jsonl`,
    /// then pass the returned file URI to this method.
    ///
    /// # Arguments
    ///
    /// * `display_name` - Human-readable name for the batch
    /// * `file_name` - File URI from upload (e.g., "files/abc123")
    ///
    /// # Returns
    ///
    /// The created batch job with its unique name.
    pub async fn create_from_file(&self, display_name: &str, file_name: &str) -> Result<BatchJob> {
        // Per Gemini Batch API spec for file-based input:
        // { "batch": { "display_name": "...", "input_config": { "gcs_uri": "..." } } }
        let payload = serde_json::json!({
            "batch": {
                "display_name": display_name,
                "input_config": {
                    "gcs_uri": file_name
                }
            }
        });

        let url = format!(
            "{}/models/{}:batchGenerateContent?key={}",
            BATCH_API_BASE,
            self.config.model.as_str(),
            self.config.api_key
        );

        debug!(display_name, file_name, "Creating file-based batch job");

        let response = self.http_client.post(&url).json(&payload).send().await?;

        self.parse_batch_response(response).await
    }

    /// Upload a JSONL file for batch processing.
    ///
    /// The file should contain one JSON object per line, each with
    /// "key" and "request" fields.
    ///
    /// # Arguments
    ///
    /// * `content` - JSONL file content as bytes
    /// * `display_name` - Optional display name for the file
    ///
    /// # Returns
    ///
    /// The file URI to use with `create_from_file`.
    pub async fn upload_jsonl(&self, content: &[u8], display_name: Option<&str>) -> Result<String> {
        let display_name = display_name.unwrap_or("batch_input.jsonl");
        let num_bytes = content.len();

        // Step 1: Start resumable upload
        let start_url = format!("{}?key={}", FILE_API_BASE, self.config.api_key);

        let start_response = self
            .http_client
            .post(&start_url)
            .header("X-Goog-Upload-Protocol", "resumable")
            .header("X-Goog-Upload-Command", "start")
            .header("X-Goog-Upload-Header-Content-Length", num_bytes.to_string())
            .header("X-Goog-Upload-Header-Content-Type", "application/jsonl")
            .json(&serde_json::json!({
                "file": {
                    "displayName": display_name
                }
            }))
            .send()
            .await?;

        if !start_response.status().is_success() {
            let body = start_response.text().await?;
            return Err(GeminiError::ApiError {
                status: 500,
                message: format!("Failed to start upload: {}", body),
                raw_response: Some(body),
            });
        }

        // Get upload URL from response headers
        let upload_url = start_response
            .headers()
            .get("x-goog-upload-url")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| GeminiError::internal("Missing upload URL in response"))?
            .to_string();

        // Step 2: Upload the actual bytes
        let upload_response = self
            .http_client
            .post(&upload_url)
            .header("Content-Length", num_bytes.to_string())
            .header("X-Goog-Upload-Offset", "0")
            .header("X-Goog-Upload-Command", "upload, finalize")
            .body(content.to_vec())
            .send()
            .await?;

        let upload_status = upload_response.status();
        if !upload_status.is_success() {
            let body = upload_response.text().await?;
            return Err(GeminiError::ApiError {
                status: upload_status.as_u16(),
                message: format!("Failed to upload file: {}", body),
                raw_response: Some(body),
            });
        }

        // Parse response to get file URI
        let file_info: serde_json::Value = upload_response.json().await?;
        let file_uri = file_info["file"]["uri"]
            .as_str()
            .or_else(|| file_info["file"]["name"].as_str())
            .ok_or_else(|| GeminiError::internal("Missing file URI in upload response"))?
            .to_string();

        info!(file_uri = %file_uri, bytes = num_bytes, "Uploaded JSONL file");

        Ok(file_uri)
    }

    /// Get the status of a batch job.
    ///
    /// # Arguments
    ///
    /// * `job_name` - The job name (e.g., "batches/123456789")
    ///
    /// # Returns
    ///
    /// The current job status.
    pub async fn get_status(&self, job_name: &str) -> Result<BatchJob> {
        let url = format!(
            "{}/{}?key={}",
            BATCH_API_BASE, job_name, self.config.api_key
        );

        trace!(job_name, "Getting batch job status");

        let response = self.http_client.get(&url).send().await?;

        self.parse_batch_response(response).await
    }

    /// Wait for a batch job to complete.
    ///
    /// Polls the job status at the configured interval until the job
    /// reaches a terminal state.
    ///
    /// # Arguments
    ///
    /// * `job_name` - The job name to wait for
    ///
    /// # Returns
    ///
    /// The final job status.
    pub async fn wait_for_completion(&self, job_name: &str) -> Result<BatchJob> {
        loop {
            let job = self.get_status(job_name).await?;

            if job.is_complete() {
                if !job.is_success() {
                    warn!(
                        job_name,
                        state = %job.state(),
                        error = ?job.error,
                        "Batch job did not succeed"
                    );
                }
                return Ok(job);
            }

            debug!(
                job_name,
                state = %job.state(),
                "Batch job still in progress, waiting..."
            );

            tokio::time::sleep(self.config.poll_interval).await;
        }
    }

    /// Get results from a completed batch job.
    ///
    /// # Arguments
    ///
    /// * `job` - The completed batch job
    ///
    /// # Returns
    ///
    /// The batch results with all responses.
    pub async fn get_results(&self, job: &BatchJob) -> Result<BatchResults> {
        if !job.is_success() {
            return Err(GeminiError::invalid_request(format!(
                "Cannot get results for job in state: {}",
                job.state()
            )));
        }

        // Check if results are in inline responses or a file
        if let Some(ref dest) = job.dest {
            if let Some(ref file_name) = dest.file_name {
                return self.download_results(&job.name, file_name).await;
            }
        }

        // For inline results, we need to get the full job details
        let url = format!(
            "{}/{}?key={}",
            BATCH_API_BASE, job.name, self.config.api_key
        );

        let response = self.http_client.get(&url).send().await?;
        let body = response.text().await?;
        let job_data: serde_json::Value = serde_json::from_str(&body)?;

        // Parse inline responses
        let responses = self.parse_inline_responses(&job_data)?;

        Ok(BatchResults {
            job_name: job.name.clone(),
            responses,
            total_cost: Cost::default(), // TODO: Calculate from responses
        })
    }

    /// Download results from a file.
    async fn download_results(&self, job_name: &str, file_name: &str) -> Result<BatchResults> {
        let url = format!(
            "{}/{}:download?alt=media&key={}",
            BATCH_API_BASE.replace("/v1beta", "/download/v1beta"),
            file_name,
            self.config.api_key
        );

        debug!(file_name, "Downloading batch results file");

        let response = self.http_client.get(&url).send().await?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await?;
            return Err(GeminiError::ApiError {
                status: status.as_u16(),
                message: format!("Failed to download results: {}", body),
                raw_response: Some(body),
            });
        }

        let body = response.text().await?;
        let responses = self.parse_jsonl_responses(&body)?;

        Ok(BatchResults {
            job_name: job_name.to_string(),
            responses,
            total_cost: Cost::default(),
        })
    }

    /// Parse inline responses from job data.
    fn parse_inline_responses(&self, job_data: &serde_json::Value) -> Result<Vec<BatchResponse>> {
        let responses = job_data["response"]["inlinedResponses"]
            .as_array()
            .ok_or_else(|| GeminiError::internal("Missing inlined responses"))?;

        let mut results = Vec::new();
        for resp in responses {
            let key = resp["key"].as_str().unwrap_or("").to_string();
            let response: Option<GenerateContentResponse> =
                serde_json::from_value(resp["response"].clone()).ok();
            let error = resp["error"]["message"].as_str().map(|s| s.to_string());

            results.push(BatchResponse {
                key,
                response,
                error,
            });
        }

        Ok(results)
    }

    /// Parse JSONL responses from file content.
    fn parse_jsonl_responses(&self, content: &str) -> Result<Vec<BatchResponse>> {
        let mut results = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let parsed: serde_json::Value = serde_json::from_str(line)?;
            let key = parsed["key"].as_str().unwrap_or("").to_string();
            let response: Option<GenerateContentResponse> =
                serde_json::from_value(parsed["response"].clone()).ok();
            let error = parsed["error"]["message"].as_str().map(|s| s.to_string());

            results.push(BatchResponse {
                key,
                response,
                error,
            });
        }

        Ok(results)
    }

    /// Cancel a running batch job.
    ///
    /// # Arguments
    ///
    /// * `job_name` - The job name to cancel
    pub async fn cancel(&self, job_name: &str) -> Result<()> {
        let url = format!(
            "{}/{}:cancel?key={}",
            BATCH_API_BASE, job_name, self.config.api_key
        );

        info!(job_name, "Cancelling batch job");

        let response = self.http_client.post(&url).send().await?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await?;
            return Err(GeminiError::ApiError {
                status: status.as_u16(),
                message: format!("Failed to cancel job: {}", body),
                raw_response: Some(body),
            });
        }

        Ok(())
    }

    /// Delete a batch job.
    ///
    /// # Arguments
    ///
    /// * `job_name` - The job name to delete
    pub async fn delete(&self, job_name: &str) -> Result<()> {
        let url = format!(
            "{}/{}:delete?key={}",
            BATCH_API_BASE, job_name, self.config.api_key
        );

        info!(job_name, "Deleting batch job");

        let response = self.http_client.post(&url).send().await?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await?;
            return Err(GeminiError::ApiError {
                status: status.as_u16(),
                message: format!("Failed to delete job: {}", body),
                raw_response: Some(body),
            });
        }

        Ok(())
    }

    /// List all batch jobs.
    ///
    /// # Arguments
    ///
    /// * `page_size` - Maximum number of jobs to return (default: 100)
    ///
    /// # Returns
    ///
    /// List of batch jobs.
    pub async fn list_jobs(&self, page_size: Option<u32>) -> Result<Vec<BatchJob>> {
        let page_size = page_size.unwrap_or(100);
        let url = format!(
            "{}/batches?pageSize={}&key={}",
            BATCH_API_BASE, page_size, self.config.api_key
        );

        let response = self.http_client.get(&url).send().await?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await?;
            return Err(GeminiError::ApiError {
                status: status.as_u16(),
                message: format!("Failed to list jobs: {}", body),
                raw_response: Some(body),
            });
        }

        let body: serde_json::Value = response.json().await?;
        let jobs: Vec<BatchJob> = body["batches"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(jobs)
    }

    /// Parse a batch job response.
    async fn parse_batch_response(&self, response: reqwest::Response) -> Result<BatchJob> {
        let status = response.status();
        let body = response.text().await?;

        trace!(status = %status, body_len = body.len(), "Received batch response");

        if status.is_success() {
            serde_json::from_str(&body).map_err(|e| GeminiError::MalformedResponse {
                message: format!("Failed to parse batch job: {}", e),
                raw_response: Some(body),
            })
        } else {
            Err(GeminiError::ApiError {
                status: status.as_u16(),
                message: format!("Batch API error: {}", body),
                raw_response: Some(body),
            })
        }
    }

    /// Get the cost tracker.
    pub async fn cost_tracker(&self) -> tokio::sync::MutexGuard<'_, CostTracker> {
        self.cost_tracker.lock().await
    }

    /// Get total cost at batch pricing (50% of standard).
    pub fn total_cost(&self) -> f64 {
        if let Ok(tracker) = self.cost_tracker.try_lock() {
            tracker.total_usd() * 0.5 // Batch API is 50% of standard cost
        } else {
            0.0
        }
    }
}

impl std::fmt::Debug for BatchClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchClient")
            .field("model", &self.config.model)
            .field("poll_interval", &self.config.poll_interval)
            .finish()
    }
}

/// Helper to build JSONL content from a list of requests.
pub fn build_jsonl(requests: &[BatchRequest]) -> Result<Vec<u8>> {
    let mut content = Vec::new();
    for request in requests {
        let line = request.to_jsonl_line()?;
        content.extend_from_slice(line.as_bytes());
        content.push(b'\n');
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_job_state() {
        assert!(BatchJobState::JobStateSucceeded.is_terminal());
        assert!(BatchJobState::JobStateFailed.is_terminal());
        assert!(BatchJobState::JobStateCancelled.is_terminal());
        assert!(!BatchJobState::JobStatePending.is_terminal());
        assert!(!BatchJobState::JobStateRunning.is_terminal());

        assert!(BatchJobState::JobStateSucceeded.is_success());
        assert!(!BatchJobState::JobStateFailed.is_success());
    }

    #[test]
    fn test_batch_request_creation() {
        let request = BatchRequest::text("req-1", "Hello, world!");
        assert_eq!(request.key, "req-1");

        let jsonl = request.to_jsonl_line().unwrap();
        assert!(jsonl.contains("req-1"));
        assert!(jsonl.contains("Hello, world!"));
    }

    #[test]
    fn test_batch_config() {
        let config = BatchConfig::new("test-key")
            .with_model(GeminiModel::Flash2_0)
            .with_poll_interval(Duration::from_secs(60))
            .with_max_cost(10.0);

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.model, GeminiModel::Flash2_0);
        assert_eq!(config.poll_interval, Duration::from_secs(60));
        assert_eq!(config.max_cost, Some(10.0));
    }

    #[test]
    fn test_build_jsonl() {
        let requests = vec![
            BatchRequest::text("req-1", "First prompt"),
            BatchRequest::text("req-2", "Second prompt"),
        ];

        let jsonl = build_jsonl(&requests).unwrap();
        let content = String::from_utf8(jsonl).unwrap();

        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("req-1"));
        assert!(lines[1].contains("req-2"));
    }
}
