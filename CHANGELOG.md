# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-03-02

### Fixed
- **CRITICAL**: AppleScript command injection via unsanitized key input (C1)
- **CRITICAL**: `DeckController::update_beat` now executes scheduled actions instead of only logging (C2)
- API key leakage in WebSocket error messages (H3)
- Blocking `std::process::Command` in async context replaced with `tokio::process::Command` (H1)
- Gemini Live session now closed gracefully on voice controller stop (H2)
- `std::sync::Mutex` in async MIDI path replaced with `tokio::sync::Mutex` (H6)
- RwLock poisoning in voice controller handled gracefully (M8)

### Changed
- `DJConfig.software` changed from `String` to `DJSoftware` enum for type safety (M1)
- `Shortcut` enum now uses internally tagged serialization instead of untagged (M4)
- `execute_parallel` renamed to `execute_batch` to reflect sequential behavior (M6)
- Audio channel buffer increased from 64 to 256 to reduce backpressure drops (M5)
- Rekordbox is now activated before sending keystrokes (M9)
- GoAway messages now log reconnection-needed status (M7)
- `ReflexConfig` and `RewardConfig` annotated as reserved for future RL (M3)

### Security
- Migrated `serde_yaml` (unmaintained) to `serde_yml` (H5a)
- Updated `reqwest` 0.11 to 0.12, `tokio-tungstenite` 0.21 to 0.24 (H5b)
- Hardened `.gitignore` with secret patterns (L7)
- Added `deny.toml` for `cargo-deny` license and duplicate checks (L5)

### Added
- `CONTRIBUTING.md` with development workflow guide (L1)
- This `CHANGELOG.md` (L1)
- `configs/validate_commands.sh` for CI command ID uniqueness check (L3)
- `rust-version = "1.75"` MSRV declaration in workspace (L8)
- Integration test exercising config-to-deck pipeline in simulation mode (M2)

## [0.1.0] - 2026-03-01

### Added
- Initial release: voice-controlled DJ agent for Rekordbox
- Gemini Live API integration for real-time speech recognition
- 6-tier progressive unlock system
- Beat-quantized action scheduling
- Rekordbox and Serato bridge implementations
- 197 mapped DJ commands
- MIDI output support
- Simulation mode for testing
- CI/CD with cross-platform binary releases
