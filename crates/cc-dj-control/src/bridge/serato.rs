//! Serato bridge implementation.

use async_trait::async_trait;
use cc_dj_types::{Action, Result, SoftwareConfig};
use midir::{MidiOutput, MidiOutputConnection};
use std::process::Command;
use std::sync::Mutex;
use tracing::{debug, info, warn};

use super::DJBridge;

/// Bridge for Serato DJ software.
pub struct SeratoBridge {
    /// Software configuration.
    config: Option<SoftwareConfig>,
    /// Whether we're in simulation mode.
    simulation_mode: bool,
    /// Cached MIDI output connection.
    midi_out: Mutex<Option<MidiOutputConnection>>,
}

impl std::fmt::Debug for SeratoBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SeratoBridge")
            .field("config", &self.config)
            .field("simulation_mode", &self.simulation_mode)
            .field("midi_out", &"<MidiOutputConnection>")
            .finish()
    }
}

impl SeratoBridge {
    /// Creates a new Serato bridge.
    pub fn new(config: Option<SoftwareConfig>) -> Self {
        Self {
            config,
            simulation_mode: false,
            midi_out: Mutex::new(None),
        }
    }

    /// Enables simulation mode (for testing).
    pub fn with_simulation(mut self) -> Self {
        self.simulation_mode = true;
        self
    }

    /// Initialize MIDI output connection if not already connected.
    fn ensure_midi_connection(&self) -> Result<()> {
        let mut midi_out = self.midi_out.lock().unwrap();
        if midi_out.is_some() {
            return Ok(());
        }

        let output = MidiOutput::new("cc-dj-control").map_err(|e| {
            cc_dj_types::DJError::midi(format!("Failed to create MIDI output: {}", e))
        })?;

        let ports = output.ports();
        if ports.is_empty() {
            warn!("No MIDI output ports available");
            return Err(cc_dj_types::DJError::midi("No MIDI output ports available"));
        }

        // Find Serato MIDI port or use first available
        let port = ports
            .iter()
            .find(|p| {
                output
                    .port_name(p)
                    .map(|n| n.to_lowercase().contains("serato"))
                    .unwrap_or(false)
            })
            .unwrap_or(&ports[0]);

        let port_name = output.port_name(port).unwrap_or_else(|_| "Unknown".into());
        info!("Connecting to MIDI port: {}", port_name);

        let conn = output.connect(port, "cc-dj-serato").map_err(|e| {
            cc_dj_types::DJError::midi(format!("Failed to connect to MIDI port: {}", e))
        })?;

        *midi_out = Some(conn);
        Ok(())
    }
}

