//! Rekordbox bridge implementation.

use async_trait::async_trait;
use cc_dj_types::{Action, DJError, Result, SoftwareConfig};
use midir::{MidiOutput, MidiOutputConnection};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Validates that a key string is safe for AppleScript injection.
///
/// Allows only short ASCII alphanumeric or punctuation strings to prevent
/// command injection via crafted key values.
fn validate_key(key: &str) -> Result<()> {
    let valid = !key.is_empty()
        && key.len() <= 2
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c.is_ascii_punctuation());
    if !valid {
        return Err(DJError::execution(format!("Invalid key value: {:?}", key)));
    }
    Ok(())
}

/// Escapes double-quote characters in a string for safe AppleScript embedding.
fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

use super::DJBridge;

/// Bridge for Rekordbox DJ software.
pub struct RekordboxBridge {
    /// Software configuration.
    config: Option<SoftwareConfig>,
    /// Whether we're in simulation mode (no actual execution).
    simulation_mode: bool,
    /// Cached MIDI output connection.
    midi_out: Mutex<Option<MidiOutputConnection>>,
}

impl std::fmt::Debug for RekordboxBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RekordboxBridge")
            .field("config", &self.config)
            .field("simulation_mode", &self.simulation_mode)
            .field("midi_out", &"<MidiOutputConnection>")
            .finish()
    }
}

impl RekordboxBridge {
    /// Creates a new Rekordbox bridge.
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
    async fn ensure_midi_connection(&self) -> Result<()> {
        let mut midi_out = self.midi_out.lock().await;
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

        // Find Rekordbox MIDI port or use first available
        let port = ports
            .iter()
            .find(|p| {
                output
                    .port_name(p)
                    .map(|n| n.to_lowercase().contains("rekordbox"))
                    .unwrap_or(false)
            })
            .unwrap_or(&ports[0]);

        let port_name = output.port_name(port).unwrap_or_else(|_| "Unknown".into());
        info!("Connecting to MIDI port: {}", port_name);

        let conn = output.connect(port, "cc-dj-rekordbox").map_err(|e| {
            cc_dj_types::DJError::midi(format!("Failed to connect to MIDI port: {}", e))
        })?;

        *midi_out = Some(conn);
        Ok(())
    }
}

#[async_trait]
impl DJBridge for RekordboxBridge {
    fn name(&self) -> &'static str {
        "Rekordbox"
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
        // Always available in simulation mode
        if self.simulation_mode {
            return true;
        }

        // Check if Rekordbox is running using pgrep on macOS/Linux
        #[cfg(target_os = "macos")]
        {
            tokio::process::Command::new("pgrep")
                .args(["-x", "rekordbox"])
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false)
        }

        #[cfg(target_os = "linux")]
        {
            tokio::process::Command::new("pgrep")
                .args(["-x", "rekordbox"])
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false)
        }

        #[cfg(target_os = "windows")]
        {
            tokio::process::Command::new("tasklist")
                .args(["/FI", "IMAGENAME eq rekordbox.exe"])
                .output()
                .await
                .map(|o| String::from_utf8_lossy(&o.stdout).contains("rekordbox.exe"))
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

        // C1: Validate key to prevent AppleScript injection
        validate_key(key)?;

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

            let safe_key = escape_applescript(&key.to_lowercase());

            // M9: Activate Rekordbox before sending keystroke
            let activate = r#"tell application "rekordbox" to activate"#;
            let keystroke = if using_clause.is_empty() {
                format!(
                    r#"tell application "System Events" to keystroke "{}""#,
                    safe_key
                )
            } else {
                format!(
                    r#"tell application "System Events" to keystroke "{}" using {{{}}}"#,
                    safe_key, using_clause
                )
            };

            let script = format!("{}\n{}", activate, keystroke);

            let output = tokio::process::Command::new("osascript")
                .args(["-e", &script])
                .output()
                .await
                .map_err(|e| DJError::execution(format!("Failed to run osascript: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(DJError::execution(format!("osascript failed: {}", stderr)));
            }

            debug!("Key sent successfully via osascript");
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            warn!("Keyboard automation not implemented for this platform");
            Err(DJError::execution(
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
        self.ensure_midi_connection().await?;

        // Build MIDI Note On message
        // Status byte: 0x90 + channel (Note On)
        let status = 0x90 | (channel & 0x0F);
        let message = [status, note & 0x7F, velocity & 0x7F];

        // Send the MIDI message
        let mut midi_out = self.midi_out.lock().await;
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
    async fn test_rekordbox_bridge_simulation() {
        let bridge = RekordboxBridge::new(None).with_simulation();
        assert_eq!(bridge.name(), "Rekordbox");
        assert!(bridge.is_available().await);

        // Simulation mode should execute without errors for any action
        let action = Action::new("PLAY_A", cc_dj_types::Tier::Transport);
        assert!(bridge.execute(&action).await.is_ok());

        // send_key in simulation should succeed regardless of key
        assert!(bridge.send_key("Z", &[]).await.is_ok());
        assert!(bridge.send_key("Z", &["shift", "cmd"]).await.is_ok());

        // send_midi in simulation should succeed
        assert!(bridge.send_midi(0, 60, 127).await.is_ok());
    }

    #[tokio::test]
    async fn test_execute_with_action_mapping() {
        use cc_dj_types::{ActionMapping, SoftwareConfig};
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert(
            "PLAY_A".to_string(),
            ActionMapping::Key {
                key: "Z".to_string(),
                modifiers: vec![],
            },
        );

        let config = SoftwareConfig {
            mode: "keyboard".to_string(),
            midi_port: None,
            map,
        };

        let bridge = RekordboxBridge::new(Some(config)).with_simulation();
        let action = Action::new("PLAY_A", cc_dj_types::Tier::Transport);

        // Should resolve the mapping and execute in simulation
        assert!(bridge.execute(&action).await.is_ok());
    }

    #[test]
    fn test_key_validation() {
        // Valid keys
        assert!(validate_key("Z").is_ok());
        assert!(validate_key("1").is_ok());
        assert!(validate_key(",").is_ok());

        // Invalid keys — injection attempts
        assert!(validate_key("").is_err());
        assert!(validate_key("abc").is_err()); // too long
        assert!(validate_key("Z\" & do shell script \"evil").is_err());
    }
}
