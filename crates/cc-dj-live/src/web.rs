//! Web server module — axum REST API, WebSocket, and static file serving.
//!
//! Provides a DJ controller web interface served from the `web/` directory,
//! with REST endpoints for state inspection and action execution, plus a
//! WebSocket endpoint for real-time 30Hz state updates and voice events.

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use cc_dj_control::DeckController;
use cc_dj_types::{DJConfig, SessionState, Tier};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{broadcast, Mutex, RwLock};
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{debug, info};

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Shared application state accessible by all handlers.
#[derive(Clone)]
pub struct AppState {
    /// Current session state (decks, mixer, energy, etc.).
    pub session: Arc<RwLock<SessionState>>,
    /// Deck controller for executing actions.
    pub deck: Arc<Mutex<DeckController>>,
    /// DJ configuration (read-only after startup).
    pub config: Arc<DJConfig>,
    /// Broadcast channel for WebSocket events.
    pub tx: broadcast::Sender<WsEvent>,
    /// Whether voice pipeline is currently active.
    pub voice_active: Arc<AtomicBool>,
    /// Session start time for clock display.
    pub started_at: std::time::Instant,
}

// ---------------------------------------------------------------------------
// WebSocket event types
// ---------------------------------------------------------------------------

/// Events broadcast to all WebSocket clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    /// Full state snapshot (sent on connect and at 30Hz).
    State {
        /// The complete session state.
        session: SessionState,
        /// Elapsed seconds since session start.
        elapsed_secs: f64,
        /// Whether voice pipeline is active.
        voice_active: bool,
    },
    /// Raw text heard from the microphone.
    VoiceHeard {
        /// The transcribed text.
        text: String,
        /// Timestamp (ms since session start).
        ts: u64,
    },
    /// A voice command was recognized and matched.
    VoiceCommand {
        /// The heard text that triggered the command.
        text: String,
        /// The canonical command name.
        command: String,
        /// The command/action ID.
        action_id: String,
        /// Timestamp (ms since session start).
        ts: u64,
    },
    /// An action was successfully executed.
    ActionExecuted {
        /// The action name (e.g. "PLAY_A").
        action: String,
        /// Timestamp (ms since session start).
        ts: u64,
    },
    /// An action failed to execute.
    ActionFailed {
        /// The action name.
        action: String,
        /// Error message.
        error: String,
        /// Timestamp (ms since session start).
        ts: u64,
    },
    /// Beat pulse (fires at BPM rate from master deck).
    Beat {
        /// Current beat position.
        beat: f64,
        /// BPM of the master deck.
        bpm: f64,
    },
    /// A tier was unlocked.
    TierUnlocked {
        /// Tier number (0-5).
        tier: u8,
        /// Tier name.
        name: String,
    },
}

// ---------------------------------------------------------------------------
// REST API types
// ---------------------------------------------------------------------------

/// Information about a single action for the API.
#[derive(Serialize)]
struct ActionInfo {
    name: String,
    tier: u8,
    tier_name: String,
    deck: Option<String>,
    quantized: bool,
    cooldown_beats: u32,
    enabled: bool,
}

/// Request body for executing an action.
#[derive(Deserialize)]
struct ExecuteRequest {
    action: String,
}

/// Generic API response.
#[derive(Serialize)]
struct ApiResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Config response with API keys redacted.
#[derive(Serialize)]
struct ConfigResponse {
    software: String,
    quant_window_deg: f64,
    tiers_enabled: Vec<u8>,
    safety: cc_dj_types::SafetyConfig,
    voice: VoiceConfigRedacted,
}

#[derive(Serialize)]
struct VoiceConfigRedacted {
    enabled: bool,
    engine: String,
    listen_timeout: f64,
    phrase_time_limit: f64,
    speak_feedback: bool,
    audio_feedback: bool,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Builds the axum router with all routes.
pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        .route("/state", get(get_state))
        .route("/actions", get(get_actions))
        .route("/config", get(get_config))
        .route("/execute", post(execute_action))
        .route("/voice/start", post(voice_start))
        .route("/voice/stop", post(voice_stop));

    // Resolve web/ directory relative to CWD
    let serve_dir = ServeDir::new("web").append_index_html_on_directories(true);

    Router::new()
        .nest("/api", api)
        .route("/ws", get(ws_handler))
        .fallback_service(serve_dir)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// ---------------------------------------------------------------------------
// REST handlers
// ---------------------------------------------------------------------------

/// GET /api/state — returns full session state.
async fn get_state(State(state): State<AppState>) -> impl IntoResponse {
    let session = state.session.read().await.clone();
    let elapsed = state.started_at.elapsed().as_secs_f64();
    let voice = state.voice_active.load(Ordering::Relaxed);

    Json(serde_json::json!({
        "session": session,
        "elapsed_secs": elapsed,
        "voice_active": voice,
    }))
}

/// GET /api/actions — returns all actions with tier/cooldown info.
async fn get_actions(State(state): State<AppState>) -> impl IntoResponse {
    let deck = state.deck.lock().await;
    let space = deck.action_space();

    let actions: Vec<ActionInfo> = Tier::all()
        .iter()
        .flat_map(|tier| {
            let enabled = space.is_tier_enabled(*tier);
            space.actions_in_tier(*tier).into_iter().map(move |a| {
                ActionInfo {
                    name: a.name.clone(),
                    tier: a.tier.number(),
                    tier_name: a.tier.name().to_string(),
                    deck: a.deck.map(|d| d.letter().to_string()),
                    quantized: a.quantized,
                    cooldown_beats: a.cooldown_beats,
                    enabled,
                }
            })
        })
        .collect();

    Json(actions)
}

/// GET /api/config — returns config with API keys redacted.
async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = &state.config;
    Json(ConfigResponse {
        software: cfg.software.to_string(),
        quant_window_deg: cfg.quant_window_deg,
        tiers_enabled: cfg.tiers_enabled.clone(),
        safety: cfg.safety.clone(),
        voice: VoiceConfigRedacted {
            enabled: cfg.voice.enabled,
            engine: cfg.voice.engine.clone(),
            listen_timeout: cfg.voice.listen_timeout,
            phrase_time_limit: cfg.voice.phrase_time_limit,
            speak_feedback: cfg.voice.speak_feedback,
            audio_feedback: cfg.voice.audio_feedback,
        },
    })
}

