//! Live API WebSocket session management.
//!
//! Provides the `LiveSession` type for managing bidirectional WebSocket
//! connections to the Gemini Live API.

use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};
use url::Url;

use super::config::{LiveConfig, LiveModel};
use super::error::{LiveError, LiveResult};
use super::messages::{
    BidiGenerateContentClientContent, BidiGenerateContentRealtimeInput,
    BidiGenerateContentToolResponse, ClientMessage, ServerMessage,
};

/// WebSocket connection type alias.
type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
/// WebSocket sender type alias.
type WsSender = SplitSink<WsStream, Message>;
/// WebSocket receiver type alias.
type WsReceiver = SplitStream<WsStream>;

/// Session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session is connecting.
    Connecting,
    /// Session is setting up.
    SettingUp,
    /// Session is ready for communication.
    Ready,
    /// Session is closing.
    Closing,
    /// Session is closed.
    Closed,
}

/// Live API session callbacks.
pub trait LiveSessionCallbacks: Send + Sync {
    /// Called when the connection is opened.
    fn on_open(&self) {}

    /// Called when a message is received.
    fn on_message(&self, message: ServerMessage);

    /// Called when an error occurs.
    fn on_error(&self, error: LiveError) {
        error!("Live session error: {}", error);
    }

    /// Called when the connection is closed.
    fn on_close(&self, code: u16, reason: &str) {
        info!("Live session closed: code={}, reason={}", code, reason);
    }
}

/// Default callbacks that collect messages into a channel.
pub struct ChannelCallbacks {
    sender: mpsc::UnboundedSender<ServerMessage>,
}

impl ChannelCallbacks {
    /// Creates new channel callbacks.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ServerMessage>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }
}

impl LiveSessionCallbacks for ChannelCallbacks {
    fn on_message(&self, message: ServerMessage) {
        let _ = self.sender.send(message);
    }
}

/// Live API WebSocket session.
pub struct LiveSession {
    /// API key.
    api_key: String,
    /// Model to use.
    model: LiveModel,
    /// Session configuration.
    config: LiveConfig,
    /// Current session state.
    state: Arc<RwLock<SessionState>>,
    /// WebSocket sender.
    sender: Arc<Mutex<Option<WsSender>>>,
    /// Session resumption handle.
    resumption_handle: Arc<RwLock<Option<String>>>,
    /// Message receiver task handle.
    receiver_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl LiveSession {
    /// Creates a new Live session builder.
    pub fn builder(api_key: impl Into<String>) -> LiveSessionBuilder {
        LiveSessionBuilder::new(api_key)
    }

    /// Creates a new Live session from environment.
    ///
    /// Reads `GEMINI_API_KEY` from the environment.
    pub fn from_env() -> LiveResult<LiveSessionBuilder> {
        let api_key = std::env::var("GEMINI_API_KEY").map_err(|_| {
            LiveError::ConfigError("GEMINI_API_KEY environment variable not set".to_string())
        })?;
        Ok(LiveSessionBuilder::new(api_key))
    }

    /// Returns the current session state.
    pub async fn state(&self) -> SessionState {
        *self.state.read().await
    }

    /// Returns the session resumption handle if available.
    pub async fn resumption_handle(&self) -> Option<String> {
        self.resumption_handle.read().await.clone()
    }

    /// Connects to the Live API.
    pub async fn connect<C: LiveSessionCallbacks + 'static>(
        &self,
        callbacks: Arc<C>,
    ) -> LiveResult<()> {
        // Build WebSocket URL
        let ws_url = format!(
            "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent?key={}",
            self.api_key
        );

        let url = Url::parse(&ws_url)
            .map_err(|e| LiveError::ConfigError(format!("Invalid WebSocket URL: {}", e)))?;

        // Update state
        {
            let mut state = self.state.write().await;
            *state = SessionState::Connecting;
        }

        // Connect to WebSocket
        debug!("Connecting to Live API...");
        let (ws_stream, _response) = connect_async(url)
            .await
            .map_err(|e| LiveError::connection_failed(e.to_string(), true))?;

        info!("Connected to Live API");
        callbacks.on_open();

        // Split the stream
        let (sender, receiver) = ws_stream.split();

        // Store the sender
        {
            let mut sender_lock = self.sender.lock().await;
            *sender_lock = Some(sender);
        }

        // Update state
        {
            let mut state = self.state.write().await;
            *state = SessionState::SettingUp;
        }

        // Send setup message
        self.send_setup().await?;

        // Start receiver task
        let state_clone = Arc::clone(&self.state);
        let resumption_clone = Arc::clone(&self.resumption_handle);
        let callbacks_clone = callbacks;

        let receiver_handle = tokio::spawn(async move {
            Self::receive_loop(receiver, state_clone, resumption_clone, callbacks_clone).await;
        });

        {
            let mut task = self.receiver_task.lock().await;
            *task = Some(receiver_handle);
        }

        Ok(())
    }

    /// Sends the setup message.
    async fn send_setup(&self) -> LiveResult<()> {
        let setup_msg = ClientMessage::setup(self.model.as_str(), self.config.clone());
        self.send_raw(setup_msg).await
    }

