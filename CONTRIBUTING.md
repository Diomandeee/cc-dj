# Contributing to cc-dj

Thanks for your interest in contributing!

## Getting Started

1. Fork and clone the repo
2. Install Rust 1.75+ via [rustup](https://rustup.rs)
3. On Linux, install ALSA dev libs: `sudo apt install libasound2-dev`
4. Run `cargo build` to verify your setup

## Development Workflow

```bash
# Build
cargo build

# Run tests
cargo test

# Lint
cargo clippy --all-targets -- -D warnings

# Format
cargo fmt
```

## Pull Requests

- Create a feature branch from `main`
- Keep PRs focused on a single change
- Ensure `cargo test`, `cargo clippy`, and `cargo fmt --check` pass
- Add tests for new functionality
- Update `CHANGELOG.md` under the `[Unreleased]` section

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the crate dependency graph and design overview.

## Code Style

- Follow standard Rust conventions
- Use `tracing` macros (`info!`, `debug!`, `warn!`) for logging
- Prefer returning `Result` over panicking
- Add doc comments (`///`) to all public items

## Reporting Issues

Open an issue on GitHub with:
- Steps to reproduce
- Expected vs actual behavior
- Rust version (`rustc --version`)
- OS and DJ software version

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
