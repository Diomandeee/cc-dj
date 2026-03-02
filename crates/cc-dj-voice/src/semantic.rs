//! Semantic matching for voice commands.

use std::collections::HashMap;

/// Semantic matcher using embeddings for command matching.
#[derive(Debug, Default)]
pub struct SemanticMatcher {
    /// Phrase embeddings.
    embeddings: HashMap<String, Vec<f32>>,
    /// Embedding dimension.
    dimension: usize,
}

impl SemanticMatcher {
    /// Creates a new semantic matcher.
    pub fn new(dimension: usize) -> Self {
        Self {
            embeddings: HashMap::new(),
            dimension,
        }
    }

    /// Adds an embedding for a phrase.
    pub fn add_embedding(&mut self, phrase: impl Into<String>, embedding: Vec<f32>) {
        debug_assert_eq!(embedding.len(), self.dimension);
        self.embeddings.insert(phrase.into(), embedding);
    }

    /// Finds the most similar phrase to the given embedding.
    pub fn find_similar(&self, embedding: &[f32], threshold: f32) -> Option<(&str, f32)> {
        let mut best_match: Option<(&str, f32)> = None;

        for (phrase, stored) in &self.embeddings {
            let similarity = cosine_similarity(embedding, stored);
            if similarity >= threshold {
                if best_match.map_or(true, |(_, s)| similarity > s) {
                    best_match = Some((phrase.as_str(), similarity));
                }
            }
        }

        best_match
    }

    /// Returns the number of indexed phrases.
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }
}

/// Computes cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.001);
    }

    #[test]
    fn test_semantic_matcher() {
        let mut matcher = SemanticMatcher::new(3);
        matcher.add_embedding("play", vec![1.0, 0.0, 0.0]);
        matcher.add_embedding("pause", vec![0.0, 1.0, 0.0]);

        let query = vec![0.9, 0.1, 0.0];
        let result = matcher.find_similar(&query, 0.8);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, "play");
    }
}

