//! Gesture recognizer for real-time gesture detection.

use crate::database::GestureDatabase;
use crate::types::{GestureType, MotionDataPoint};
use std::collections::VecDeque;

/// Configuration for gesture recognition.
#[derive(Debug, Clone)]
pub struct RecognizerConfig {
    /// Size of the motion data buffer.
    pub buffer_size: usize,
    /// Minimum confidence threshold for recognition.
    pub confidence_threshold: f32,
    /// Minimum samples required for recognition.
    pub min_samples: usize,
}

impl Default for RecognizerConfig {
    fn default() -> Self {
        Self {
            buffer_size: 100,
            confidence_threshold: 0.7,
            min_samples: 10,
        }
    }
}

/// Result of gesture recognition.
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    /// Recognized gesture type.
    pub gesture_type: GestureType,
    /// Confidence score (0.0-1.0).
    pub confidence: f32,
    /// Duration of the gesture in milliseconds.
    pub duration_ms: u64,
}

/// Real-time gesture recognizer.
#[derive(Debug)]
pub struct GestureRecognizer {
    /// Configuration.
    config: RecognizerConfig,
    /// Motion data buffer.
    buffer: VecDeque<MotionDataPoint>,
    /// Reference gesture database.
    database: GestureDatabase,
    /// Last recognized gesture.
    last_recognition: Option<RecognitionResult>,
}

impl GestureRecognizer {
    /// Creates a new gesture recognizer.
    pub fn new(config: RecognizerConfig, database: GestureDatabase) -> Self {
        Self {
            buffer: VecDeque::with_capacity(config.buffer_size),
            config,
            database,
            last_recognition: None,
        }
    }

    /// Creates a recognizer with default configuration.
    pub fn with_database(database: GestureDatabase) -> Self {
        Self::new(RecognizerConfig::default(), database)
    }

    /// Processes a new motion data point.
    pub fn process(&mut self, data: MotionDataPoint) -> Option<RecognitionResult> {
        // Add to buffer
        self.buffer.push_back(data);

        // Maintain buffer size
        while self.buffer.len() > self.config.buffer_size {
            self.buffer.pop_front();
        }

        // Need minimum samples for recognition
        if self.buffer.len() < self.config.min_samples {
            return None;
        }

        // Try to recognize gesture
        self.recognize()
    }

    /// Attempts to recognize a gesture from the current buffer.
    fn recognize(&mut self) -> Option<RecognitionResult> {
        let buffer_data: Vec<_> = self.buffer.iter().cloned().collect();

        let mut best_match: Option<(GestureType, f32)> = None;

        // Compare against all gesture types in database
        for gesture_type in self.database.gesture_types() {
            let samples = self.database.get(gesture_type);
            if samples.is_empty() {
                continue;
            }

            // Calculate average similarity to all samples
            let mut total_similarity = 0.0;
            for sample in samples {
                let similarity = self.calculate_similarity(&buffer_data, &sample.data_points);
                total_similarity += similarity;
            }
            let avg_similarity = total_similarity / samples.len() as f32;

            if avg_similarity >= self.config.confidence_threshold
                && best_match.is_none_or(|(_, s)| avg_similarity > s)
            {
                best_match = Some((gesture_type, avg_similarity));
            }
        }

        best_match.map(|(gesture_type, confidence)| {
            let duration_ms = self.buffer.back().map(|p| p.timestamp_ms).unwrap_or(0)
                - self.buffer.front().map(|p| p.timestamp_ms).unwrap_or(0);

            let result = RecognitionResult {
                gesture_type,
                confidence,
                duration_ms,
            };

            self.last_recognition = Some(result.clone());

            // Clear buffer after recognition
            self.buffer.clear();

            result
        })
    }

    /// Calculates similarity between two motion sequences.
    fn calculate_similarity(&self, a: &[MotionDataPoint], b: &[MotionDataPoint]) -> f32 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }

        // Simple DTW-like similarity based on accelerometer data
        let len = a.len().min(b.len());
        let mut total_distance = 0.0;

        for i in 0..len {
            let a_mag = a[i].acceleration_magnitude();
            let b_mag = b[i].acceleration_magnitude();
            total_distance += (a_mag - b_mag).abs();
        }

        let avg_distance = total_distance / len as f32;

        // Convert distance to similarity (inverse relationship)
        1.0 / (1.0 + avg_distance)
    }

    /// Returns the last recognition result.
    pub fn last_recognition(&self) -> Option<&RecognitionResult> {
        self.last_recognition.as_ref()
    }

    /// Clears the motion buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.last_recognition = None;
    }

    /// Returns the current buffer size.
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recognizer_creation() {
        let db = GestureDatabase::new();
        let recognizer = GestureRecognizer::with_database(db);
        assert_eq!(recognizer.buffer_len(), 0);
    }

    #[test]
    fn test_recognizer_buffer() {
        let db = GestureDatabase::new();
        let mut recognizer = GestureRecognizer::with_database(db);

        for i in 0..50 {
            recognizer.process(MotionDataPoint::new(i * 10, [0.0; 3], [0.0; 3]));
        }

        assert_eq!(recognizer.buffer_len(), 50);
    }
}
