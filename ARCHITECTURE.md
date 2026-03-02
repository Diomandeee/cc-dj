# Architecture

cc-dj is a multi-crate Rust workspace organized around a clear data flow: microphone audio in, DJ software commands out.

## Crate Dependency Graph

```
cc-dj-live (binary — CLI entry point)
  │
  ├── cc-dj-voice (voice pipeline)
  │     ├── cc-gemini (Gemini Live API WebSocket client)
  │     │     └── tokio-tungstenite 0.24, reqwest 0.12, serde
  │     ├── cc-dj-types (shared types)
  │     └── cpal (audio capture)
  │
  ├── cc-dj-control (DJ software bridge)
  │     ├── cc-dj-types
  │     └── midir (MIDI)
  │
  ├── cc-dj-auto (auto-mixing engine)
  │     ├── cc-dj-control
  │     └── cc-dj-types
  │
  └── cc-dj-gesture (gesture recognition)
        └── cc-dj-types
```

## Data Flow

```
┌──────────┐    PCM audio     ┌─────────────┐   WebSocket    ┌──────────────┐
│ cpal Mic │ ──────────────── │ cc-dj-voice │ ────────────── │ Gemini Live  │
│ (16kHz)  │                  │ controller  │                │  API         │
└──────────┘                  └──────┬──────┘                └──────┬───────┘
                                     │                              │
                              transcribed text              speech recognition
                                     │                              │
                              ┌──────▼──────┐                       │
                              │   Intent    │ ◄─────────────────────┘
                              │  Processor  │
                              └──────┬──────┘
                                     │
                              matched command
                                     │
                              ┌──────▼──────┐
                              │  Command    │   3+ consecutive
                              │  Orbiter    │   matches required
                              └──────┬──────┘
                                     │
                              confirmed command
                                     │
                              ┌──────▼──────┐    keystroke/MIDI    ┌───────────┐
                              │ cc-dj-      │ ──────────────────── │ Rekordbox │
                              │ control     │                      │ / Serato  │
                              └─────────────┘                      └───────────┘
```

## Crate Responsibilities

### cc-dj-types

Shared type definitions used by every other crate. Serialization via `serde` + `serde_yml`.

- **DJSoftware**: Enum (`Rekordbox`, `Serato`) — validated at parse time, no stringly-typed config
- **Command**: DJ command definition — canonical name, synonyms, category, deck, action type, keyboard shortcut, safety flags
- **Shortcut**: Tagged enum (`Key`, `KeyCombo`, `Sequence`, `Midi`) with custom deserializer accepting both bare strings (`shortcut: Z`) and tagged objects (`shortcut: { type: key, key: Z }`)
- **CommandCatalog**: Loads command definitions from YAML, provides exact match + synonym matching
- **Action**: Executable action with tier, quantization window, cooldown, parameters
- **ActionSpace**: Tier-masked action set with cooldown tracking
- **Tier**: 6 progressive unlock levels — Transport (0), Looping (1), Cues (2), FX (3), Library (4), Blend (5)
- **DeckState / SessionState**: Real-time state tracking for decks, mixer, and session
- **DJConfig**: Full configuration with safety, voice, reflex, and reward settings
- **DJError**: Comprehensive error enum with all failure modes

### cc-dj-voice

The voice-to-intent pipeline. Captures microphone audio, streams to Gemini Live API, and maps transcribed text to commands.

- **VoiceController**: Main orchestrator — manages Gemini session lifecycle (including graceful close on shutdown), mic capture, and intent processing. Holds an `Arc<LiveSession>` for clean WebSocket teardown. Uses a callback to dispatch recognized commands. Audio channel buffered at 256 frames to absorb network latency.
- **IntentProcessor**: Loads the command catalog from YAML and matches transcribed text to commands. Supports exact match, synonym matching, and custom voice mappings from config. Protected by `RwLock` with poison recovery.
- **SemanticMatcher**: Embedding-based cosine similarity for fuzzy command matching when exact/synonym match fails.
- **CommandOrbiter**: Stability-aware command retrieval. Requires 3+ consecutive matches above a confidence threshold before dispatching. Prevents accidental command execution from ambient noise or partial speech.
- **mic**: `cpal`-based microphone capture. Opens the default input device, negotiates 16kHz mono format, converts f32 samples to 16-bit LE PCM for Gemini.

### cc-dj-control

Bridges between recognized commands and DJ software. Handles both keyboard automation and MIDI output.