#[async_trait]
impl DJBridge for SeratoBridge {
    fn name(&self) -> &'static str {
        "Serato"
    }

    async fn execute(&self, action: &Action) -> Result<()> {
        debug!("Executing action: {}", action.name);

        if self.simulation_mode {
            info!("[SIMULATION] Would execute action: {}", action.name);
            return Ok(());
        }

        // Look up the action mapping
        if let Some(config) = &self.config {
            if let Some(mapping) = config.map.get(&action.name) {
                match mapping {
                    cc_dj_types::ActionMapping::Key { key, modifiers } => {
                        let mods: Vec<&str> = modifiers.iter().map(|s| s.as_str()).collect();
                        self.send_key(key, &mods).await?;
                    }
                    cc_dj_types::ActionMapping::Sequence { steps } => {
                        for step in steps {
                            let mods: Vec<&str> =
                                step.modifiers.iter().map(|s| s.as_str()).collect();
                            self.send_key(&step.key, &mods).await?;
                            if step.delay_ms > 0 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(
                                    step.delay_ms as u64,
                                ))
                                .await;
                            }
                        }
                    }
                    cc_dj_types::ActionMapping::Midi {
                        channel,
                        note,
                        velocity,
                    } => {
                        self.send_midi(*channel, *note, *velocity).await?;
                    }
                }
            } else {
                debug!("No mapping found for action: {}", action.name);
            }
        }

        Ok(())
    }

    async fn is_available(&self) -> bool {
        // Check if Serato is running using pgrep on macOS/Linux
        #[cfg(target_os = "macos")]
        {
            // Serato DJ Pro on macOS
            Command::new("pgrep")
                .args(["-x", "Serato DJ Pro"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }

        #[cfg(target_os = "linux")]
        {
            Command::new("pgrep")
                .args(["-x", "serato"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }

        #[cfg(target_os = "windows")]
        {
            Command::new("tasklist")
                .args(["/FI", "IMAGENAME eq Serato DJ Pro.exe"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains("Serato DJ Pro.exe"))
                .unwrap_or(false)
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            warn!("Process detection not supported on this platform");
            true // Assume available on unsupported platforms
        }
    }

    async fn send_key(&self, key: &str, modifiers: &[&str]) -> Result<()> {
        if self.simulation_mode {
            info!(
                "[SIMULATION] Send key: {} with modifiers: {:?}",
                key, modifiers
            );
            return Ok(());
        }

        debug!("Sending key: {} with modifiers: {:?}", key, modifiers);

        // Build AppleScript command for macOS
        #[cfg(target_os = "macos")]
        {
            // Map modifier names to AppleScript key codes
            let mut using_clause = String::new();
            for modifier in modifiers {
                let m = match modifier.to_lowercase().as_str() {
                    "cmd" | "command" => "command down",
                    "ctrl" | "control" => "control down",
                    "alt" | "option" => "option down",
                    "shift" => "shift down",
                    _ => continue,
                };
                if !using_clause.is_empty() {
                    using_clause.push_str(", ");
                }
                using_clause.push_str(m);
            }

            let script = if using_clause.is_empty() {
                format!(
                    r#"tell application "System Events" to keystroke "{}""#,
                    key.to_lowercase()
                )
            } else {
                format!(
                    r#"tell application "System Events" to keystroke "{}" using {{{}}}"#,
                    key.to_lowercase(),
                    using_clause
                )
            };

            let output = Command::new("osascript")
                .args(["-e", &script])
                .output()
                .map_err(|e| {
                    cc_dj_types::DJError::execution(format!("Failed to run osascript: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(cc_dj_types::DJError::execution(format!(
                    "osascript failed: {}",
                    stderr
                )));
            }

            debug!("Key sent successfully via osascript");
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            warn!("Keyboard automation not implemented for this platform");
            Err(cc_dj_types::DJError::execution(
                "Keyboard automation not supported on this platform. Use simulation mode for testing."
            ))
        }
    }

    async fn send_midi(&self, channel: u8, note: u8, velocity: u8) -> Result<()> {
        if self.simulation_mode {
            info!(
                "[SIMULATION] Send MIDI: ch={}, note={}, vel={}",
                channel, note, velocity
            );
            return Ok(());
        }

        debug!(
            "Sending MIDI: ch={}, note={}, vel={}",
            channel, note, velocity
        );

        // Ensure MIDI connection is established
        self.ensure_midi_connection()?;

        // Build MIDI Note On message
        // Status byte: 0x90 + channel (Note On)
        let status = 0x90 | (channel & 0x0F);
        let message = [status, note & 0x7F, velocity & 0x7F];

        // Send the MIDI message
        let mut midi_out = self.midi_out.lock().unwrap();
        if let Some(ref mut conn) = *midi_out {
            conn.send(&message).map_err(|e| {
                cc_dj_types::DJError::midi(format!("Failed to send MIDI message: {}", e))
            })?;
            debug!("MIDI message sent successfully");
        } else {
            return Err(cc_dj_types::DJError::midi(
                "MIDI connection not established",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_serato_bridge() {
        let bridge = SeratoBridge::new(None).with_simulation();
        assert_eq!(bridge.name(), "Serato");
    }

    #[tokio::test]
    async fn test_execute_action() {
        let bridge = SeratoBridge::new(None).with_simulation();
        let action = Action::new("PLAY_A", cc_dj_types::Tier::Transport);

        let result = bridge.execute(&action).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_simulation_mode_key() {
        let bridge = SeratoBridge::new(None).with_simulation();
        let result = bridge.send_key("z", &["cmd"]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_simulation_mode_midi() {
        let bridge = SeratoBridge::new(None).with_simulation();
        let result = bridge.send_midi(0, 60, 127).await;
        assert!(result.is_ok());
    }
}
