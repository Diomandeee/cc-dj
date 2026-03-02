//! cc-dj — Voice-controlled DJ agent for Rekordbox.
//!
//! Streams microphone audio to Gemini Live for transcription, maps spoken
//! commands to DJ actions, and sends keyboard shortcuts to Rekordbox.
//!
//! # Usage
//!
//! ```sh
//! GEMINI_API_KEY=<key> cargo run -- --config configs/dj.yaml --commands configs/commands.yaml
//! ```
//!
//! # Web UI
//!
//! ```sh
//! GEMINI_API_KEY=test cargo run -- --simulate --web-port 8080
//! # Open http://localhost:8080
//! ```

mod web;

use anyhow::{Context, Result};
use cc_dj_control::DeckController;
use cc_dj_types::{DJConfig, SessionState};
use cc_dj_voice::VoiceController;
use clap::Parser;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::info;

use web::{AppState, WsEvent};

/// Voice-to-Rekordbox DJ controller.
#[derive(Parser)]
#[command(name = "cc-dj", about = "Voice-controlled DJ agent")]
struct Args {
    /// Path to dj.yaml configuration file.
    #[arg(short, long, default_value = "configs/dj.yaml")]
    config: String,

    /// Path to commands.yaml command definitions.
    #[arg(long, default_value = "configs/commands.yaml")]
    commands: String,

    /// Override DJ software (rekordbox / serato).
    #[arg(long, value_enum)]
    software: Option<cc_dj_types::DJSoftware>,

    /// Simulation mode — log actions without sending keystrokes.
    #[arg(long)]
    simulate: bool,

    /// Web UI port (0 = disabled).
    #[arg(long, default_value = "8080")]
    web_port: u16,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialise tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&args.log_level)),
        )
        .init();

    // Load config
    let mut config = DJConfig::from_file(&args.config).context("Failed to load DJ config")?;

    if let Some(sw) = args.software {
        config.software = sw;
    }

    info!("Software: {}", config.software);
    info!("Tiers enabled: {:?}", config.tiers_enabled);

    // Load commands YAML
    let commands_yaml =
        std::fs::read_to_string(&args.commands).context("Failed to read commands file")?;

    // Read API key
    let api_key =
        std::env::var("GEMINI_API_KEY").context("GEMINI_API_KEY environment variable not set")?;

    // Shared config
    let config_arc = Arc::new(config.clone());

    // Build DeckController
    let deck = Arc::new(Mutex::new(DeckController::new(config.clone())));

    // Build session state (2 decks)
    let session = Arc::new(RwLock::new(SessionState::new(2)));

    // Broadcast channel for WebSocket events (256-message buffer)
    let (tx, _rx) = broadcast::channel::<WsEvent>(256);

    // Voice status flag
    let voice_active = Arc::new(AtomicBool::new(false));

    // Build web AppState
    let app_state = AppState {
        session: session.clone(),
        deck: deck.clone(),
        config: config_arc,
        tx: tx.clone(),
        voice_active: voice_active.clone(),
        started_at: std::time::Instant::now(),
    };

    // Start web server if port > 0
    if args.web_port > 0 {
        let web_state = app_state.clone();
        let port = args.web_port;

        // Spawn state broadcaster (30Hz)
        web::spawn_state_broadcaster(web_state.clone());

        // Spawn axum server
        tokio::spawn(async move {
            if let Err(e) = web::start_server(web_state, port).await {
                tracing::error!("Web server error: {}", e);
            }
        });

        info!("Web UI: http://localhost:{}", args.web_port);
    }

    // Build VoiceController
    let mut voice = VoiceController::new(&api_key, config);

    voice
        .load_commands(&commands_yaml)
        .context("Failed to load command definitions")?;

    // Wire command callback → DeckController + broadcast
    let deck_for_cb = deck.clone();
    let tx_for_cb = tx.clone();
    let started_at = app_state.started_at;
    let simulate = args.simulate;

    voice.on_command(move |cmd| {
        info!("[DJ] Command: {} -> {}", cmd.canonical, cmd.id);

        let ts = started_at.elapsed().as_millis() as u64;

        // Broadcast voice command event
        let _ = tx_for_cb.send(WsEvent::VoiceCommand {
            text: cmd.canonical.clone(),
            command: cmd.canonical.clone(),
            action_id: cmd.id.clone(),
            ts,
        });

        if simulate {
            info!("[SIMULATE] Would execute: {}", cmd.id);
            let _ = tx_for_cb.send(WsEvent::ActionExecuted {
                action: cmd.id.clone(),
                ts,
            });
            return;
        }

        let deck = deck_for_cb.clone();
        let action_name = cmd.id.clone();
        let tx = tx_for_cb.clone();

        tokio::spawn(async move {
            let ts = started_at.elapsed().as_millis() as u64;
            let mut d = deck.lock().await;
            match d.execute(&action_name).await {
                Ok(()) => {
                    let _ = tx.send(WsEvent::ActionExecuted {
                        action: action_name,
                        ts,
                    });
                }
                Err(e) => {
                    tracing::warn!("Execute failed for {}: {}", action_name, e);
                    let _ = tx.send(WsEvent::ActionFailed {
                        action: action_name,
                        error: e.to_string(),
                        ts,
                    });
                }
            }
        });
    });

    // Mark voice as active
    voice_active.store(true, Ordering::Relaxed);

    // Start voice pipeline
    voice
        .start()
        .await
        .context("Failed to start voice controller")?;

    info!("cc-dj running. Speak commands. Ctrl+C to stop.");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");
    voice_active.store(false, Ordering::Relaxed);
    voice
        .stop()
        .await
        .context("Failed to stop voice controller")?;

    Ok(())
}
