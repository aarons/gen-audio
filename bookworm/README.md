# bookworm

CLI tool to clean and standardize epub filenames using AI.

## Overview

`bookworm` iterates through epub files in a directory and renames them to a clean, standardized format using an LLM. This is useful for organizing ebook collections that have messy filenames from various sources.

## Output Format

**For series books:**
```
[Series Name] [Series Number] - [Book Title] - [Author Name].epub
```

**For standalone books:**
```
[Book Title] - [Author Name].epub
```

## Examples

```
Input:  "Furies of Calderon -- Butcher, Jim -- Codex Alera 1, 2011 -- Penguin..."
Output: "Codex Alera 1 - Furies of Calderon - Jim Butcher.epub"

Input:  "How to Survive a Horror Story -- Mallory Arnold -- Sourcebooks..."
Output: "How to Survive a Horror Story - Mallory Arnold.epub"

Input:  "The Travelling Cat Chronicles The most uplifting -- Arikawa, Hiro..."
Output: "The Travelling Cat Chronicles - Hiro Arikawa.epub"
```

## Installation

```bash
cargo install --path bookworm
```

## Usage

```bash
# Process epub files in current directory
bookworm

# Process epub files in a specific directory
bookworm --dir /path/to/ebooks

# Process epub files recursively in subdirectories
bookworm --dir /path/to/ebooks --recursive

# Use a specific LLM preset
bookworm --model claude-api

# Enable debug output
bookworm --debug
```

## Configuration

bookworm uses the shared LLM configuration at `./config/llm.toml` (in the project root).

### Managing Presets

```bash
# List available presets
bookworm config list

# Show current configuration
bookworm config show

# Set default preset
bookworm config set-default claude-api

# Add a new preset
bookworm config add-preset my-preset -p anthropic -M claude-sonnet-4-20250514
```

### Available Providers

- `claude-cli` - Uses installed Claude CLI (default, no API key needed)
- `anthropic` - Direct Anthropic API (requires `ANTHROPIC_API_KEY`)
- `openrouter` - OpenRouter API (requires `OPENROUTER_API_KEY`)
- `cerebras` - Cerebras API (requires `CEREBRAS_API_KEY`)

## Behavior

- **Auto-rename**: Files are renamed immediately without confirmation
- **Conflict handling**: If a target filename exists, adds numeric suffix `(1)`, `(2)`, etc.
- **Skip clean files**: Files that already match the clean format are skipped
- **Error handling**: Errors on individual files are logged but processing continues
