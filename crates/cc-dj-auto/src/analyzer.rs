//! Track analyzer for extracting DJ-relevant features.
//!
//! This module provides track analysis functionality for DJ applications.
//! It supports multiple analysis sources:
//!
//! - **JSON sidecar files**: Load pre-computed analysis from `track.mp3.analysis.json`
//! - **Cache files**: Load from a central analysis cache directory
//! - **External integration**: Can be extended to integrate with DJ software databases
//!
//! Note: This crate does not include audio processing dependencies.
//! For raw audio analysis, use external tools (Essentia, librosa, DJ software)
//! and import the results via JSON.

use cc_dj_types::{DJError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Source of track analysis data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisSource {
    /// Analysis loaded from JSON sidecar file.
    JsonSidecar,
    /// Analysis loaded from cache directory.
    Cache,
    /// Analysis from external DJ software (Rekordbox, Serato, etc.).
    DjSoftware,
    /// Placeholder/estimated values (not actual analysis).
    #[default]
    Placeholder,
}

/// Analysis results for a track.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackAnalysis {
    /// Track BPM.
    pub bpm: f64,
    /// Key (Camelot notation).
    pub key: Option<String>,
    /// Energy level (1-10).
    pub energy: u8,
    /// Danceability score (0.0-1.0).
    pub danceability: f32,
    /// Section markers (in seconds).
    pub sections: Vec<SectionMarker>,
    /// Beat grid offsets (in seconds).
    pub beat_grid: Vec<f64>,
    /// Peak/drop locations (in seconds).
    pub peaks: Vec<f64>,
    /// Recommended intro/outro points.
    pub mix_points: MixPoints,
    /// Source of this analysis data.
    #[serde(default)]
    pub source: AnalysisSource,
}

/// A section marker in a track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionMarker {
    /// Start time in seconds.
    pub start_secs: f64,
    /// End time in seconds.
    pub end_secs: f64,
    /// Section type (intro, verse, chorus, breakdown, drop, outro).
    pub section_type: SectionType,
    /// Energy level for this section.
    pub energy: u8,
}

/// Type of track section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionType {
    /// Introduction.
    Intro,
    /// Verse section.
    Verse,
    /// Chorus/hook.
    Chorus,
    /// Breakdown (low energy).
    Breakdown,
    /// Drop (high energy peak).
    Drop,
    /// Bridge section.
    Bridge,
    /// Outro.
    Outro,
    /// Unknown section type.
    Unknown,
}

/// Recommended mix points for a track.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MixPoints {
    /// Best point to start mixing in (seconds).
    pub mix_in: Option<f64>,
    /// Best point to start mixing out (seconds).
    pub mix_out: Option<f64>,
    /// Safe loop points (seconds).
    pub loop_points: Vec<f64>,
}

/// Track analyzer for DJ preparation.
///
/// Supports loading pre-computed analysis from JSON files and provides
/// harmonic compatibility checking via the Camelot wheel.
pub struct TrackAnalyzer {
    /// Minimum BPM for analysis.
    min_bpm: f64,
    /// Maximum BPM for analysis.
    max_bpm: f64,
    /// Optional cache directory for analysis files.
    cache_dir: Option<String>,
    /// Whether to use placeholder when analysis not found.
    use_placeholder: bool,
}

impl TrackAnalyzer {
    /// Creates a new track analyzer.
    pub fn new() -> Self {
        Self {
            min_bpm: 60.0,
            max_bpm: 200.0,
            cache_dir: None,
            use_placeholder: true,
        }
    }

    /// Sets the BPM range for analysis.
    pub fn with_bpm_range(mut self, min: f64, max: f64) -> Self {
        self.min_bpm = min;
        self.max_bpm = max;
        self
    }

    /// Sets the cache directory for analysis files.
    pub fn with_cache_dir(mut self, dir: impl Into<String>) -> Self {
        self.cache_dir = Some(dir.into());
        self
    }

    /// Sets whether to use placeholder values when analysis is not found.
    pub fn with_placeholder(mut self, enabled: bool) -> Self {
        self.use_placeholder = enabled;
        self
    }

