//! Voice controller - main orchestrator for voice control.

use cc_dj_types::{Command, DJConfig, DJError, Result};
use cc_gemini::live::{ChannelCallbacks, LiveConfig, LiveSession, SessionState, VadConfigBuilder};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

use crate::intent::IntentProcessor;
use crate::orbiter::CommandOrbiter;

/// Type alias for command callback.
pub type CommandCallback = Arc<dyn Fn(&Command) + Send + Sync + 'static>;

/// Voice controller for DJ agent.
///
/// Coordinates Gemini Live API voice recognition with command processing.
/// Call [`load_commands`](VoiceController::load_commands) before [`start`](VoiceController::start).
pub struct VoiceController {
    /// Gemini API key.
    api_key: String,
    /// DJ configuration.
    #[allow(dead_code)]
    config: Arc<DJConfig>,
    /// Command orbiter for retrieval.
    orbiter: CommandOrbiter,
    /// Intent processor (shared with transcription task).
    intent_processor: Arc<RwLock<IntentProcessor>>,
    /// Whether the controller is running.
    running: bool,
    /// Callback invoked when a command is recognized.
    command_callback: Option<CommandCallback>,
    /// Shutdown signal shared with mic thread and async tasks.
    shutdown: Arc<AtomicBool>,
    /// Mic capture thread handle.
    capture_handle: Option<std::thread::JoinHandle<()>>,
    /// Task that sends audio chunks to Gemini.
    audio_task: Option<tokio::task::JoinHandle<()>>,
    /// Task that receives transcriptions and invokes commands.
    transcription_task: Option<tokio::task::JoinHandle<()>>,
}

impl VoiceController {
    /// Creates a new voice controller.
    pub fn new(api_key: impl Into<String>, config: DJConfig) -> Self {
        let config = Arc::new(config);

        Self {
            api_key: api_key.into(),
            config: config.clone(),
            orbiter: CommandOrbiter::new(),
            intent_processor: Arc::new(RwLock::new(IntentProcessor::new(config))),
            running: false,
            command_callback: None,
            shutdown: Arc::new(AtomicBool::new(false)),
            capture_handle: None,
            audio_task: None,
            transcription_task: None,
        }
    }

    /// Loads command definitions from YAML (call before `start`).
    pub fn load_commands(&self, yaml: &str) -> Result<()> {
        self.intent_processor
            .write()
            .map_err(|e| DJError::voice(format!("Lock poisoned: {}", e)))?
            .load_commands(yaml)
    }

    /// Starts the voice controller.
    ///
    /// Opens a Gemini Live session, starts microphone capture, and begins
    /// streaming audio for transcription. Recognised text is mapped to DJ
    /// commands via the intent processor and dispatched through the callback.
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting voice controller...");

        // Build LiveConfig tuned for a DJ booth:
        // - text-only responses (no audio playback from the model)
        // - input transcription enabled
        // - low VAD start sensitivity (avoid triggering on music)
        // - 800ms silence to mark end of speech
        let vad = VadConfigBuilder::new()
            .low_start_sensitivity()
            .silence_duration(800)
            .build();

        let mut live_config = LiveConfig::audio().with_input_transcription();
        live_config.realtime_input_config = Some(vad);

        // Build & connect the Gemini Live session
        let session = Arc::new(
            LiveSession::builder(&self.api_key)
                .config(live_config)
                .build(),
        );

        let (callbacks, mut msg_rx) = ChannelCallbacks::new();

        session
            .connect(Arc::new(callbacks))
            .await
            .map_err(|e| DJError::voice(format!("Gemini connect failed: {}", e)))?;

