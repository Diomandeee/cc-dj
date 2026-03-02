//! Command orbiter - embedding-based command retrieval.

use cc_dj_types::{Command, CommandCatalog};
use std::collections::HashMap;

/// Stability tracking state for embedding-based retrieval.
#[derive(Debug, Default)]
struct StabilityTracker {
    /// Recent similarity scores per command ID.
    history: HashMap<String, Vec<f32>>,
    /// Maximum history length per command.
    max_history: usize,
    /// Last matched command ID.
    last_match: Option<String>,
    /// Consecutive match count for the last command.
    consecutive_count: u32,
}

impl StabilityTracker {
    fn new(max_history: usize) -> Self {
        Self {
            history: HashMap::new(),
            max_history,
            last_match: None,
            consecutive_count: 0,
        }
    }

    /// Records a similarity score for a command.
    fn record(&mut self, cmd_id: &str, similarity: f32) {
        let history = self.history.entry(cmd_id.to_string()).or_default();
        history.push(similarity);
        if history.len() > self.max_history {
            history.remove(0);
        }
    }

    /// Updates the match state and returns true if stable.
    fn update_match(&mut self, cmd_id: Option<&str>, stability_threshold: f32) -> bool {
        match cmd_id {
            Some(id) => {
                if self.last_match.as_deref() == Some(id) {
                    self.consecutive_count += 1;
                } else {
                    self.last_match = Some(id.to_string());
                    self.consecutive_count = 1;
                }
                // Stable if we have 3+ consecutive matches with high confidence
                self.consecutive_count >= 3
                    && self
                        .history
                        .get(id)
                        .is_some_and(|h| h.last().is_some_and(|&s| s >= stability_threshold))
            }
            None => {
                self.consecutive_count = 0;
                false
            }
        }
    }

    /// Resets the tracker state.
    fn reset(&mut self) {
        self.history.clear();
        self.last_match = None;
        self.consecutive_count = 0;
    }
}

/// Command orbiter for embedding-based retrieval.
///
/// Uses semantic embeddings to find the most likely command
/// from partial or noisy voice input.
#[derive(Debug, Default)]
pub struct CommandOrbiter {
    /// Command catalog.
    catalog: CommandCatalog,
    /// Embedding cache (command ID -> embedding).
    embeddings: HashMap<String, Vec<f32>>,
    /// Stability threshold for committing to a command.
    stability_threshold: f32,
    /// Stability tracker for consecutive matches.
    stability_tracker: StabilityTracker,
}

impl CommandOrbiter {
    /// Creates a new command orbiter.
    pub fn new() -> Self {
        Self {
            catalog: CommandCatalog::new(),
            embeddings: HashMap::new(),
            stability_threshold: 0.85,
            stability_tracker: StabilityTracker::new(10),
        }
    }

    /// Creates an orbiter with a pre-loaded catalog.
    pub fn with_catalog(catalog: CommandCatalog) -> Self {
        Self {
            catalog,
            embeddings: HashMap::new(),
            stability_threshold: 0.85,
            stability_tracker: StabilityTracker::new(10),
        }
    }

    /// Sets the stability threshold.
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.stability_threshold = threshold;
        self
    }

    /// Loads the command catalog from YAML.
    pub fn load_catalog(&mut self, yaml: &str) -> cc_dj_types::Result<()> {
        self.catalog = CommandCatalog::from_yaml(yaml)?;
        // Pre-compute embeddings for all commands would go here
        // For now, we use text-based similarity
        Ok(())
    }

    /// Indexes a command with its embedding.
    pub fn index_command(&mut self, cmd_id: &str, embedding: Vec<f32>) {
        self.embeddings.insert(cmd_id.to_string(), embedding);
    }

    /// Finds the best matching command for the given text.
    pub fn find_command(&self, text: &str) -> Option<&Command> {
        let matches = self.catalog.find_matching(text);
        matches.first().copied()
    }

    /// Computes cosine similarity between two embeddings.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Updates with a new embedding and checks for stability.
    ///
    /// Returns the matched command if the input has stabilized above the threshold.
    pub fn update_embedding(&mut self, embedding: &[f32]) -> Option<&Command> {
        if embedding.is_empty() {
            return None;
        }

        // Find best matching command by embedding similarity
        let mut best_match: Option<(String, f32)> = None;

        for (cmd_id, cmd_embedding) in &self.embeddings {
            let similarity = Self::cosine_similarity(embedding, cmd_embedding);
            self.stability_tracker.record(cmd_id, similarity);

            if similarity > best_match.as_ref().map_or(0.0, |b| b.1) {
                best_match = Some((cmd_id.clone(), similarity));
            }
        }

        // Check stability
        let matched_id = best_match
            .filter(|(_, sim)| *sim >= self.stability_threshold)
            .map(|(id, _)| id);

        let is_stable = self
            .stability_tracker
            .update_match(matched_id.as_deref(), self.stability_threshold);

        if is_stable {
            if let Some(ref id) = matched_id {
                // Reset tracker after successful match
                self.stability_tracker.reset();
                return self.catalog.get(id);
            }
        }

        None
    }

    /// Resets the stability tracker.
    pub fn reset(&mut self) {
        self.stability_tracker.reset();
    }

    /// Returns the number of indexed commands.
    pub fn command_count(&self) -> usize {
        self.catalog.len()
    }

    /// Returns the number of indexed embeddings.
    pub fn embedding_count(&self) -> usize {
        self.embeddings.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orbiter_creation() {
        let orbiter = CommandOrbiter::new();
        assert_eq!(orbiter.command_count(), 0);
        assert_eq!(orbiter.embedding_count(), 0);
    }

    #[test]
    fn test_orbiter_with_catalog() {
        let yaml = r#"
commands:
  - id: "1"
    canonical: Play
    synonyms: [start, go]
    category: transport
    action_type: play_pause
    shortcut: Z
"#;
        let catalog = CommandCatalog::from_yaml(yaml).unwrap();
        let orbiter = CommandOrbiter::with_catalog(catalog);
        assert_eq!(orbiter.command_count(), 1);
    }

    #[test]
    fn test_cosine_similarity() {
        // Identical vectors should have similarity 1.0
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((CommandOrbiter::cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        // Orthogonal vectors should have similarity 0.0
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(CommandOrbiter::cosine_similarity(&a, &b).abs() < 0.001);
    }

    #[test]
    fn test_embedding_indexing() {
        let mut orbiter = CommandOrbiter::new();
        orbiter.index_command("play", vec![1.0, 0.0, 0.0]);
        orbiter.index_command("stop", vec![0.0, 1.0, 0.0]);
        assert_eq!(orbiter.embedding_count(), 2);
    }
}