    /// Sends a raw client message.
    async fn send_raw(&self, message: ClientMessage) -> LiveResult<()> {
        let json = serde_json::to_string(&message)
            .map_err(|e| LiveError::SerializationError(e.to_string()))?;

        let mut sender_lock = self.sender.lock().await;
        if let Some(ref mut sender) = *sender_lock {
            sender
                .send(Message::Text(json))
                .await
                .map_err(|e| LiveError::connection_failed(e.to_string(), true))?;
            Ok(())
        } else {
            Err(LiveError::SessionExpired)
        }
    }

    /// Sends text content.
    pub async fn send_text(&self, text: impl Into<String>) -> LiveResult<()> {
        let state = self.state().await;
        if state != SessionState::Ready {
            return Err(LiveError::SessionExpired);
        }

        let content = BidiGenerateContentClientContent::text(text);
        self.send_raw(ClientMessage::client_content(content)).await
    }

    /// Sends audio data.
    ///
    /// Audio should be 16-bit PCM at the specified sample rate (typically 16000 Hz).
    pub async fn send_audio(&self, data: &[u8], sample_rate: u32) -> LiveResult<()> {
        let state = self.state().await;
        if state != SessionState::Ready {
            return Err(LiveError::SessionExpired);
        }

        let input = BidiGenerateContentRealtimeInput::audio(data, sample_rate);
        self.send_raw(ClientMessage::realtime_input(input)).await
    }

    /// Sends video data.
    pub async fn send_video(&self, data: &[u8], mime_type: &str) -> LiveResult<()> {
        let state = self.state().await;
        if state != SessionState::Ready {
            return Err(LiveError::SessionExpired);
        }

        let input = BidiGenerateContentRealtimeInput::video(data, mime_type);
        self.send_raw(ClientMessage::realtime_input(input)).await
    }

    /// Sends an activity start marker (for manual VAD).
    pub async fn send_activity_start(&self) -> LiveResult<()> {
        let input = BidiGenerateContentRealtimeInput::activity_start();
        self.send_raw(ClientMessage::realtime_input(input)).await
    }

    /// Sends an activity end marker (for manual VAD).
    pub async fn send_activity_end(&self) -> LiveResult<()> {
        let input = BidiGenerateContentRealtimeInput::activity_end();
        self.send_raw(ClientMessage::realtime_input(input)).await
    }

    /// Sends an audio stream end marker.
    pub async fn send_audio_stream_end(&self) -> LiveResult<()> {
        let input = BidiGenerateContentRealtimeInput::audio_stream_end();
        self.send_raw(ClientMessage::realtime_input(input)).await
    }

    /// Sends a tool response.
    pub async fn send_tool_response(
        &self,
        response: BidiGenerateContentToolResponse,
    ) -> LiveResult<()> {
        self.send_raw(ClientMessage::tool_response(response)).await
    }

    /// Closes the session.
    pub async fn close(&self) -> LiveResult<()> {
        {
            let mut state = self.state.write().await;
            *state = SessionState::Closing;
        }

        // Close the WebSocket
        {
            let mut sender_lock = self.sender.lock().await;
            if let Some(ref mut sender) = *sender_lock {
                let _ = sender.send(Message::Close(None)).await;
            }
            *sender_lock = None;
        }

        // Cancel the receiver task
        {
            let mut task = self.receiver_task.lock().await;
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }

        {
            let mut state = self.state.write().await;
            *state = SessionState::Closed;
        }

        info!("Live session closed");
        Ok(())
    }

