//! LLM provider implementations

mod anthropic;
mod claude_cli;
pub mod mock;
mod openai_compatible;

pub use anthropic::AnthropicProvider;
pub use claude_cli::ClaudeCliProvider;
pub use mock::MockProvider;
pub use openai_compatible::OpenAICompatibleProvider;

use crate::config::{ModelPreset, ProviderConfig};
use crate::error::{LlmError, Result};
use crate::provider::LlmProvider;

/// Supported provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    ClaudeCli,
    Anthropic,
    OpenRouter,
    Cerebras,
}

impl ProviderKind {
    /// Parse provider kind from string
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "claude-cli" | "claude_cli" | "claudecli" => Ok(Self::ClaudeCli),
            "anthropic" => Ok(Self::Anthropic),
            "openrouter" => Ok(Self::OpenRouter),
            "cerebras" => Ok(Self::Cerebras),
            _ => Err(LlmError::ConfigError(format!("Unknown provider: {}", s))),
        }
    }

    /// Get the environment variable name for this provider's API key
    pub fn env_var(&self) -> Option<&'static str> {
        match self {
            Self::ClaudeCli => None,
            Self::Anthropic => Some("ANTHROPIC_API_KEY"),
            Self::OpenRouter => Some("OPENROUTER_API_KEY"),
            Self::Cerebras => Some("CEREBRAS_API_KEY"),
        }
    }
}

/// Create a provider instance from a preset and optional config
pub fn get_provider(
    preset: &ModelPreset,
    provider_config: Option<&ProviderConfig>,
) -> Result<Box<dyn LlmProvider>> {
    let kind = ProviderKind::from_str(&preset.provider)?;

    match kind {
        ProviderKind::ClaudeCli => {
            let cli_path = provider_config.and_then(|c| c.cli_path.clone());
            Ok(Box::new(ClaudeCliProvider::new(&preset.model, cli_path)?))
        }
        ProviderKind::Anthropic => {
            let api_key = get_api_key(provider_config, "ANTHROPIC_API_KEY", "Anthropic")?;
            Ok(Box::new(AnthropicProvider::new(&preset.model, api_key)?))
        }
        ProviderKind::OpenRouter => {
            let api_key = get_api_key(provider_config, "OPENROUTER_API_KEY", "OpenRouter")?;
            Ok(Box::new(OpenAICompatibleProvider::openrouter(
                &preset.model,
                api_key,
            )?))
        }
        ProviderKind::Cerebras => {
            let api_key = get_api_key(provider_config, "CEREBRAS_API_KEY", "Cerebras")?;
            Ok(Box::new(OpenAICompatibleProvider::cerebras(
                &preset.model,
                api_key,
            )?))
        }
    }
}

/// Get API key from config or environment variable
fn get_api_key(
    config: Option<&ProviderConfig>,
    env_var: &str,
    provider_name: &str,
) -> Result<String> {
    // Check config first
    if let Some(key) = config.and_then(|c| c.api_key.clone()) {
        return Ok(key);
    }

    // Fall back to environment variable
    std::env::var(env_var).map_err(|_| LlmError::MissingApiKey {
        provider: provider_name.to_string(),
        env_var: env_var.to_string(),
    })
}
