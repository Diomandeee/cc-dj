//! DJ Gesture Recognizer - Maps gestures to DJ commands.
//!
//! Provides basic gesture-to-command mapping for the DJ agent.
//! The basic recognizer uses the built-in `GestureRecognizer` from this crate.

use std::collections::HashMap;
use std::sync::Arc;

use cc_dj_types::DJConfig;

// ============================================================================
// GESTURE COMMAND MAPPING
// ============================================================================

/// Mapping from gesture label ID to DJ command ID.
///
/// Use this to configure which gestures trigger which DJ actions.
#[derive(Debug, Clone, Default)]
pub struct GestureCommandMapping {
    /// Maps gesture label ID to command ID string.
    pub mappings: HashMap<u32, String>,
}

impl GestureCommandMapping {
    /// Creates a new empty mapping.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a mapping from gesture to command.
    pub fn add(&mut self, gesture_id: u32, command_id: impl Into<String>) {
        self.mappings.insert(gesture_id, command_id.into());
    }

    /// Gets the command ID for a gesture.
    #[must_use]
    pub fn get_command(&self, gesture_id: u32) -> Option<&str> {
        self.mappings.get(&gesture_id).map(|s| s.as_str())
    }

    /// Returns the number of mappings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Returns true if there are no mappings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Creates a default mapping for common hand gestures.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut mapping = Self::new();

        mapping.add(0, "swipe_left_action");
        mapping.add(1, "swipe_right_action");
        mapping.add(2, "swipe_up_action");
        mapping.add(3, "swipe_down_action");
        mapping.add(4, "circle_cw_action");
        mapping.add(5, "circle_ccw_action");
        mapping.add(6, "flick_action");
        mapping.add(7, "shake_action");
        mapping.add(8, "punch_action");
        mapping.add(9, "raise_action");

        mapping
    }
}

// ============================================================================
// DJ GESTURE RECOGNIZER (basic implementation)
// ============================================================================

/// Basic DJ gesture recognizer.
///
/// Maps recognized gestures to DJ commands using `GestureCommandMapping`.
pub struct DJGestureRecognizer {
    #[allow(dead_code)]
    config: Arc<DJConfig>,
}

impl DJGestureRecognizer {
    /// Creates a new recognizer.
    #[must_use]
    pub fn new(config: Arc<DJConfig>) -> Self {
        Self { config }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gesture_command_mapping() {
        let mapping = GestureCommandMapping::with_defaults();
        assert!(!mapping.is_empty());
        assert!(mapping.get_command(0).is_some());
        assert_eq!(mapping.get_command(0), Some("swipe_left_action"));
    }

    #[test]
    fn test_mapping_operations() {
        let mut mapping = GestureCommandMapping::new();
        assert!(mapping.is_empty());

        mapping.add(42, "test_command");
        assert_eq!(mapping.len(), 1);
        assert_eq!(mapping.get_command(42), Some("test_command"));
        assert_eq!(mapping.get_command(99), None);
    }

    #[test]
    fn test_dj_recognizer_creation() {
        let config = Arc::new(DJConfig::default());
        let _recognizer = DJGestureRecognizer::new(config);
    }
}
