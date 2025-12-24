# Gen Audio

This is a TTS program for generating audio from epub and other texts.

## Build Commands

Use make to build the project

## Workspace Structure

- **gen-audiobook**: EPUB to audiobook converter using Chatterbox TTS. Embeds Python via PyO3 for TTS synthesis. Supports local and distributed processing.
- **bookworm**: CLI tool that renames EPUB files using LLM-generated clean filenames.
- **llm-client**: Shared library providing unified interface for multiple LLM providers (Claude CLI, Anthropic API, OpenRouter, Cerebras).
- **xtask**: Development task runner that provisions Python for PyO3 compilation.