    /// Analyzes a track file by loading pre-computed analysis.
    ///
    /// This method attempts to load analysis in the following order:
    /// 1. JSON sidecar file (e.g., `track.mp3.analysis.json`)
    /// 2. Cache directory file (e.g., `cache/track_hash.json`)
    /// 3. Placeholder values (if enabled)
    ///
    /// For raw audio analysis, use external tools and save results to JSON.
    pub async fn analyze(&self, path: &str) -> Result<TrackAnalysis> {
        // Try JSON sidecar file first
        let sidecar_path = format!("{}.analysis.json", path);
        if let Ok(analysis) = self.load_from_json(&sidecar_path, AnalysisSource::JsonSidecar) {
            tracing::debug!("Loaded analysis from sidecar: {}", sidecar_path);
            return Ok(analysis);
        }

        // Try cache directory
        if let Some(ref cache_dir) = self.cache_dir {
            let filename = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let cache_path = format!("{}/{}.analysis.json", cache_dir, filename);
            if let Ok(analysis) = self.load_from_json(&cache_path, AnalysisSource::Cache) {
                tracing::debug!("Loaded analysis from cache: {}", cache_path);
                return Ok(analysis);
            }
        }

        // Return placeholder if enabled
        if self.use_placeholder {
            tracing::warn!(
                "No analysis found for '{}', using placeholder values. \
                 For accurate analysis, generate JSON using external tools.",
                path
            );
            return Ok(self.create_placeholder_analysis(path));
        }

        Err(DJError::ConfigError(format!(
            "No analysis found for '{}'. Generate analysis JSON using external tools \
             (Essentia, librosa, or DJ software) and save as '{}.analysis.json'",
            path, path
        )))
    }

    /// Loads analysis from a JSON file.
    fn load_from_json(&self, path: &str, source: AnalysisSource) -> Result<TrackAnalysis> {
        let content = std::fs::read_to_string(path)?;
        let mut analysis: TrackAnalysis = serde_json::from_str(&content)?;
        analysis.source = source;

        // Validate BPM range
        if analysis.bpm < self.min_bpm || analysis.bpm > self.max_bpm {
            tracing::warn!(
                "BPM {} outside expected range [{}, {}], may need adjustment",
                analysis.bpm,
                self.min_bpm,
                self.max_bpm
            );
        }

        Ok(analysis)
    }

    /// Saves analysis to a JSON file.
    pub fn save_analysis(&self, path: &str, analysis: &TrackAnalysis) -> Result<()> {
        let json = serde_json::to_string_pretty(analysis)?;
        std::fs::write(path, json)?;
        tracing::info!("Saved analysis to: {}", path);
        Ok(())
    }

    /// Creates a placeholder analysis for testing/demo purposes.
    fn create_placeholder_analysis(&self, path: &str) -> TrackAnalysis {
        // Generate deterministic placeholder based on path hash
        let hash = path.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
        let bpm_offset = (hash % 40) as f64;
        let key_num = ((hash / 40) % 12) as u8 + 1;
        let key_mode = if hash % 2 == 0 { 'A' } else { 'B' };

        TrackAnalysis {
            bpm: 110.0 + bpm_offset, // 110-150 BPM range
            key: Some(format!("{}{}", key_num, key_mode)),
            energy: ((hash % 8) + 3) as u8, // 3-10 range
            danceability: 0.6 + (hash % 40) as f32 / 100.0,
            sections: vec![
                SectionMarker {
                    start_secs: 0.0,
                    end_secs: 32.0,
                    section_type: SectionType::Intro,
                    energy: 3,
                },
                SectionMarker {
                    start_secs: 32.0,
                    end_secs: 64.0,
                    section_type: SectionType::Breakdown,
                    energy: 5,
                },
                SectionMarker {
                    start_secs: 64.0,
                    end_secs: 128.0,
                    section_type: SectionType::Drop,
                    energy: 9,
                },
                SectionMarker {
                    start_secs: 128.0,
                    end_secs: 180.0,
                    section_type: SectionType::Outro,
                    energy: 4,
                },
            ],
            beat_grid: vec![],
            peaks: vec![64.0, 128.0],
            mix_points: MixPoints {
                mix_in: Some(0.0),
                mix_out: Some(160.0),
                loop_points: vec![32.0, 64.0, 96.0],
            },
            source: AnalysisSource::Placeholder,
        }
    }

    /// Checks if two tracks are harmonically compatible using the Camelot wheel.
    ///
    /// Compatible combinations:
    /// - Same key (e.g., 8A and 8A)
    /// - Adjacent on wheel (e.g., 8A and 7A or 9A)
    /// - Relative major/minor (e.g., 8A and 8B)
    pub fn are_compatible(&self, key_a: &str, key_b: &str) -> bool {
        let parse_camelot = |key: &str| -> Option<(u8, char)> {
            let key = key.trim().to_uppercase();
            if key.len() < 2 {
                return None;
            }
            let num: u8 = key[..key.len() - 1].parse().ok()?;
            let mode = key.chars().last()?;
            if !(1..=12).contains(&num) || (mode != 'A' && mode != 'B') {
                return None;
            }
            Some((num, mode))
        };

        let (num_a, mode_a) = match parse_camelot(key_a) {
            Some(v) => v,
            None => return true, // Unknown key, assume compatible
        };
        let (num_b, mode_b) = match parse_camelot(key_b) {
            Some(v) => v,
            None => return true,
        };

        // Same key
        if num_a == num_b && mode_a == mode_b {
            return true;
        }

        // Adjacent on wheel (±1, wrapping 1-12)
        let adjacent = |a: u8, b: u8| -> bool {
            (a == b + 1) || (b == a + 1) || (a == 1 && b == 12) || (a == 12 && b == 1)
        };
        if adjacent(num_a, num_b) && mode_a == mode_b {
            return true;
        }

        // Same number, different mode (relative major/minor)
        if num_a == num_b {
            return true;
        }

        false
    }

