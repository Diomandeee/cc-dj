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

use anyhow::{Context, Result};
use cc_dj_control::DeckController;
use cc_dj_types::DJConfig;
use cc_dj_voice::VoiceController;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

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
    #[arg(long)]
    software: Option<String>,

    /// Simulation mode — log actions without sending keystrokes.
    #[arg(long)]
    simulate: bool,

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

    if let Some(ref sw) = args.software {
        config.software = sw.clone();
    }

    info!("Software: {}", config.software);
    info!("Tiers enabled: {:?}", config.tiers_enabled);

    // Load commands YAML
    let commands_yaml =
        std::fs::read_to_string(&args.commands).context("Failed to read commands file")?;

    // Read API key
    let api_key =
        std::env::var("GEMINI_API_KEY").context("GEMINI_API_KEY environment variable not set")?;

    // Build DeckController
    let deck = Arc::new(Mutex::new(DeckController::new(config.clone())));

    // Build VoiceController
    let mut voice = VoiceController::new(&api_key, config);

    voice
        .load_commands(&commands_yaml)
        .context("Failed to load command definitions")?;

    // Wire command callback → DeckController
    let deck_for_cb = deck.clone();
    let simulate = args.simulate;

    voice.on_command(move |cmd| {
        info!("[DJ] Command: {} -> {}", cmd.canonical, cmd.id);

        if simulate {
            info!("[SIMULATE] Would execute: {}", cmd.id);
            return;
        }

        let deck = deck_for_cb.clone();
        let action_name = cmd.id.clone();

        tokio::spawn(async move {
            let mut d = deck.lock().await;
            if let Err(e) = d.execute(&action_name).await {
                tracing::warn!("Execute failed for {}: {}", action_name, e);
            }
        });
    });

    // Start voice pipeline
    voice
        .start()
        .await
        .context("Failed to start voice controller")?;

    info!("cc-dj running. Speak commands. Ctrl+C to stop.");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");
    voice
        .stop()
        .await
        .context("Failed to stop voice controller")?;

    Ok(())
}