/// POST /api/execute — execute a DJ action.
async fn execute_action(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> impl IntoResponse {
    let action_name = req.action.clone();
    let ts = state.started_at.elapsed().as_millis() as u64;

    let mut deck = state.deck.lock().await;
    match deck.execute(&action_name).await {
        Ok(()) => {
            let _ = state.tx.send(WsEvent::ActionExecuted {
                action: action_name.clone(),
                ts,
            });
            (
                StatusCode::OK,
                Json(ApiResponse {
                    ok: true,
                    message: Some(format!("Executed {}", action_name)),
                    error: None,
                }),
            )
        }
        Err(e) => {
            let error_msg = e.to_string();
            let _ = state.tx.send(WsEvent::ActionFailed {
                action: action_name,
                error: error_msg.clone(),
                ts,
            });
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ApiResponse {
                    ok: false,
                    message: None,
                    error: Some(error_msg),
                }),
            )
        }
    }
}

/// POST /api/voice/start — start voice pipeline.
async fn voice_start(State(state): State<AppState>) -> impl IntoResponse {
    state.voice_active.store(true, Ordering::Relaxed);
    Json(ApiResponse {
        ok: true,
        message: Some("Voice started".into()),
        error: None,
    })
}

/// POST /api/voice/stop — stop voice pipeline.
async fn voice_stop(State(state): State<AppState>) -> impl IntoResponse {
    state.voice_active.store(false, Ordering::Relaxed);
    Json(ApiResponse {
        ok: true,
        message: Some("Voice stopped".into()),
        error: None,
    })
}

// ---------------------------------------------------------------------------
// WebSocket handler
// ---------------------------------------------------------------------------

/// GET /ws — upgrade to WebSocket for real-time events.
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

/// Handle a single WebSocket connection.
async fn handle_ws(mut socket: WebSocket, state: AppState) {
    info!("WebSocket client connected");

    // Send initial state snapshot
    let session = state.session.read().await.clone();
    let elapsed = state.started_at.elapsed().as_secs_f64();
    let voice = state.voice_active.load(Ordering::Relaxed);

    let snapshot = WsEvent::State {
        session,
        elapsed_secs: elapsed,
        voice_active: voice,
    };

    if let Ok(json) = serde_json::to_string(&snapshot) {
        if socket.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    // Subscribe to broadcast channel
    let mut rx = state.tx.subscribe();

    // Forward all broadcast events to this client
    loop {
        tokio::select! {
            // Broadcast event → send to client
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        if let Ok(json) = serde_json::to_string(&event) {
                            if socket.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("WebSocket client lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            // Client message → handle pings / detect disconnect
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {} // Ignore text/binary from client
                    Some(Err(_)) => break,
                }
            }
        }
    }

    info!("WebSocket client disconnected");
}

// ---------------------------------------------------------------------------
// State broadcast loop (30Hz)
// ---------------------------------------------------------------------------

/// Spawns a task that broadcasts the session state at ~30Hz.
pub fn spawn_state_broadcaster(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(33));
        let mut beat_acc: f64 = 0.0;
        let mut last_beat_int: u64 = 0;

        loop {
            interval.tick().await;

            let session = state.session.read().await.clone();
            let elapsed = state.started_at.elapsed().as_secs_f64();
            let voice = state.voice_active.load(Ordering::Relaxed);

            // Broadcast state
            let _ = state.tx.send(WsEvent::State {
                session: session.clone(),
                elapsed_secs: elapsed,
                voice_active: voice,
            });

            // Simulate beat events from master deck BPM
            if let Some(master) = session.master_deck() {
                if master.bpm > 0.0 && master.is_playing {
                    let beats_per_sec = master.effective_bpm() / 60.0;
                    beat_acc += beats_per_sec * 0.033;
                    let current_beat_int = beat_acc as u64;
                    if current_beat_int > last_beat_int {
                        last_beat_int = current_beat_int;
                        let _ = state.tx.send(WsEvent::Beat {
                            beat: beat_acc,
                            bpm: master.effective_bpm(),
                        });
                    }
                }
            }

            // If no subscribers, slow down to avoid burning CPU
            if state.tx.receiver_count() == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
    });
}

/// Starts the axum web server on the given port.
pub async fn start_server(state: AppState, port: u16) -> anyhow::Result<()> {
    let router = build_router(state.clone());
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    info!("Web server listening on http://0.0.0.0:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
