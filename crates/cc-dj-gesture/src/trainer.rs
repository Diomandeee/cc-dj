//! Gesture trainer for learning custom gestures.

use crate::database::GestureDatabase;
use crate::types::{GestureType, MotionDataPoint, RecordedGesture};
use cc_dj_types::Result;
use std::time::Instant;

/// State of the gesture trainer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainerState {
    /// Idle, not recording.
    Idle,
    /// Recording a gesture.
    Recording,
    /// Processing recorded data.
    Processing,
}

/// Gesture trainer for learning custom gestures.
#[derive(Debug)]
pub struct GestureTrainer {
    /// Current state.
    state: TrainerState,
    /// Recording buffer.
    recording_buffer: Vec<MotionDataPoint>,
    /// Current gesture being trained.
    current_gesture: Option<GestureType>,
    /// Recording start time.
    recording_start: Option<Instant>,
    /// Maximum recording duration (ms).
    max_duration_ms: u64,
    /// Minimum recording duration (ms).
    min_duration_ms: u64,
}

impl GestureTrainer {
    /// Creates a new gesture trainer.
    pub fn new() -> Self {
        Self {
            state: TrainerState::Idle,
            recording_buffer: Vec::new(),
            current_gesture: None,
            recording_start: None,
            max_duration_ms: 5000,
            min_duration_ms: 200,
        }
    }

    /// Sets the maximum recording duration.
    pub fn with_max_duration(mut self, ms: u64) -> Self {
        self.max_duration_ms = ms;
        self
    }

    /// Sets the minimum recording duration.
    pub fn with_min_duration(mut self, ms: u64) -> Self {
        self.min_duration_ms = ms;
        self
    }

    /// Starts recording a gesture.
    pub fn start_recording(&mut self, gesture_type: GestureType) -> Result<()> {
        if self.state != TrainerState::Idle {
            return Err(cc_dj_types::DJError::gesture("Already recording"));
        }

        self.state = TrainerState::Recording;
        self.current_gesture = Some(gesture_type);
        self.recording_buffer.clear();
        self.recording_start = Some(Instant::now());

        tracing::info!("Started recording gesture: {:?}", gesture_type);
        Ok(())
    }

    /// Adds a motion data point during recording.
    pub fn add_data(&mut self, data: MotionDataPoint) -> Result<()> {
        if self.state != TrainerState::Recording {
            return Err(cc_dj_types::DJError::gesture("Not recording"));
        }

        // Check max duration
        if let Some(start) = self.recording_start {
            if start.elapsed().as_millis() as u64 > self.max_duration_ms {
                tracing::warn!("Recording exceeded max duration, stopping");
                return self.stop_recording().map(|_| ());
            }
        }

        self.recording_buffer.push(data);
        Ok(())
    }

    /// Stops recording and returns the recorded gesture.
    pub fn stop_recording(&mut self) -> Result<RecordedGesture> {
        if self.state != TrainerState::Recording {
            return Err(cc_dj_types::DJError::gesture("Not recording"));
        }

        let gesture_type = self.current_gesture.ok_or_else(|| {
            cc_dj_types::DJError::gesture("No gesture type set")
        })?;

        // Check min duration
        if let Some(start) = self.recording_start {
            if (start.elapsed().as_millis() as u64) < self.min_duration_ms {
                self.state = TrainerState::Idle;
                return Err(cc_dj_types::DJError::gesture("Recording too short"));
            }
        }

        let data_points = std::mem::take(&mut self.recording_buffer);
        let gesture = RecordedGesture::new(gesture_type, data_points);

        self.state = TrainerState::Idle;
        self.current_gesture = None;
        self.recording_start = None;

        tracing::info!("Recorded gesture with {} data points", gesture.len());
        Ok(gesture)
    }

    /// Cancels the current recording.
    pub fn cancel_recording(&mut self) {
        self.state = TrainerState::Idle;
        self.current_gesture = None;
        self.recording_buffer.clear();
        self.recording_start = None;
        tracing::info!("Recording cancelled");
    }

    /// Returns the current state.
    pub fn state(&self) -> TrainerState {
        self.state
    }

    /// Returns the current recording duration in milliseconds.
    pub fn recording_duration_ms(&self) -> u64 {
        self.recording_start
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    /// Returns the number of data points recorded.
    pub fn recorded_points(&self) -> usize {
        self.recording_buffer.len()
    }

    /// Saves a recorded gesture to a database.
    pub fn save_to_database(gesture: RecordedGesture, database: &mut GestureDatabase) {
        database.add(gesture);
    }
}

impl Default for GestureTrainer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trainer_lifecycle() {
        let mut trainer = GestureTrainer::new()
            .with_min_duration(0);

        assert_eq!(trainer.state(), TrainerState::Idle);

        trainer.start_recording(GestureType::SwipeLeft).unwrap();
        assert_eq!(trainer.state(), TrainerState::Recording);

        trainer.add_data(MotionDataPoint::new(0, [0.0; 3], [0.0; 3])).unwrap();
        trainer.add_data(MotionDataPoint::new(100, [1.0; 3], [0.0; 3])).unwrap();

        let gesture = trainer.stop_recording().unwrap();
        assert_eq!(gesture.gesture_type, GestureType::SwipeLeft);
        assert_eq!(gesture.len(), 2);
        assert_eq!(trainer.state(), TrainerState::Idle);
    }

    #[test]
    fn test_trainer_cancel() {
        let mut trainer = GestureTrainer::new();

        trainer.start_recording(GestureType::Circle).unwrap();
        trainer.cancel_recording();

        assert_eq!(trainer.state(), TrainerState::Idle);
        assert_eq!(trainer.recorded_points(), 0);
    }
}