    /// Returns compatible keys for a given key.
    pub fn compatible_keys(&self, key: &str) -> Vec<String> {
        let parse_camelot = |key: &str| -> Option<(u8, char)> {
            let key = key.trim().to_uppercase();
            if key.len() < 2 {
                return None;
            }
            let num: u8 = key[..key.len() - 1].parse().ok()?;
            let mode = key.chars().last()?;
            if !(1..=12).contains(&num) || (mode != 'A' && mode != 'B') {
                return None;
            }
            Some((num, mode))
        };

        let (num, mode) = match parse_camelot(key) {
            Some(v) => v,
            None => return vec![],
        };

        let mut compatible = vec![format!("{}{}", num, mode)]; // Same key

        // Relative major/minor
        let other_mode = if mode == 'A' { 'B' } else { 'A' };
        compatible.push(format!("{}{}", num, other_mode));

        // Adjacent keys (same mode)
        let prev = if num == 1 { 12 } else { num - 1 };
        let next = if num == 12 { 1 } else { num + 1 };
        compatible.push(format!("{}{}", prev, mode));
        compatible.push(format!("{}{}", next, mode));

        compatible
    }
}

impl Default for TrackAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harmonic_compatibility() {
        let analyzer = TrackAnalyzer::new();

        // Same key
        assert!(analyzer.are_compatible("8A", "8A"));
        assert!(analyzer.are_compatible("8a", "8A")); // Case insensitive

        // Adjacent keys
        assert!(analyzer.are_compatible("8A", "7A"));
        assert!(analyzer.are_compatible("8A", "9A"));

        // Wrap around (1 and 12 are adjacent)
        assert!(analyzer.are_compatible("1A", "12A"));
        assert!(analyzer.are_compatible("12B", "1B"));

        // Relative major/minor
        assert!(analyzer.are_compatible("8A", "8B"));

        // Non-compatible
        assert!(!analyzer.are_compatible("8A", "3A"));
        assert!(!analyzer.are_compatible("8A", "5B"));
    }

    #[test]
    fn test_compatible_keys() {
        let analyzer = TrackAnalyzer::new();

        let keys = analyzer.compatible_keys("8A");
        assert!(keys.contains(&"8A".to_string())); // Same
        assert!(keys.contains(&"8B".to_string())); // Relative
        assert!(keys.contains(&"7A".to_string())); // Adjacent -1
        assert!(keys.contains(&"9A".to_string())); // Adjacent +1

        // Test wrap around
        let keys = analyzer.compatible_keys("1A");
        assert!(keys.contains(&"12A".to_string()));
        assert!(keys.contains(&"2A".to_string()));
    }

    #[tokio::test]
    async fn test_analyze_placeholder() {
        let analyzer = TrackAnalyzer::new();
        let analysis = analyzer.analyze("test.mp3").await.unwrap();

        assert!(analysis.bpm > 0.0);
        assert!(analysis.bpm >= 110.0 && analysis.bpm <= 150.0);
        assert!(!analysis.sections.is_empty());
        assert_eq!(analysis.source, AnalysisSource::Placeholder);
    }

    #[tokio::test]
    async fn test_analyze_no_placeholder() {
        let analyzer = TrackAnalyzer::new().with_placeholder(false);
        let result = analyzer.analyze("nonexistent.mp3").await;

        assert!(result.is_err());
    }

    #[test]
    fn test_deterministic_placeholder() {
        let analyzer = TrackAnalyzer::new();

        // Same path should give same placeholder values
        let analysis1 = analyzer.create_placeholder_analysis("test.mp3");
        let analysis2 = analyzer.create_placeholder_analysis("test.mp3");

        assert_eq!(analysis1.bpm, analysis2.bpm);
        assert_eq!(analysis1.key, analysis2.key);
        assert_eq!(analysis1.energy, analysis2.energy);

        // Different paths should give different values
        let analysis3 = analyzer.create_placeholder_analysis("other.mp3");
        // Could be same by chance, but at least one should differ
        let different = analysis1.bpm != analysis3.bpm
            || analysis1.key != analysis3.key
            || analysis1.energy != analysis3.energy;
        assert!(different);
    }

    #[test]
    fn test_analysis_source() {
        let analysis = TrackAnalysis::default();
        assert_eq!(analysis.source, AnalysisSource::Placeholder);
    }
}
