# Gen Audio

EPUB to audiobook converter using distributed TTS workers.

## Build & Run

```bash
make              # Build coordinator + Docker worker image
make run-worker   # Start local worker (CPU)
make help         # Show all targets
```

## Architecture

- **gen-audiobook** (Rust): Coordinator - EPUB parsing, job scheduling, SSH/SFTP to workers, FFmpeg assembly
- **gen-audio-worker** (Python): TTS worker - runs on GPU machines, uses Chatterbox TTS
- **bookworm**: CLI tool for renaming EPUBs via LLM
- **llm-client**: Shared LLM provider interface

Workers communicate via SSH. Coordinator sends jobs as JSON, worker writes results to files, coordinator fetches via SFTP.

