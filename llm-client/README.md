# llm-client

Shared LLM client library for the cli-programs workspace.

## Overview

This crate provides a unified interface for multiple LLM providers, allowing CLI tools to easily switch between different backends:

- **Claude CLI** - Uses the installed Claude Code CLI (subprocess)
- **Anthropic API** - Direct API calls to Anthropic
- **OpenRouter** - Access to many models via a single API
- **Cerebras** - Fast Llama inference

## Configuration

Configuration is stored at `~/.config/cli-programs/llm.toml`:

```toml
default_preset = "claude-cli"

[presets.claude-cli]
provider = "claude-cli"
model = "sonnet"

[presets.claude-api]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[presets.openrouter-sonnet]
provider = "openrouter"
model = "anthropic/claude-sonnet-4"

[presets.cerebras-llama]
provider = "cerebras"
model = "llama-3.3-70b"

[providers.anthropic]
# API key from ANTHROPIC_API_KEY env var (or override here)

[providers.openrouter]
# API key from OPENROUTER_API_KEY env var

[providers.cerebras]
# API key from CEREBRAS_API_KEY env var
```

## Environment Variables

API keys can be set via environment variables:

- `ANTHROPIC_API_KEY` - For Anthropic API provider
- `OPENROUTER_API_KEY` - For OpenRouter provider
- `CEREBRAS_API_KEY` - For Cerebras provider

## Usage

```rust
use llm_client::{Config, get_provider, LlmRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    let preset = config.get_preset("claude-cli")?;
    let provider = get_provider(preset, config.get_provider_config(&preset.provider))?;

    let request = LlmRequest {
        prompt: "Hello!".to_string(),
        system_prompt: Some("You are helpful.".to_string()),
        max_tokens: None,
        temperature: None,
    };

    let response = provider.complete(request).await?;
    println!("{}", response.content);

    Ok(())
}
```

## Dependencies

This crate uses [reqwest](https://crates.io/crates/reqwest) for HTTP requests to API-based providers.
