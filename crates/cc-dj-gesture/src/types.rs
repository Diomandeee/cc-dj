//! Gesture types and motion data structures.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Types of gestures that can be recognized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GestureType {
    /// Hand swipe left.
    SwipeLeft,
    /// Hand swipe right.
    SwipeRight,
    /// Hand swipe up.
    SwipeUp,
    /// Hand swipe down.
    SwipeDown,
    /// Hand raised.
    HandRaise,
    /// Hand lowered.
    HandLower,
    /// Circular motion.
    Circle,
    /// Pointing gesture.
    Point,
    /// Fist/grab gesture.
    Fist,
    /// Open palm.
    OpenPalm,
    /// Two-finger pinch.
    Pinch,
    /// Custom/learned gesture.
    Custom(u32),
}

impl GestureType {
    /// Returns the gesture name.
    pub fn name(&self) -> String {
        match self {
            Self::SwipeLeft => "swipe_left".to_string(),
            Self::SwipeRight => "swipe_right".to_string(),
            Self::SwipeUp => "swipe_up".to_string(),
            Self::SwipeDown => "swipe_down".to_string(),
            Self::HandRaise => "hand_raise".to_string(),
            Self::HandLower => "hand_lower".to_string(),
            Self::Circle => "circle".to_string(),
            Self::Point => "point".to_string(),
            Self::Fist => "fist".to_string(),
            Self::OpenPalm => "open_palm".to_string(),
            Self::Pinch => "pinch".to_string(),
            Self::Custom(id) => format!("custom_{}", id),
        }
    }

    /// Creates from a name string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "swipe_left" => Some(Self::SwipeLeft),
            "swipe_right" => Some(Self::SwipeRight),
            "swipe_up" => Some(Self::SwipeUp),
            "swipe_down" => Some(Self::SwipeDown),
            "hand_raise" => Some(Self::HandRaise),
            "hand_lower" => Some(Self::HandLower),
            "circle" => Some(Self::Circle),
            "point" => Some(Self::Point),
            "fist" => Some(Self::Fist),
            "open_palm" => Some(Self::OpenPalm),
            "pinch" => Some(Self::Pinch),
            s if s.starts_with("custom_") => {
                s.strip_prefix("custom_")?.parse().ok().map(Self::Custom)
            }
            _ => None,
        }
    }
}

/// A single motion data point from sensors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionDataPoint {
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,

    /// Accelerometer data (x, y, z) in m/s².
    pub accelerometer: [f32; 3],

    /// Gyroscope data (x, y, z) in rad/s.
    pub gyroscope: [f32; 3],

    /// Magnetometer data (x, y, z) in μT (optional).
    #[serde(default)]
    pub magnetometer: Option<[f32; 3]>,

    /// Quaternion orientation (w, x, y, z) (optional).
    #[serde(default)]
    pub quaternion: Option<[f32; 4]>,

    /// Joint positions from pose estimation (optional).
    #[serde(default)]
    pub joints: Option<JointPositions>,
}

impl MotionDataPoint {
    /// Creates a new motion data point with accelerometer and gyroscope data.
    pub fn new(timestamp_ms: u64, accelerometer: [f32; 3], gyroscope: [f32; 3]) -> Self {
        Self {
            timestamp_ms,
            accelerometer,
            gyroscope,
            magnetometer: None,
            quaternion: None,
            joints: None,
        }
    }

    /// Returns the acceleration magnitude.
    pub fn acceleration_magnitude(&self) -> f32 {
        let [x, y, z] = self.accelerometer;
        (x * x + y * y + z * z).sqrt()
    }

    /// Returns the angular velocity magnitude.
    pub fn angular_velocity_magnitude(&self) -> f32 {
        let [x, y, z] = self.gyroscope;
        (x * x + y * y + z * z).sqrt()
    }
}

/// Joint positions from pose estimation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JointPositions {
    /// Left wrist position (x, y, z).
    pub left_wrist: Option<[f32; 3]>,
    /// Right wrist position (x, y, z).
    pub right_wrist: Option<[f32; 3]>,
    /// Left elbow position (x, y, z).
    pub left_elbow: Option<[f32; 3]>,
    /// Right elbow position (x, y, z).
    pub right_elbow: Option<[f32; 3]>,
    /// Left shoulder position (x, y, z).
    pub left_shoulder: Option<[f32; 3]>,
    /// Right shoulder position (x, y, z).
    pub right_shoulder: Option<[f32; 3]>,
    /// Head position (x, y, z).
    pub head: Option<[f32; 3]>,
}

/// A recorded gesture sample for training.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedGesture {
    /// Gesture type/label.
    pub gesture_type: GestureType,
    /// Sequence of motion data points.
    pub data_points: Vec<MotionDataPoint>,
    /// Recording timestamp.
    pub recorded_at: String,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Optional metadata.
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

impl RecordedGesture {
    /// Creates a new recorded gesture.
    pub fn new(gesture_type: GestureType, data_points: Vec<MotionDataPoint>) -> Self {
        let duration_ms = data_points
            .last()
            .map(|p| p.timestamp_ms)
            .unwrap_or(0)
            .saturating_sub(data_points.first().map(|p| p.timestamp_ms).unwrap_or(0));

        Self {
            gesture_type,
            data_points,
            recorded_at: chrono::Utc::now().to_rfc3339(),
            duration_ms,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Returns the number of data points.
    pub fn len(&self) -> usize {
        self.data_points.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.data_points.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gesture_type_roundtrip() {
        for gesture in [
            GestureType::SwipeLeft,
            GestureType::Circle,
            GestureType::Custom(42),
        ] {
            let name = gesture.name();
            let parsed = GestureType::from_name(&name);
            assert_eq!(parsed, Some(gesture));
        }
    }

    #[test]
    fn test_motion_data_point() {
        let point = MotionDataPoint::new(1000, [0.0, 0.0, 9.8], [0.1, 0.0, 0.0]);
        assert!((point.acceleration_magnitude() - 9.8).abs() < 0.01);
    }
}

