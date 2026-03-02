//! Gesture database for storing and retrieving learned gestures.

use crate::types::{GestureType, RecordedGesture};
use cc_dj_types::Result;
use std::collections::HashMap;
use std::path::Path;

/// Database for storing gesture samples.
#[derive(Debug, Default)]
pub struct GestureDatabase {
    /// Gestures indexed by type.
    gestures: HashMap<GestureType, Vec<RecordedGesture>>,
    /// Path to persistent storage.
    storage_path: Option<std::path::PathBuf>,
}

impl GestureDatabase {
    /// Creates a new empty database.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a database with persistent storage.
    pub fn with_storage(path: impl AsRef<Path>) -> Self {
        Self {
            gestures: HashMap::new(),
            storage_path: Some(path.as_ref().to_path_buf()),
        }
    }

    /// Adds a gesture sample to the database.
    pub fn add(&mut self, gesture: RecordedGesture) {
        self.gestures
            .entry(gesture.gesture_type)
            .or_default()
            .push(gesture);
    }

    /// Gets all samples for a gesture type.
    pub fn get(&self, gesture_type: GestureType) -> &[RecordedGesture] {
        self.gestures
            .get(&gesture_type)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns all gesture types in the database.
    pub fn gesture_types(&self) -> Vec<GestureType> {
        self.gestures.keys().copied().collect()
    }

    /// Returns the total number of samples.
    pub fn sample_count(&self) -> usize {
        self.gestures.values().map(|v| v.len()).sum()
    }

    /// Returns the number of samples for a gesture type.
    pub fn sample_count_for(&self, gesture_type: GestureType) -> usize {
        self.gestures.get(&gesture_type).map(|v| v.len()).unwrap_or(0)
    }

    /// Saves the database to storage.
    pub fn save(&self) -> Result<()> {
        if let Some(path) = &self.storage_path {
            let json = serde_json::to_string_pretty(&self.gestures)?;
            std::fs::write(path, json)?;
        }
        Ok(())
    }

    /// Loads the database from storage.
    pub fn load(&mut self) -> Result<()> {
        if let Some(path) = &self.storage_path {
            if path.exists() {
                let json = std::fs::read_to_string(path)?;
                self.gestures = serde_json::from_str(&json)?;
            }
        }
        Ok(())
    }

    /// Clears all samples for a gesture type.
    pub fn clear(&mut self, gesture_type: GestureType) {
        self.gestures.remove(&gesture_type);
    }

    /// Clears all samples.
    pub fn clear_all(&mut self) {
        self.gestures.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MotionDataPoint;

    #[test]
    fn test_database_add_get() {
        let mut db = GestureDatabase::new();
        
        let gesture = RecordedGesture::new(
            GestureType::SwipeLeft,
            vec![MotionDataPoint::new(0, [0.0; 3], [0.0; 3])],
        );
        
        db.add(gesture);
        
        assert_eq!(db.sample_count(), 1);
        assert_eq!(db.sample_count_for(GestureType::SwipeLeft), 1);
        assert_eq!(db.get(GestureType::SwipeLeft).len(), 1);
    }

    #[test]
    fn test_database_gesture_types() {
        let mut db = GestureDatabase::new();
        
        db.add(RecordedGesture::new(GestureType::SwipeLeft, vec![]));
        db.add(RecordedGesture::new(GestureType::Circle, vec![]));
        
        let types = db.gesture_types();
        assert_eq!(types.len(), 2);
    }
}

