//! Response types for the Gemini API.
//!
//! This module provides types for parsing Gemini API responses,
//! including generated content, usage metadata, and safety ratings.

use crate::cost::Cost;
use serde::{Deserialize, Serialize};

/// Response from the generateContent endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentResponse {
    /// Generated content candidates.
    #[serde(default)]
    pub candidates: Vec<Candidate>,

    /// Token usage metadata.
    #[serde(default)]
    pub usage_metadata: Option<UsageMetadata>,

    /// Prompt feedback (if content was blocked).
    #[serde(default)]
    pub prompt_feedback: Option<PromptFeedback>,
}

impl GenerateContentResponse {
    /// Get the primary response text.
    ///
    /// Returns the text from the first candidate's first text part.
    /// Returns `None` if no text is available.
    pub fn text(&self) -> Option<&str> {
        self.candidates
            .first()
            .and_then(|c| c.content.as_ref())
            .and_then(|content| content.parts.first())
            .map(|part| {
                let ResponsePart::Text { text } = part;
                text.as_str()
            })
    }

    /// Get all response text concatenated.
    pub fn full_text(&self) -> String {
        self.candidates
            .first()
            .and_then(|c| c.content.as_ref())
            .map(|content| {
                content
                    .parts
                    .iter()
                    .map(|part| {
                        let ResponsePart::Text { text } = part;
                        text.as_str()
                    })
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    }

    /// Check if the response was blocked by safety filters.
    pub fn is_blocked(&self) -> bool {
        self.prompt_feedback
            .as_ref()
            .map(|f| f.block_reason.is_some())
            .unwrap_or(false)
            || self
                .candidates
                .first()
                .map(|c| {
                    c.finish_reason
                        .as_ref()
                        .map(|r| r == "SAFETY" || r == "BLOCKED")
                        .unwrap_or(false)
                })
                .unwrap_or(false)
    }

    /// Get the block reason if content was blocked.
    pub fn block_reason(&self) -> Option<&str> {
        self.prompt_feedback
            .as_ref()
            .and_then(|f| f.block_reason.as_deref())
    }

    /// Get usage metadata.
    pub fn usage(&self) -> Option<&UsageMetadata> {
        self.usage_metadata.as_ref()
    }

    /// Extract cost from usage metadata.
    pub fn cost(&self) -> Cost {
        self.usage_metadata
            .as_ref()
            .map(|u| u.to_cost())
            .unwrap_or_default()
    }

    /// Get the finish reason for the first candidate.
    pub fn finish_reason(&self) -> Option<&str> {
        self.candidates
            .first()
            .and_then(|c| c.finish_reason.as_deref())
    }

    /// Check if the response completed successfully.
    pub fn is_complete(&self) -> bool {
        self.finish_reason()
            .map(|r| r == "STOP" || r == "END_TURN")
            .unwrap_or(false)
    }

    /// Get safety ratings for the response.
    pub fn safety_ratings(&self) -> Vec<&SafetyRating> {
        self.candidates
            .first()
            .map(|c| c.safety_ratings.iter().collect())
            .unwrap_or_default()
    }
}

/// A candidate response from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The generated content.
    pub content: Option<ResponseContent>,

    /// Reason the generation stopped.
    #[serde(default)]
    pub finish_reason: Option<String>,

    /// Safety ratings for this candidate.
    #[serde(default)]
    pub safety_ratings: Vec<SafetyRating>,

    /// Token count for this candidate.
    #[serde(default)]
    pub token_count: Option<u32>,

    /// Index of this candidate.
    #[serde(default)]
    pub index: Option<u32>,
}

/// Content in a response candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseContent {
    /// Parts that make up this content.
    pub parts: Vec<ResponsePart>,

    /// Role (always "model" for responses).
    #[serde(default)]
    pub role: Option<String>,
}

/// A part of response content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponsePart {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
}

/// Token usage metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    /// Number of tokens in the prompt.
    #[serde(default)]
    pub prompt_token_count: u64,

    /// Number of tokens in the response.
    #[serde(default)]
    pub candidates_token_count: u64,

    /// Total token count.
    #[serde(default)]
    pub total_token_count: u64,

    /// Number of tokens used for cached content.
    #[serde(default)]
    pub cached_content_token_count: Option<u64>,
}

impl UsageMetadata {
    /// Convert to a Cost struct.
    ///
    /// Note: This treats all prompt tokens as input tokens.
    /// Image tokens are included in prompt_token_count.
    pub fn to_cost(&self) -> Cost {
        Cost {
            input_tokens: self.prompt_token_count,
            output_tokens: self.candidates_token_count,
            image_tokens: 0, // Included in prompt_token_count
        }
    }

    /// Get the effective input tokens (excluding cached).
    pub fn effective_input_tokens(&self) -> u64 {
        self.prompt_token_count
            .saturating_sub(self.cached_content_token_count.unwrap_or(0))
    }
}

