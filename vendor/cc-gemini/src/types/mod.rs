//! Gemini API request and response types.
//!
//! This module contains the data structures for communicating with the
//! Gemini API, including request construction and response parsing.

/// Request types for the Gemini API.
pub mod request;

/// Response types from the Gemini API.
pub mod response;

// Re-export commonly used types at the module level for convenience
pub use request::{
    Content, GenerateContentRequest, GenerationConfig, HarmBlockThreshold, HarmCategory,
    InlineData, Part, Role, SafetySetting,
};
pub use response::{
    AnalysisResult, ApiErrorDetails, ApiErrorResponse, Candidate, GenerateContentResponse,
    PromptFeedback, ResponseContent, ResponsePart, SafetyRating, TokenUsage, UsageMetadata,
};
