//! gena configuration management for Chatterbox TTS.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// Default values for Chatterbox TTS
const DEFAULT_EXAGGERATION: f32 = 0.5;
const DEFAULT_CFG: f32 = 0.5;
const DEFAULT_TEMPERATURE: f32 = 0.8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenaConfig {
    /// Default voice reference audio path for cloning
    #[serde(default)]
    pub voice_ref: Option<PathBuf>,

    /// Device to use (mps, cuda, cpu). None means auto-detect.
    #[serde(default)]
    pub device: Option<String>,

    /// Expressiveness/exaggeration (0.25-2.0)
    #[serde(default = "default_exaggeration")]
    pub exaggeration: f32,

    /// Pacing/CFG weight (0.0-1.0)
    #[serde(default = "default_cfg")]
    pub cfg: f32,

    /// Temperature for randomness (0.05-5.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Target chunk size for text processing
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
}

fn default_exaggeration() -> f32 {
    DEFAULT_EXAGGERATION
}

fn default_cfg() -> f32 {
    DEFAULT_CFG
}

fn default_temperature() -> f32 {
    DEFAULT_TEMPERATURE
}

fn default_chunk_size() -> usize {
    280
}

impl Default for GenaConfig {
    fn default() -> Self {
        Self {
            voice_ref: None,
            device: None,
            exaggeration: default_exaggeration(),
            cfg: default_cfg(),
            temperature: default_temperature(),
            chunk_size: default_chunk_size(),
        }
    }
}

impl GenaConfig {
    /// Get the config file path: ~/.config/cli-programs/gena.toml
    pub fn config_path() -> Result<PathBuf> {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("cli-programs")
            .join("gena.toml"))
    }

    /// Load config from file, returning default if file doesn't exist
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        let config: GenaConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GenaConfig::default();
        assert_eq!(config.exaggeration, 0.5);
        assert_eq!(config.cfg, 0.5);
        assert_eq!(config.temperature, 0.8);
        assert!(config.voice_ref.is_none());
        assert!(config.device.is_none());
    }

    #[test]
    fn test_config_path() {
        let path = GenaConfig::config_path();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("cli-programs/gena.toml"));
    }

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
voice_ref = "/path/to/voice.wav"
device = "mps"
exaggeration = 0.7
cfg = 0.3
temperature = 1.0
"#;
        let config: GenaConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.voice_ref, Some(PathBuf::from("/path/to/voice.wav")));
        assert_eq!(config.device, Some("mps".to_string()));
        assert_eq!(config.exaggeration, 0.7);
        assert_eq!(config.cfg, 0.3);
        assert_eq!(config.temperature, 1.0);
    }

    #[test]
    fn test_parse_empty_config() {
        let toml_str = "";
        let config: GenaConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.exaggeration, 0.5);
        assert_eq!(config.cfg, 0.5);
        assert_eq!(config.temperature, 0.8);
    }
}