- **DeckController**: High-level action executor. Validates tier permissions, checks cooldowns, optionally quantizes to the beat grid, and dispatches to the bridge. Bridge is held as `Arc<dyn DJBridge>` for safe sharing across async tasks.
- **ActionScheduler**: Beat-quantized scheduling queue. Actions are scheduled for a target beat and executed when the current beat falls within the quantization window.
- **ChainExecutor**: Sequential batch execution of multi-step action chains with configurable inter-action delay (`execute_batch`).
- **DJBridge** (trait): Async interface for DJ software communication. `execute()`, `is_available()`, `send_key()`, `send_midi()`.
- **RekordboxBridge**: macOS AppleScript keyboard automation + MIDI via `tokio::sync::Mutex`. Activates Rekordbox before sending keystrokes. Input keys validated against injection (max 2 ASCII chars). Process detection via async `pgrep`.
- **SeratoBridge**: Same pattern for Serato DJ.
- Both bridges support a **simulate** mode for dry-run testing without a running DJ application.

### cc-dj-auto

Automated DJ features — track analysis, transition recommendations, and energy management.

- **AutoMixer**: State machine (Idle → Playing → Preparing → Transitioning → Paused) that monitors playback and auto-triggers transitions.
- **TransitionAdvisor**: Analyzes section boundaries, key compatibility (Camelot wheel), and energy contours to recommend transition style and timing.
- **TrackAnalyzer**: Loads track features from JSON sidecar files or cache. Extracts BPM, key, energy, danceability, section markers, and mix points.
- **MixStrategy**: Configurable mixing presets — default, minimal, club, lounge. Controls transition duration, auto-sync, harmonic mixing, and preferred transition styles.
- **TransitionStyle**: Cut, Fade, EQ Swap, Echo Out, Backspin, Loop Fade.

### cc-dj-gesture

Gesture recognition for motion-based DJ control (basic implementation).

- **GestureRecognizer**: Buffer-based gesture detection using acceleration magnitude similarity matching.
- **GestureDatabase**: Persistent storage for learned gestures (JSON sidecar).
- **GestureTrainer**: Record and train custom gestures with duration validation.
- **DJGestureRecognizer**: Maps recognized gestures to DJ commands.
- **GestureCommandMapping**: Configurable gesture-to-command mapping.

### cc-gemini (vendored)

Production-grade Gemini API client. Used by cc-dj-voice for the Live API, but the crate provides the full Gemini API surface. API keys are sanitized in all error messages to prevent leakage via logs or crash dumps.

- **GeminiClient**: HTTP API client (`reqwest` 0.12) with rate limiting, cost tracking, and retry logic.
- **BatchClient**: Batch API support for offline processing.
- **LiveSession**: WebSocket-based Live API session (`tokio-tungstenite` 0.24) for real-time audio streaming.
- **RateLimiter**: Dual-bucket (RPM + TPM) token bucket rate limiter.
- **CostTracker**: Thread-safe cost accounting with configurable limits.
- **VAD config**: Voice Activity Detection configuration for the Live API.

## Safety System

The tier system prevents dangerous actions:

| Tier | Category | Risk |
|------|----------|------|
| 0 | Transport | Low — play, pause, cue |
| 1 | Looping | Low — loop controls |
| 2 | Cues | Medium — hot cue set/delete |
| 3 | FX | Medium — effects, EQ |
| 4 | Library | High — track loading |
| 5 | Blend | High — crossfader, transitions |

Additional safety constraints:
- **Lock playing deck**: Cannot STOP a live deck
- **Forbid load on live**: Cannot load a track while the deck is playing
- **EQ rate limiting**: Maximum 6dB change per beat
- **Crossfader slope**: Maximum 0.25 change per beat
- **Cooldown tracking**: Per-action beat-based cooldowns (e.g., 16 beats for track load)
- **AppleScript injection prevention**: Key values validated (max 2 ASCII chars, no injection payloads)

## Testing

```bash
cargo test           # 136 tests (unit + integration)
cargo clippy -- -D warnings  # Zero warnings
cargo fmt --check    # Format check
```

Integration tests in `crates/cc-dj-control/tests/` exercise the config → deck → bridge pipeline in simulation mode.

## Building

```bash
cargo build --release
```

Linux requires ALSA:

```bash
sudo apt install libasound2-dev
```

Minimum supported Rust version: **1.75** (declared in `Cargo.toml`).

The binary is at `target/release/cc-dj`.
