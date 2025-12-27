# Gen Audio

Convert EPUB books to audiobooks using distributed TTS workers.

## Quick Start

```bash
# Build everything
make

# Start local worker (requires Docker)
make run-worker

# Convert a book
./gen-audio ~/Documents/books/mybook.epub
```

## Development

### Prerequisites

- Rust toolchain
- Docker
- SSH key (`~/.ssh/id_ed25519.pub` or `~/.ssh/id_rsa.pub`)
- FFmpeg (for audio assembly)

### Build Commands

```bash
make              # Build coordinator binary + CPU Docker image
make build        # Build coordinator only (./gen-audio)
make docker       # Build CPU Docker image
make docker-gpu   # Build GPU Docker image
make test         # Run tests
make clean        # Clean build artifacts
```

### Worker Management

```bash
make run-worker      # Start local CPU worker (port 2222)
make run-worker-gpu  # Start local GPU worker
make stop-worker     # Stop worker
make logs            # View worker logs
```

### Updating Workers

After changing `gen-audio-worker/` code:

```bash
make docker stop-worker run-worker
```

### Worker Configuration

Workers are configured in `workers.toml`:

```toml
[defaults]
max_concurrent_jobs = 6

[[workers]]
name = "local"
host = "localhost"
port = 2222
user = "root"
```

## Architecture

```
┌─────────────────────────────────────────────┐
│         Coordinator (gen-audiobook)         │
│  Rust binary that orchestrates everything   │
└──────────────────────┬──────────────────────┘
                       │ SSH/SFTP
                       ▼
┌─────────────────────────────────────────────┐
│         Worker (gen-audio-worker)           │
│  Python TTS using Chatterbox, runs in       │
│  Docker locally or on remote GPU machines   │
└─────────────────────────────────────────────┘
```

### Communication Protocol

1. Coordinator sends TTS job as JSON via SSH stdin
2. Worker synthesizes audio, writes to `~/.gen-audio/worker/output/`
3. Worker writes result JSON to `~/.gen-audio/worker/results/`
4. Worker prints result file path to stdout
5. Coordinator fetches result + audio via SSH/SFTP

### Project Structure

- `gen-audiobook/` - Rust coordinator (EPUB parsing, scheduling, assembly)
- `gen-audio-worker/` - Python worker package + Docker setup
- `bookworm/` - EPUB renaming CLI tool
- `llm-client/` - Shared LLM provider library
