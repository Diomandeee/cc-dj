//! Request types for the Gemini API.
//!
//! This module provides types for constructing Gemini API requests,
//! including support for text prompts, inline images, and generation
//! configuration.

use base64::Engine;
use serde::{Deserialize, Serialize};

/// Main request body for the generateContent endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentRequest {
    /// Content parts to send to the model.
    pub contents: Vec<Content>,

    /// Optional generation configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,

    /// Optional safety settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_settings: Option<Vec<SafetySetting>>,

    /// Optional system instruction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,
}

impl GenerateContentRequest {
    /// Create a simple text-only request.
    pub fn text(prompt: impl Into<String>) -> Self {
        Self {
            contents: vec![Content::user_text(prompt)],
            generation_config: None,
            safety_settings: None,
            system_instruction: None,
        }
    }

    /// Create an image analysis request.
    ///
    /// # Arguments
    ///
    /// * `image_data` - Raw image bytes
    /// * `mime_type` - MIME type (e.g., "image/jpeg", "image/png")
    /// * `prompt` - Analysis prompt
    pub fn image(image_data: &[u8], mime_type: &str, prompt: impl Into<String>) -> Self {
        Self {
            contents: vec![Content::user_with_image(image_data, mime_type, prompt)],
            generation_config: None,
            safety_settings: None,
            system_instruction: None,
        }
    }

    /// Create a multi-turn conversation request.
    pub fn conversation(contents: Vec<Content>) -> Self {
        Self {
            contents,
            generation_config: None,
            safety_settings: None,
            system_instruction: None,
        }
    }

    /// Set generation configuration.
    pub fn with_generation_config(mut self, config: GenerationConfig) -> Self {
        self.generation_config = Some(config);
        self
    }

    /// Set safety settings.
    pub fn with_safety_settings(mut self, settings: Vec<SafetySetting>) -> Self {
        self.safety_settings = Some(settings);
        self
    }

    /// Set system instruction.
    pub fn with_system_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.system_instruction = Some(Content::system(instruction));
        self
    }

    /// Set the maximum output tokens.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        let config = self.generation_config.get_or_insert_with(GenerationConfig::default);
        config.max_output_tokens = Some(max_tokens);
        self
    }

    /// Set the temperature for response randomness.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        let config = self.generation_config.get_or_insert_with(GenerationConfig::default);
        config.temperature = Some(temperature);
        self
    }

    /// Request JSON output format.
    pub fn with_json_output(mut self) -> Self {
        let config = self.generation_config.get_or_insert_with(GenerationConfig::default);
        config.response_mime_type = Some("application/json".to_string());
        self
    }
}

/// A content block in the request or response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    /// Role of the content author.
    pub role: Role,

    /// Parts that make up this content.
    pub parts: Vec<Part>,
}

impl Content {
    /// Create a user message with text.
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            parts: vec![Part::text(text)],
        }
    }

    /// Create a user message with image and text.
    pub fn user_with_image(
        image_data: &[u8],
        mime_type: &str,
        text: impl Into<String>,
    ) -> Self {
        Self {
            role: Role::User,
            parts: vec![Part::inline_image(image_data, mime_type), Part::text(text)],
        }
    }

    /// Create a model response.
    pub fn model_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::Model,
            parts: vec![Part::text(text)],
        }
    }

    /// Create a system instruction.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::User, // System instructions use "user" role in Gemini
            parts: vec![Part::text(text)],
        }
    }

    /// Add a part to this content.
    pub fn add_part(mut self, part: Part) -> Self {
        self.parts.push(part);
        self
    }
}

/// Role of a content author.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User input.
    User,
    /// Model response.
    Model,
}

/// A part of content (text, image, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Part {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Inline binary data (images, audio, etc.).
    InlineData {
        /// The binary data.
        #[serde(rename = "inlineData")]
        inline_data: InlineData,
    },
    /// Reference to uploaded file.
    FileData {
        /// The file reference.
        #[serde(rename = "fileData")]
        file_data: FileData,
    },
}

impl Part {
    /// Create a text part.
    pub fn text(text: impl Into<String>) -> Self {
        Part::Text { text: text.into() }
    }

    /// Create an inline image part from raw bytes.
    pub fn inline_image(data: &[u8], mime_type: &str) -> Self {
        Part::InlineData {
            inline_data: InlineData::from_bytes(data, mime_type),
        }
    }

    /// Create a file reference part.
    pub fn file_ref(uri: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Part::FileData {
            file_data: FileData {
                file_uri: uri.into(),
                mime_type: mime_type.into(),
            },
        }
    }
}

/// Inline binary data (base64 encoded).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineData {
    /// MIME type of the data.
    pub mime_type: String,

    /// Base64-encoded binary data.
    pub data: String,
}

impl InlineData {
    /// Create inline data from raw bytes.
    pub fn from_bytes(data: &[u8], mime_type: &str) -> Self {
        Self {
            mime_type: mime_type.to_string(),
            data: base64::engine::general_purpose::STANDARD.encode(data),
        }
    }

    /// Decode the base64 data to bytes.
    pub fn decode(&self) -> Result<Vec<u8>, base64::DecodeError> {
        base64::engine::general_purpose::STANDARD.decode(&self.data)
    }

