# gen-audio

Convert EPUB files to audiobooks using distributed TTS workers with Chatterbox (neural text-to-speech with voice cloning).

## Features

- **Distributed processing**: Offload TTS to GPU workers via SSH
- **Docker-based workers**: Easy deployment locally or on cloud GPUs (vast.ai, etc.)
- **High-quality neural TTS**: Uses Chatterbox from Resemble AI for natural-sounding speech
- **Voice cloning**: Clone any voice from a reference audio file
- **M4B output**: Creates audiobooks with chapter markers
- **Resumable sessions**: Pick up where you left off if interrupted

## Quick Start

### 1. Build everything

```bash
make
```

This builds:
- `./gen-audio` - the coordinator binary
- `gen-audio-worker` - Docker image with TTS engine

### 2. Start a local worker

```bash
make run-worker
```

This starts a Docker container with SSH access on port 2222.

### 3. Add the worker

```bash
./gen-audio workers add local localhost -p 2222
./gen-audio workers test local
# → OK (device: cpu, ready: yes)
```

### 4. Convert an EPUB

```bash
./gen-audio book.epub
```

### 5. Stop the worker when done

```bash
make stop-worker
```

## Requirements

- Docker
- FFmpeg (`brew install ffmpeg` or `apt install ffmpeg`)
- SSH key (`~/.ssh/id_ed25519.pub` or `~/.ssh/id_rsa.pub`)

## Makefile Targets

| Target | Description |
|--------|-------------|
| `make` | Build coordinator and Docker image |
| `make build` | Build only the coordinator (`./gen-audio`) |
| `make docker` | Build CPU Docker image |
| `make docker-gpu` | Build GPU Docker image (CUDA) |
| `make run-worker` | Start local worker (CPU) |
| `make run-worker-gpu` | Start local worker (GPU) |
| `make stop-worker` | Stop the worker container |
| `make logs` | View worker logs |
| `make test` | Run tests |
| `make clean` | Clean build artifacts |

## Usage

```bash
# Basic conversion
./gen-audio book.epub

# Use voice cloning
./gen-audio book.epub --voice my-voice.wav

# Adjust TTS parameters
./gen-audio book.epub --exaggeration 0.7 --cfg 0.5 --temperature 0.8

# Convert specific chapters
./gen-audio book.epub --chapters 0-10

# Specify output file
./gen-audio book.epub -o audiobook.m4b
```

### TTS Parameters

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| `--exaggeration` | 0.25-2.0 | 0.5 | Expressiveness/drama |
| `--cfg` | 0.0-1.0 | 0.5 | Pacing/guidance strength |
| `--temperature` | 0.05-5.0 | 0.8 | Randomness in speech |

## Worker Management

```bash
# Add a worker
./gen-audio workers add <name> <host> [-p <port>] [-u <user>]

# List workers
./gen-audio workers list

# Test connection
./gen-audio workers test [name]

# Remove a worker
./gen-audio workers remove <name>
```

### Monitoring Workers

```bash
# View container logs
make logs

# Or directly
docker logs -f gen-audio-worker

# Check worker status via SSH
ssh -p 2222 root@localhost "gen-audio-worker status"
```

### Worker Configuration

Workers are stored in `workers.toml` in the current directory:

```toml
[[workers]]
name = "local"
host = "localhost"
port = 2222
user = "root"
priority = 1
```

## Deploying to vast.ai

1. Create a vast.ai instance with the `gen-audio-worker` Docker image
2. Add it as a worker:
   ```bash
   ./gen-audio workers add vast <instance-ip> -p <ssh-port> -u root
   ```
3. Test the connection:
   ```bash
   ./gen-audio workers test vast
   ```

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│           Rust Coordinator (gen-audio)                   │
│  - EPUB parsing, text chunking, job scheduling           │
│  - SSH connection pooling, SFTP file transfer            │
│  - Audio assembly (FFmpeg)                               │
└─────────────────────────┬────────────────────────────────┘
                          │ SSH/SFTP
                          ▼
┌──────────────────────────────────────────────────────────┐
│           Docker Worker (gen-audio-worker)               │
│  - SSH server for remote access                          │
│  - Chatterbox TTS with voice cloning                     │
│  - GPU acceleration (CUDA) or CPU fallback               │
└──────────────────────────────────────────────────────────┘
```

## How It Works

1. **Parse**: Extracts chapters and text from EPUB
2. **Chunk**: Splits text into TTS-friendly segments (~280 chars)
3. **Dispatch**: Sends jobs to workers via SSH (JSON stdin)
4. **Synthesize**: Workers generate audio with Chatterbox
5. **Download**: Retrieves audio files via SFTP
6. **Assemble**: Combines chunks into M4B with chapter markers (FFmpeg)

## Troubleshooting

### No workers configured

```
./gen-audio book.epub
# Error: No workers configured.
#
# Quick start (local Docker worker):
#   make run-worker
#   ./gen-audio workers add local localhost -p 2222
```

### Worker not ready

Check worker logs:
```bash
make logs
```

### First synthesis is slow

The first run downloads Chatterbox model weights (~1-2 GB). Subsequent runs use cached weights.

### Connection refused

Ensure the worker container is running:
```bash
docker ps | grep gen-audio-worker
```

### Voice sounds robotic

Increase exaggeration: `./gen-audio book.epub --exaggeration 0.8`

### Audio has artifacts

Try lowering temperature: `./gen-audio book.epub --temperature 0.5`
