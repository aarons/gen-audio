# gen-audio

Convert EPUB files to audiobooks using Chatterbox TTS (neural text-to-speech with voice cloning).

## Features

- **Zero-dependency setup**: Automatically downloads and manages Python, FFmpeg, and TTS dependencies
- **High-quality neural TTS**: Uses Chatterbox from Resemble AI for natural-sounding speech
- **Voice cloning**: Clone any voice from a reference audio file
- **M4B output**: Creates audiobooks with chapter markers
- **Resumable sessions**: Pick up where you left off if interrupted
- **GPU acceleration**: Automatically uses MPS (Apple Silicon), CUDA, or CPU

## Installation

```bash
cargo install --path .
```

Or use the workspace installer:

```bash
cargo run -p update-cli-programs --release
```

## Quick Start

```bash
# Convert an EPUB to audiobook (auto-downloads dependencies on first run)
gen-audio book.epub

# Use a custom voice (clone from audio sample)
gen-audio book.epub --voice reference.wav

# Specify output file
gen-audio book.epub -o audiobook.m4b
```

On first run, gen-audio will download (~2.1 GB total):
- Python 3.11 (~25 MB)
- FFmpeg 7.1 (~30 MB)
- Chatterbox TTS + PyTorch (~2 GB)

All dependencies are stored in `~/.local/share/gen-audio/` and can be removed with `gen-audio uninstall`.

## Usage

```bash
# Basic conversion
gen-audio book.epub

# Use voice cloning
gen-audio book.epub --voice my-voice.wav

# Adjust TTS parameters
gen-audio book.epub --exaggeration 0.7 --cfg 0.5 --temperature 0.8

# Convert specific chapters
gen-audio book.epub --chapters 0-10

# Force GPU device
gen-audio book.epub --device mps    # Apple Silicon
gen-audio book.epub --device cuda   # NVIDIA GPU
gen-audio book.epub --device cpu    # Force CPU
```

### TTS Parameters

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| `--exaggeration` | 0.25-2.0 | 0.5 | Expressiveness/drama |
| `--cfg` | 0.0-1.0 | 0.5 | Pacing/guidance strength |
| `--temperature` | 0.05-5.0 | 0.8 | Randomness in speech |

## Configuration

Configuration is stored at `~/.config/cli-programs/gen-audio.toml`.

```bash
# Show current configuration
gen-audio config show

# Set default voice reference
gen-audio config set-voice ~/voices/narrator.wav

# Set default exaggeration
gen-audio config set-exaggeration 0.6

# Set default CFG/pacing
gen-audio config set-cfg 0.5

# Set default temperature
gen-audio config set-temperature 0.8
```

## Managing Dependencies

```bash
# Show environment info
gen-audio info

# Upgrade Python packages
gen-audio setup --upgrade

# Remove and re-download all dependencies
gen-audio setup --clean

# Uninstall all gen-audio dependencies (~2.5 GB)
gen-audio uninstall

# Also remove Chatterbox models from cache (~1-2 GB)
gen-audio uninstall --include-models
```

### Storage Locations

| Location | Contents |
|----------|----------|
| `~/.local/share/gen-audio/` | Python, FFmpeg, venv, sessions |
| `~/.cache/huggingface/` | Chatterbox model weights (shared) |
| `~/.config/cli-programs/gen-audio.toml` | Configuration |

## How It Works

1. **Bootstrap**: Downloads Python and FFmpeg if not present
2. **Parse**: Extracts chapters and text from EPUB
3. **Chunk**: Splits text into TTS-friendly segments (~280 chars)
4. **Synthesize**: Generates audio using Chatterbox TTS
5. **Assemble**: Combines chunks into M4B with chapter markers

## Requirements

- macOS (Apple Silicon or Intel) or Linux (x86_64 or ARM64)
- ~3 GB disk space for dependencies
- ~2 GB RAM minimum (more for GPU acceleration)
- Internet connection for first-run setup

## Development

gen-audiobook uses PyO3 to embed Python, which requires Python to be available at compile time. The `xtask` tool handles this automatically:

```bash
# Run tests (auto-provisions Python if needed)
cargo xtask test

# Build the project
cargo xtask build --release

# Just provision Python for development
cargo xtask setup

# Run any cargo command with correct Python environment
cargo xtask cargo check
```

The development Python is stored at `target/python-dev/` and is cleaned by `cargo clean`.

## Troubleshooting

### First run is slow
The first synthesis triggers Chatterbox model download (~1-2 GB). Subsequent runs are faster.

### Out of memory
Try using CPU instead of GPU: `gen-audio book.epub --device cpu`

### Voice sounds robotic
Increase exaggeration: `gen-audio book.epub --exaggeration 0.8`

### Audio has artifacts
Try lowering temperature: `gen-audio book.epub --temperature 0.5`
