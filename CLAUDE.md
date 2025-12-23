# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

This is a Rust workspace using PyO3 for Python embedding. Due to PyO3's compile-time Python dependency, use `cargo xtask` for gen-audiobook development:

```bash
# Build and test gen-audiobook (provisions Python automatically)
cargo xtask test                  # Run tests
cargo xtask build --release       # Build release binary
cargo xtask cargo check           # Run any cargo command with Python env

# Build other crates (no special setup needed)
cargo build -p bookworm --release
cargo build -p llm-client --release
```

## Architecture

### Workspace Structure

- **gen-audiobook** (`gena`): EPUB to audiobook converter using Chatterbox TTS. Embeds Python via PyO3 for TTS synthesis. Supports local and distributed processing.
- **bookworm**: CLI tool that renames EPUB files using LLM-generated clean filenames.
- **llm-client**: Shared library providing unified interface for multiple LLM providers (Claude CLI, Anthropic API, OpenRouter, Cerebras).
- **xtask**: Development task runner that provisions Python for PyO3 compilation.

### gen-audiobook Modules

- `bootstrap/`: Auto-downloads Python, FFmpeg, and TTS dependencies to `~/.local/share/gena/`
- `tts/`: Chatterbox TTS backend using PyO3-embedded Python
- `text/`: Text processing pipeline (cleaner → chunker → seams for natural chunk boundaries)
- `session/`: Resumable session management for interrupted conversions
- `audio/`: FFmpeg-based audio assembly into M4B with chapter markers
- `coordinator/`: Distributed processing orchestration with remote workers via SSH
- `worker/`: Remote worker protocol for distributed TTS jobs

### llm-client Providers

Configuration at `~/.config/cli-programs/llm.toml`. Providers:
- `claude-cli`: Subprocess to installed Claude CLI (no API key)
- `anthropic`: Direct API with `ANTHROPIC_API_KEY`
- `openrouter`: OpenRouter API with `OPENROUTER_API_KEY`
- `cerebras`: Cerebras API with `CEREBRAS_API_KEY`

## Key Patterns

- PyO3 requires Python at compile time. The `xtask` tool provisions Python to `target/python-dev/` and sets `PYO3_PYTHON` and `LIBRARY_PATH`.
- gen-audiobook uses auto-bootstrapping: first run downloads all dependencies without user intervention.
- Sessions persist to allow resuming interrupted conversions. State stored in `~/.local/share/gena/sessions/`.
- Text chunking targets ~280 characters with natural sentence boundaries for optimal TTS quality.
