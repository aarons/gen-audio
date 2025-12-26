# Gen Audio

This is a TTS program for generating audio from epub and other texts.

## Build Commands

Use `cargo build` to build the project.

## Workspace Structure

- **gen-audiobook**: EPUB to audiobook converter. Coordinates distributed TTS workers via SSH/SFTP. Handles text chunking, job scheduling, and M4B assembly with FFmpeg.
- **gen-audio-worker**: Python TTS worker package. Runs on GPU machines (vast.ai, etc.) and synthesizes audio using Chatterbox TTS. Communicates via SSH stdin/stdout.
- **bookworm**: CLI tool that renames EPUB files using LLM-generated clean filenames.
- **llm-client**: Shared library providing unified interface for multiple LLM providers (Claude CLI, Anthropic API, OpenRouter, Cerebras).

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│           Rust Coordinator (gen-audiobook)               │
│  - EPUB parsing, text chunking, job scheduling           │
│  - SSH connection pooling, SFTP file transfer            │
│  - Audio assembly (FFmpeg)                               │
└─────────────────────────┬────────────────────────────────┘
                          │ SSH/SFTP
                          ▼
┌──────────────────────────────────────────────────────────┐
│           Python Worker (gen-audio-worker)               │
│  - gen-audio-worker status  → JSON to stdout             │
│  - gen-audio-worker run     → JSON stdin, file output    │
└──────────────────────────────────────────────────────────┘
```

## Worker Setup (vast.ai or any SSH-accessible GPU)

```bash
pip install gen-audio-worker[chatterbox]
```

The coordinator connects via SSH and runs `gen-audio-worker run` for each chunk.