    /// Receive loop for processing server messages.
    async fn receive_loop<C: LiveSessionCallbacks>(
        mut receiver: WsReceiver,
        state: Arc<RwLock<SessionState>>,
        resumption_handle: Arc<RwLock<Option<String>>>,
        callbacks: Arc<C>,
    ) {
        while let Some(result) = receiver.next().await {
            match result {
                Ok(Message::Text(text)) => {
                    debug!("WS recv: {}", &text[..text.len().min(500)]);
                    match serde_json::from_str::<ServerMessage>(&text) {
                        Ok(message) => {
                            // Handle setup complete
                            if message.is_setup_complete() {
                                let mut s = state.write().await;
                                *s = SessionState::Ready;
                                info!("Live session ready");
                            }

                            // Handle session resumption update
                            if let Some(ref update) = message.session_resumption_update {
                                if update.resumable {
                                    if let Some(ref handle) = update.new_handle {
                                        let mut h = resumption_handle.write().await;
                                        *h = Some(handle.clone());
                                        debug!("Session resumption handle updated");
                                    }
                                }
                            }

                            // Handle GoAway
                            if let Some(ref go_away) = message.go_away {
                                warn!("Server sent GoAway: {} remaining", go_away.time_left);
                            }

                            callbacks.on_message(message);
                        }
                        Err(e) => {
                            error!("Failed to parse server message: {}", e);
                            callbacks.on_error(LiveError::DeserializationError(e.to_string()));
                        }
                    }
                }
                Ok(Message::Binary(data)) => {
                    // Gemini Live API may send JSON as binary frames
                    debug!("Received binary message: {} bytes", data.len());
                    if let Ok(text) = String::from_utf8(data) {
                        debug!("Binary->text: {}", &text[..text.len().min(500)]);
                        match serde_json::from_str::<ServerMessage>(&text) {
                            Ok(message) => {
                                if message.is_setup_complete() {
                                    let mut s = state.write().await;
                                    *s = SessionState::Ready;
                                    info!("Live session ready");
                                }
                                if let Some(ref update) = message.session_resumption_update {
                                    if update.resumable {
                                        if let Some(ref handle) = update.new_handle {
                                            let mut h = resumption_handle.write().await;
                                            *h = Some(handle.clone());
                                        }
                                    }
                                }
                                if let Some(ref go_away) = message.go_away {
                                    warn!("Server sent GoAway: {} remaining", go_away.time_left);
                                }
                                callbacks.on_message(message);
                            }
                            Err(e) => {
                                debug!("Binary frame not JSON: {}", e);
                            }
                        }
                    }
                }
                Ok(Message::Ping(_)) => {
                    debug!("Received ping");
                }
                Ok(Message::Pong(_)) => {
                    debug!("Received pong");
                }
                Ok(Message::Close(frame)) => {
                    let (code, reason) = frame
                        .map(|f| (f.code.into(), f.reason.to_string()))
                        .unwrap_or((1000, "Normal close".to_string()));

                    let mut s = state.write().await;
                    *s = SessionState::Closed;

                    callbacks.on_close(code, &reason);
                    break;
                }
                Ok(Message::Frame(_)) => {
                    // Raw frame, usually not received
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    callbacks.on_error(LiveError::connection_failed(e.to_string(), true));

                    let mut s = state.write().await;
                    *s = SessionState::Closed;
                    break;
                }
            }
        }
    }
}

/// Builder for creating Live sessions.
pub struct LiveSessionBuilder {
    api_key: String,
    model: LiveModel,
    config: LiveConfig,
}

impl LiveSessionBuilder {
    /// Creates a new session builder.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: LiveModel::default(),
            config: LiveConfig::default(),
        }
    }

    /// Sets the model.
    pub fn model(mut self, model: LiveModel) -> Self {
        self.model = model;
        self
    }

    /// Sets the session configuration.
    pub fn config(mut self, config: LiveConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets the system instruction.
    pub fn system_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.config = self.config.with_system_instruction(instruction);
        self
    }

    /// Enables input transcription.
    pub fn with_input_transcription(mut self) -> Self {
        self.config = self.config.with_input_transcription();
        self
    }

    /// Enables output transcription.
    pub fn with_output_transcription(mut self) -> Self {
        self.config = self.config.with_output_transcription();
        self
    }

    /// Enables context compression for long sessions.
    pub fn with_context_compression(mut self) -> Self {
        self.config = self.config.with_context_compression();
        self
    }

    /// Sets a session handle for resumption.
    pub fn resume_from(mut self, handle: impl Into<String>) -> Self {
        self.config = self.config.with_session_resumption(Some(handle.into()));
        self
    }

    /// Builds the session.
    pub fn build(self) -> LiveSession {
        LiveSession {
            api_key: self.api_key,
            model: self.model,
            config: self.config,
            state: Arc::new(RwLock::new(SessionState::Closed)),
            sender: Arc::new(Mutex::new(None)),
            resumption_handle: Arc::new(RwLock::new(None)),
            receiver_task: Arc::new(Mutex::new(None)),
        }
    }
}

/// Simple helper to wait for a message with timeout.
pub async fn wait_for_message(
    receiver: &mut mpsc::UnboundedReceiver<ServerMessage>,
    timeout: Duration,
) -> Option<ServerMessage> {
    tokio::time::timeout(timeout, receiver.recv())
        .await
        .ok()
        .flatten()
}

/// Collects messages until turn complete.
pub async fn collect_turn(
    receiver: &mut mpsc::UnboundedReceiver<ServerMessage>,
    timeout: Duration,
) -> Vec<ServerMessage> {
    let mut messages = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining, receiver.recv()).await {
            Ok(Some(msg)) => {
                let is_complete = msg.is_turn_complete() || msg.is_generation_complete();
                messages.push(msg);
                if is_complete {
                    break;
                }
            }
            _ => break,
        }
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_builder() {
        let session = LiveSession::builder("test_key")
            .model(LiveModel::Flash25NativeAudio)
            .system_instruction("You are helpful")
            .with_input_transcription()
            .build();

        assert!(session.config.system_instruction.is_some());
        assert!(session.config.input_audio_transcription.is_some());
    }

    #[test]
    fn test_channel_callbacks() {
        let (callbacks, mut receiver) = ChannelCallbacks::new();

        let msg = ServerMessage {
            usage_metadata: None,
            setup_complete: Some(super::super::messages::SetupComplete {}),
            server_content: None,
            tool_call: None,
            tool_call_cancellation: None,
            go_away: None,
            session_resumption_update: None,
        };

        callbacks.on_message(msg);

        // Should receive the message
        let received = receiver.try_recv();
        assert!(received.is_ok());
    }
}
