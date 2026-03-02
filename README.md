# cc-dj — Voice-Controlled DJ Agent

Control Rekordbox (or Serato) with your voice using Google's Gemini Live API for real-time speech recognition.

Say "play", "loop 4 bars", or "sync" — and it happens instantly.

## Quick Start

1. Get a [Gemini API key](https://aistudio.google.com/apikey)
2. Download the latest binary from [Releases](https://github.com/diomandeee/cc-dj/releases)
3. Run:

```bash
export GEMINI_API_KEY=your_key_here
./cc-dj
```

4. Open Rekordbox. Speak commands: "play", "pause", "sync", "loop 4 bars"

## Installation

### Download Binary (Recommended)

Grab the latest release for your platform:

| Platform | Binary |
|----------|--------|
| macOS (Apple Silicon) | [cc-dj-aarch64-apple-darwin.tar.gz](https://github.com/diomandeee/cc-dj/releases/latest) |
| macOS (Intel) | [cc-dj-x86_64-apple-darwin.tar.gz](https://github.com/diomandeee/cc-dj/releases/latest) |
| Linux (x86_64) | [cc-dj-x86_64-unknown-linux-gnu.tar.gz](https://github.com/diomandeee/cc-dj/releases/latest) |
| Windows (x86_64) | [cc-dj-x86_64-pc-windows-msvc.zip](https://github.com/diomandeee/cc-dj/releases/latest) |

### Build from Source

```bash
git clone https://github.com/diomandeee/cc-dj
cd cc-dj
cargo build --release
./target/release/cc-dj
```

Linux requires ALSA dev libs:

```bash
sudo apt install libasound2-dev
```

## CLI Options

```
Usage: cc-dj [OPTIONS]

Options:
  --config <PATH>       Custom config file (default: configs/dj.yaml)
  --commands <PATH>     Custom commands file (default: configs/commands.yaml)
  --software <NAME>     Override DJ software: rekordbox or serato
  --simulate            Dry run without sending keystrokes
  --log-level <LEVEL>   trace / debug / info / warn / error (default: info)
  -h, --help            Print help
  -V, --version         Print version
```

### Examples

```bash
# Default Rekordbox mode
cc-dj

# Serato mode
cc-dj --software serato

# Dry run (prints commands without executing)
cc-dj --simulate

# Verbose logging
cc-dj --log-level debug

# Custom config
cc-dj --config ~/my-dj-config.yaml
```

## Supported Commands

Voice commands are defined in `configs/commands.yaml`. Highlights:

| Category | Commands |
|----------|----------|
| **Transport** | play, pause, cue, stop |
| **Sync** | sync, sync master |
| **Looping** | loop 4 bars, loop 8 bars, loop 16, halve loop, double loop |
| **Hot Cues** | hot cue A/B/C/D, set hot cue, delete hot cue |
| **Effects** | echo, reverb, flanger, filter |
| **Mixer** | volume up/down, bass up/down, treble up/down |
| **Library** | next track, load track, browse |
| **Layout** | zoom in, zoom out |

Full command list with synonyms: see [configs/commands.yaml](configs/commands.yaml).

## Configuration

The main config file (`configs/dj.yaml`) controls:

- **DJ software**: `rekordbox` or `serato`
- **Tier system**: Progressive unlock of command categories (Transport → Looping → Cues → FX → Library → Blend)
- **Safety**: Lock playing deck, prevent loading on live deck, EQ rate limiting
- **Voice**: Engine selection, custom voice-to-command mappings, VAD settings
- **Keyboard mappings**: Per-software shortcut definitions

## How It Works

```
Microphone → Gemini Live API → Speech Text → Intent Processor → DJ Command → Keystroke
```

1. **Mic capture** (`cpal`): Captures audio from your default input device at 16kHz mono
2. **Gemini Live** (`cc-gemini`): Streams audio to Google's Gemini Live API via WebSocket for real-time transcription
3. **Intent processing**: Matches transcribed text to DJ commands using exact match, synonym lookup, and semantic similarity
4. **Command orbiter**: Requires 3+ consecutive matches above a confidence threshold before executing (prevents accidental triggers)
5. **Beat-quantized execution**: Actions are scheduled to fire on the next beat boundary
6. **Bridge**: Sends keyboard shortcuts (or MIDI) to Rekordbox/Serato

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full technical breakdown.

```
cc-dj-live (binary)
  ├── cc-dj-voice (Gemini Live + mic capture)
  │     ├── cc-gemini (WebSocket client, vendored)
  │     └── cc-dj-types
  ├── cc-dj-control (Rekordbox/Serato bridges)
  │     └── cc-dj-types
  ├── cc-dj-auto (auto-mixing logic)
  │     └── cc-dj-types
  └── cc-dj-gesture (IMU gesture recognition)
        └── cc-dj-types
```

## Requirements

- **Gemini API key** (free tier works)
- **macOS, Linux, or Windows**
- **Rekordbox** or **Serato** running
- **Microphone** connected

## License

MIT — see [LICENSE](LICENSE).
