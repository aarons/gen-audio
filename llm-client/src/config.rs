use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{LlmError, Result};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default preset to use when no --model flag is provided (fallback)
    #[serde(default = "default_preset")]
    pub default_preset: String,

    /// Per-program default presets (program name -> preset name)
    #[serde(default)]
    pub defaults: HashMap<String, String>,

    /// Named model presets for quick access
    #[serde(default)]
    pub presets: HashMap<String, ModelPreset>,

    /// Provider-specific configuration
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

fn default_preset() -> String {
    "claude-cli".to_string()
}

/// A named model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreset {
    /// Provider identifier (claude-cli, anthropic, openrouter, cerebras)
    pub provider: String,

    /// Model name/identifier for the provider
    pub model: String,
}

/// Provider-specific configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// API key (optional, can use env var instead)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Path to CLI binary (for claude-cli provider)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_path: Option<PathBuf>,

    /// Custom base URL (for API providers)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Config {
    /// Load configuration from the default location
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// Get the configuration file path
    pub fn config_path() -> Result<PathBuf> {
        let home =
            std::env::var("HOME").map_err(|_| LlmError::ConfigError("HOME not set".into()))?;
        Ok(PathBuf::from(home).join(".config/cli-programs/llm.toml"))
    }

    /// Get a preset by name
    pub fn get_preset(&self, name: &str) -> Result<&ModelPreset> {
        self.presets
            .get(name)
            .ok_or_else(|| LlmError::InvalidPreset(name.to_string()))
    }

    /// Get the default preset name for a specific program
    ///
    /// Falls back to `default_preset` if no program-specific default is set.
    pub fn get_default_for_program(&self, program: &str) -> &str {
        self.defaults
            .get(program)
            .map(String::as_str)
            .unwrap_or(&self.default_preset)
    }

    /// Get provider config by provider name
    pub fn get_provider_config(&self, provider: &str) -> Option<&ProviderConfig> {
        self.providers.get(provider)
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut presets = HashMap::new();

        // Default preset: claude-cli with sonnet model (matches current gc behavior)
        presets.insert(
            "claude-cli".to_string(),
            ModelPreset {
                provider: "claude-cli".to_string(),
                model: "sonnet".to_string(),
            },
        );

        Self {
            default_preset: "claude-cli".to_string(),
            defaults: HashMap::new(),
            presets,
            providers: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.default_preset, "claude-cli");
        assert!(config.presets.contains_key("claude-cli"));

        let preset = config.get_preset("claude-cli").unwrap();
        assert_eq!(preset.provider, "claude-cli");
        assert_eq!(preset.model, "sonnet");
    }

    #[test]
    fn test_invalid_preset() {
        let config = Config::default();
        let result = config.get_preset("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.default_preset, config.default_preset);
    }

    #[test]
    fn test_config_path() {
        let path = Config::config_path().unwrap();
        assert!(
            path.to_string_lossy()
                .contains(".config/cli-programs/llm.toml")
        );
    }

    #[test]
    fn test_get_default_for_program() {
        let mut config = Config::default();

        // Without program-specific default, should fall back to default_preset
        assert_eq!(config.get_default_for_program("gc"), "claude-cli");
        assert_eq!(config.get_default_for_program("ask"), "claude-cli");

        // Add program-specific defaults
        config
            .defaults
            .insert("gc".to_string(), "anthropic-sonnet".to_string());
        config
            .defaults
            .insert("ask".to_string(), "qwen3".to_string());

        // Should now return program-specific defaults
        assert_eq!(config.get_default_for_program("gc"), "anthropic-sonnet");
        assert_eq!(config.get_default_for_program("ask"), "qwen3");

        // Unknown program should still fall back
        assert_eq!(config.get_default_for_program("bookname"), "claude-cli");
    }
}