/// Feedback about the prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptFeedback {
    /// Reason the prompt was blocked.
    #[serde(default)]
    pub block_reason: Option<String>,

    /// Safety ratings for the prompt.
    #[serde(default)]
    pub safety_ratings: Vec<SafetyRating>,
}

/// Safety rating for content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyRating {
    /// The harm category.
    pub category: String,

    /// Probability of harm.
    pub probability: String,

    /// Whether this category was blocked.
    #[serde(default)]
    pub blocked: Option<bool>,
}

impl SafetyRating {
    /// Check if this rating indicates the content was blocked.
    pub fn is_blocked(&self) -> bool {
        self.blocked.unwrap_or(false) || self.probability == "HIGH" || self.probability == "MEDIUM"
    }

    /// Get a human-readable description of the rating.
    pub fn description(&self) -> String {
        format!(
            "{}: {} (blocked: {})",
            self.category,
            self.probability,
            self.blocked.unwrap_or(false)
        )
    }
}

/// Error response from the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// Error details.
    pub error: ApiErrorDetails,
}

/// Detailed error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorDetails {
    /// Error code.
    #[serde(default)]
    pub code: Option<u32>,

    /// Error message.
    pub message: String,

    /// Error status.
    #[serde(default)]
    pub status: Option<String>,

    /// Additional details.
    #[serde(default)]
    pub details: Option<Vec<serde_json::Value>>,
}

impl ApiErrorDetails {
    /// Check if this is a rate limit error.
    pub fn is_rate_limit(&self) -> bool {
        self.code == Some(429)
            || self.status.as_deref() == Some("RESOURCE_EXHAUSTED")
            || self.message.to_lowercase().contains("rate limit")
    }

    /// Check if this is an authentication error.
    pub fn is_auth_error(&self) -> bool {
        self.code == Some(401)
            || self.code == Some(403)
            || self.status.as_deref() == Some("UNAUTHENTICATED")
            || self.status.as_deref() == Some("PERMISSION_DENIED")
    }

    /// Check if this is a bad request error.
    pub fn is_bad_request(&self) -> bool {
        self.code == Some(400) || self.status.as_deref() == Some("INVALID_ARGUMENT")
    }

    /// Check if this is a server error.
    pub fn is_server_error(&self) -> bool {
        self.code.map(|c| c >= 500).unwrap_or(false)
            || self.status.as_deref() == Some("INTERNAL")
            || self.status.as_deref() == Some("UNAVAILABLE")
    }
}

/// Simplified analysis result for common use cases.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// Extracted text content.
    pub text: String,

    /// Description or analysis.
    pub description: String,

    /// Detected text (OCR results).
    pub detected_text: Vec<String>,

    /// Detected objects or entities.
    pub detected_objects: Vec<String>,

    /// Token usage for this request.
    pub tokens_used: TokenUsage,

    /// Estimated cost in USD.
    pub estimated_cost: f64,

    /// Raw response for debugging.
    pub raw_response: String,
}

/// Token usage summary.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// Input tokens.
    pub input_tokens: u64,

    /// Output tokens.
    pub output_tokens: u64,

    /// Image tokens (if applicable).
    pub image_tokens: u64,
}

impl TokenUsage {
    /// Total tokens used.
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.image_tokens
    }
}

impl From<&UsageMetadata> for TokenUsage {
    fn from(meta: &UsageMetadata) -> Self {
        Self {
            input_tokens: meta.prompt_token_count,
            output_tokens: meta.candidates_token_count,
            image_tokens: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_parsing() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello, world!"}],
                    "role": "model"
                },
                "finishReason": "STOP",
                "safetyRatings": []
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            }
        }"#;

        let response: GenerateContentResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.text(), Some("Hello, world!"));
        assert!(!response.is_blocked());
        assert!(response.is_complete());
        assert_eq!(response.usage().unwrap().total_token_count, 15);
    }

    #[test]
    fn test_blocked_response() {
        let json = r#"{
            "candidates": [{
                "finishReason": "SAFETY",
                "safetyRatings": [{
                    "category": "HARM_CATEGORY_DANGEROUS_CONTENT",
                    "probability": "HIGH",
                    "blocked": true
                }]
            }]
        }"#;

        let response: GenerateContentResponse = serde_json::from_str(json).unwrap();

        assert!(response.is_blocked());
        assert!(!response.is_complete());
    }

    #[test]
    fn test_usage_to_cost() {
        let usage = UsageMetadata {
            prompt_token_count: 100,
            candidates_token_count: 50,
            total_token_count: 150,
            cached_content_token_count: None,
        };

        let cost = usage.to_cost();
        assert_eq!(cost.input_tokens, 100);
        assert_eq!(cost.output_tokens, 50);
    }

    #[test]
    fn test_error_response() {
        let json = r#"{
            "error": {
                "code": 429,
                "message": "Rate limit exceeded",
                "status": "RESOURCE_EXHAUSTED"
            }
        }"#;

        let error: ApiErrorResponse = serde_json::from_str(json).unwrap();

        assert!(error.error.is_rate_limit());
        assert!(!error.error.is_auth_error());
    }
}
