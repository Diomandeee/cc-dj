//! Action scheduler for beat-quantized execution.

use cc_dj_types::Action;
use std::collections::VecDeque;

/// A scheduled action waiting for execution.
#[derive(Debug, Clone)]
struct ScheduledAction {
    /// The action to execute.
    action: Action,
    /// Target beat for execution.
    target_beat: f64,
}

/// Scheduler for beat-quantized action execution.
#[derive(Debug)]
pub struct ActionScheduler {
    /// Queue of scheduled actions.
    queue: VecDeque<ScheduledAction>,
    /// Quantization window in degrees.
    quant_window_deg: f64,
}

impl ActionScheduler {
    /// Creates a new action scheduler.
    pub fn new(quant_window_deg: f64) -> Self {
        Self {
            queue: VecDeque::new(),
            quant_window_deg,
        }
    }

    /// Schedules an action for the next beat.
    pub fn schedule(&mut self, action: Action, current_beat: f64) {
        let target_beat = current_beat.ceil();
        
        tracing::debug!(
            "Scheduling action {} for beat {} (current: {})",
            action.name,
            target_beat,
            current_beat
        );

        self.queue.push_back(ScheduledAction {
            action,
            target_beat,
        });
    }

    /// Schedules an action for a specific beat.
    pub fn schedule_at(&mut self, action: Action, target_beat: f64) {
        self.queue.push_back(ScheduledAction {
            action,
            target_beat,
        });
    }

    /// Polls for actions ready to execute.
    pub fn poll(&mut self, current_beat: f64) -> Option<Action> {
        // Check if front of queue is ready
        if let Some(scheduled) = self.queue.front() {
            let beat_diff = current_beat - scheduled.target_beat;
            let phase_deg = beat_diff.fract() * 360.0;
            
            // Check if we're within the quantization window
            if beat_diff >= 0.0 && phase_deg.abs() <= self.quant_window_deg {
                return self.queue.pop_front().map(|s| s.action);
            }
            
            // Remove stale actions (missed their window)
            if beat_diff > 1.0 {
                tracing::warn!(
                    "Dropping stale action {} (target: {}, current: {})",
                    scheduled.action.name,
                    scheduled.target_beat,
                    current_beat
                );
                self.queue.pop_front();
            }
        }

        None
    }

    /// Returns the number of scheduled actions.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Clears all scheduled actions.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Returns the next scheduled beat.
    pub fn next_beat(&self) -> Option<f64> {
        self.queue.front().map(|s| s.target_beat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_dj_types::Tier;

    #[test]
    fn test_scheduler() {
        let mut scheduler = ActionScheduler::new(15.0);
        
        let action = Action::new("PLAY_A", Tier::Transport);
        scheduler.schedule(action.clone(), 3.5);
        
        assert_eq!(scheduler.len(), 1);
        assert_eq!(scheduler.next_beat(), Some(4.0));
    }

    #[test]
    fn test_poll() {
        let mut scheduler = ActionScheduler::new(15.0);
        
        let action = Action::new("PLAY_A", Tier::Transport);
        scheduler.schedule_at(action.clone(), 4.0);
        
        // Not ready yet
        assert!(scheduler.poll(3.5).is_none());
        
        // Ready now
        let polled = scheduler.poll(4.0);
        assert!(polled.is_some());
        assert_eq!(polled.unwrap().name, "PLAY_A");
        
        // Queue should be empty
        assert!(scheduler.is_empty());
    }
}