        // Wait for Ready (10 s timeout)
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(10);
        loop {
            if session.state().await == SessionState::Ready {
                break;
            }
            if tokio::time::Instant::now() > deadline {
                return Err(DJError::voice("Gemini session setup timed out"));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        info!("Gemini Live session ready");

        // --- Mic capture ---
        self.shutdown.store(false, Ordering::SeqCst);
        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);

        let mic_handle = crate::mic::start_mic_capture(self.shutdown.clone(), audio_tx)
            .map_err(|e| DJError::voice(format!("Mic init failed: {}", e)))?;

        let sample_rate = mic_handle.sample_rate;
        self.capture_handle = Some(mic_handle.thread);

        // --- Audio sender task ---
        let session_audio = session.clone();
        let shutdown_audio = self.shutdown.clone();

        self.audio_task = Some(tokio::spawn(async move {
            while let Some(chunk) = audio_rx.recv().await {
                if shutdown_audio.load(Ordering::Relaxed) {
                    break;
                }
                if let Err(e) = session_audio.send_audio(&chunk, sample_rate).await {
                    warn!("send_audio error: {}", e);
                    break;
                }
            }
            debug!("Audio sender task exited");
        }));

        // --- Transcription receiver task ---
        let intent_processor = self.intent_processor.clone();
        let callback = self.command_callback.clone();
        let shutdown_rx = self.shutdown.clone();

        self.transcription_task = Some(tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                if shutdown_rx.load(Ordering::Relaxed) {
                    break;
                }

                // Handle input transcription
                if let Some(text) = msg.input_transcription() {
                    let text = text.trim();
                    if text.is_empty() {
                        continue;
                    }

                    info!("[VOICE] Heard: \"{}\"", text);

                    let commands = {
                        let processor = intent_processor.read().unwrap();
                        processor.process(text)
                    };

                    if let Some(ref cb) = callback {
                        for cmd in &commands {
                            info!("[DJ] Command: {} -> {}", cmd.canonical, cmd.id);
                            cb(cmd);
                        }
                    }
                }

                // Warn on GoAway (session nearing expiry)
                if msg.go_away.is_some() {
                    warn!("Gemini GoAway — session expiring soon");
                }
            }
            debug!("Transcription task exited");
        }));

        self.running = true;
        info!("Voice controller started (mic -> Gemini Live -> commands)");
        Ok(())
    }

    /// Stops the voice controller and cleans up resources.
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping voice controller...");
        self.shutdown.store(true, Ordering::SeqCst);

        // Abort async tasks
        if let Some(task) = self.audio_task.take() {
            task.abort();
        }
        if let Some(task) = self.transcription_task.take() {
            task.abort();
        }

        // Join mic capture thread
        if let Some(handle) = self.capture_handle.take() {
            let _ = handle.join();
        }

        self.running = false;
        info!("Voice controller stopped");
        Ok(())
    }

    /// Returns true if the controller is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Processes recognized text and returns matching commands.
    ///
    /// If a callback is registered via `on_command`, it will be invoked
    /// for each recognized command.
    pub fn process_text(&self, text: &str) -> Vec<Command> {
        debug!("Processing text: {}", text);

        let commands = self.intent_processor.read().unwrap().process(text);

        // Invoke callback for each recognized command
        if let Some(ref callback) = self.command_callback {
            for cmd in &commands {
                debug!("Invoking callback for command: {}", cmd.canonical);
                callback(cmd);
            }
        }

        commands
    }

    /// Sets the callback for when a command is recognized.
    ///
    /// The callback will be invoked for each command returned by `process_text`
    /// and for each transcription received from Gemini Live during `start`.
    pub fn on_command<F>(&mut self, callback: F)
    where
        F: Fn(&Command) + Send + Sync + 'static,
    {
        self.command_callback = Some(Arc::new(callback));
    }

    /// Clears the command callback.
    pub fn clear_command_callback(&mut self) {
        self.command_callback = None;
    }

    /// Returns the command orbiter for embedding-based retrieval.
    pub fn orbiter(&self) -> &CommandOrbiter {
        &self.orbiter
    }

    /// Returns a mutable reference to the command orbiter.
    pub fn orbiter_mut(&mut self) -> &mut CommandOrbiter {
        &mut self.orbiter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_voice_controller_creation() {
        let config = DJConfig::default();
        let controller = VoiceController::new("test_key", config);
        assert!(!controller.is_running());
    }

    #[test]
    fn test_command_callback() {
        let config = DJConfig::default();
        let mut controller = VoiceController::new("test_key", config);

        // Track callback invocations
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        controller.on_command(move |_cmd| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Process some text (may not match any commands with empty catalog)
        let _commands = controller.process_text("play");

        // Callback invocation count depends on catalog matches
        // With empty catalog, no commands match, so callback won't be invoked
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_clear_callback() {
        let config = DJConfig::default();
        let mut controller = VoiceController::new("test_key", config);

        controller.on_command(|_cmd| {});
        assert!(controller.command_callback.is_some());

        controller.clear_command_callback();
        assert!(controller.command_callback.is_none());
    }
}