    /// Estimated token count based on data size.
    pub fn estimated_tokens(&self) -> u64 {
        // Rough estimate: ~1 token per 100 bytes of original data
        let original_size = self.data.len() * 3 / 4; // Approximate decoded size
        ((original_size / 100) as u64).max(256)
    }
}

/// Reference to an uploaded file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileData {
    /// URI of the uploaded file.
    pub file_uri: String,

    /// MIME type of the file.
    pub mime_type: String,
}

/// Configuration for content generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    /// Maximum number of tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,

    /// Temperature for randomness (0.0 - 2.0).
    ///
    /// Lower values make output more deterministic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p nucleus sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// Response MIME type (e.g., "application/json").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,

    /// Number of response candidates to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<u32>,
}

impl GenerationConfig {
    /// Create config for deterministic output.
    pub fn deterministic() -> Self {
        Self {
            temperature: Some(0.0),
            top_p: Some(1.0),
            ..Default::default()
        }
    }

    /// Create config for creative output.
    pub fn creative() -> Self {
        Self {
            temperature: Some(1.0),
            top_p: Some(0.95),
            ..Default::default()
        }
    }

    /// Create config for JSON output.
    pub fn json_output() -> Self {
        Self {
            response_mime_type: Some("application/json".to_string()),
            ..Default::default()
        }
    }

    /// Set maximum output tokens.
    pub fn with_max_tokens(mut self, max: u32) -> Self {
        self.max_output_tokens = Some(max);
        self
    }

    /// Set temperature.
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp.clamp(0.0, 2.0));
        self
    }
}

/// Safety setting for content filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetySetting {
    /// The safety category.
    pub category: HarmCategory,

    /// The blocking threshold.
    pub threshold: HarmBlockThreshold,
}

impl SafetySetting {
    /// Create a safety setting.
    pub fn new(category: HarmCategory, threshold: HarmBlockThreshold) -> Self {
        Self { category, threshold }
    }

    /// Disable all safety filters (use with caution).
    pub fn none() -> Vec<SafetySetting> {
        vec![
            Self::new(HarmCategory::HateSpeech, HarmBlockThreshold::BlockNone),
            Self::new(HarmCategory::DangerousContent, HarmBlockThreshold::BlockNone),
            Self::new(HarmCategory::Harassment, HarmBlockThreshold::BlockNone),
            Self::new(HarmCategory::SexuallyExplicit, HarmBlockThreshold::BlockNone),
        ]
    }

    /// Use default safety settings.
    pub fn default_settings() -> Vec<SafetySetting> {
        vec![
            Self::new(HarmCategory::HateSpeech, HarmBlockThreshold::BlockMediumAndAbove),
            Self::new(HarmCategory::DangerousContent, HarmBlockThreshold::BlockMediumAndAbove),
            Self::new(HarmCategory::Harassment, HarmBlockThreshold::BlockMediumAndAbove),
            Self::new(HarmCategory::SexuallyExplicit, HarmBlockThreshold::BlockMediumAndAbove),
        ]
    }
}

/// Categories of harmful content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarmCategory {
    /// Hate speech.
    #[serde(rename = "HARM_CATEGORY_HATE_SPEECH")]
    HateSpeech,

    /// Dangerous content.
    #[serde(rename = "HARM_CATEGORY_DANGEROUS_CONTENT")]
    DangerousContent,

    /// Harassment.
    #[serde(rename = "HARM_CATEGORY_HARASSMENT")]
    Harassment,

    /// Sexually explicit content.
    #[serde(rename = "HARM_CATEGORY_SEXUALLY_EXPLICIT")]
    SexuallyExplicit,
}

/// Threshold for blocking harmful content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarmBlockThreshold {
    /// Block when probability is negligible or higher.
    BlockLowAndAbove,

    /// Block when probability is medium or higher.
    BlockMediumAndAbove,

    /// Block only high probability content.
    BlockOnlyHigh,

    /// Don't block any content.
    BlockNone,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_request() {
        let request = GenerateContentRequest::text("Hello, world!");
        assert_eq!(request.contents.len(), 1);
        assert_eq!(request.contents[0].role, Role::User);
    }

    #[test]
    fn test_image_request() {
        let image_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
        let request = GenerateContentRequest::image(&image_data, "image/jpeg", "Describe this");

        assert_eq!(request.contents.len(), 1);
        assert_eq!(request.contents[0].parts.len(), 2);
    }

    #[test]
    fn test_generation_config() {
        let request = GenerateContentRequest::text("Hello")
            .with_temperature(0.5)
            .with_max_tokens(100)
            .with_json_output();

        let config = request.generation_config.unwrap();
        assert_eq!(config.temperature, Some(0.5));
        assert_eq!(config.max_output_tokens, Some(100));
        assert_eq!(config.response_mime_type, Some("application/json".to_string()));
    }

    #[test]
    fn test_inline_data() {
        let data = b"test image data";
        let inline = InlineData::from_bytes(data, "image/png");

        assert_eq!(inline.mime_type, "image/png");
        assert!(!inline.data.is_empty());

        let decoded = inline.decode().unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_serialization() {
        let request = GenerateContentRequest::text("Hello");
        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("contents"));
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
    }
}
