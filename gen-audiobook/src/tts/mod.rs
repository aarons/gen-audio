//! TTS backend trait and types.

pub mod chatterbox;

use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Options for TTS synthesis with Chatterbox.
#[derive(Debug, Clone)]
pub struct TtsOptions {
    /// Path to voice reference audio for cloning
    pub voice_ref: Option<PathBuf>,
    /// Expressiveness/exaggeration (0.25-2.0, default 0.5)
    /// Higher values = more dramatic/emotional
    pub exaggeration: f32,
    /// Pacing/CFG weight (0.0-1.0, default 0.5)
    /// Lower values = faster speech
    pub cfg: f32,
    /// Temperature for randomness (0.05-5.0, default 0.8)
    /// Lower values = more consistent/predictable
    pub temperature: f32,
}

impl Default for TtsOptions {
    fn default() -> Self {
        Self {
            voice_ref: None,
            exaggeration: 0.5,
            cfg: 0.5,
            temperature: 0.8,
        }
    }
}

impl TtsOptions {
    /// Create new TTS options with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the voice reference path.
    pub fn with_voice_ref(mut self, path: impl Into<PathBuf>) -> Self {
        self.voice_ref = Some(path.into());
        self
    }

    /// Set the exaggeration level.
    pub fn with_exaggeration(mut self, exaggeration: f32) -> Self {
        self.exaggeration = exaggeration.clamp(0.25, 2.0);
        self
    }

    /// Set the CFG/pacing weight.
    pub fn with_cfg(mut self, cfg: f32) -> Self {
        self.cfg = cfg.clamp(0.0, 1.0);
        self
    }

    /// Set the temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature.clamp(0.05, 5.0);
        self
    }
}

/// TTS backend trait - all TTS engines implement this.
#[async_trait]
pub trait TtsBackend: Send + Sync {
    /// Synthesize text to audio file.
    async fn synthesize(
        &self,
        text: &str,
        output_path: &Path,
        options: &TtsOptions,
    ) -> Result<()>;

    /// Synthesize with retry logic for error handling.
    async fn synthesize_with_retry(
        &self,
        text: &str,
        output_path: &Path,
        options: &TtsOptions,
        max_retries: u32,
    ) -> Result<()>;

    /// Device being used (mps, cuda, cpu).
    fn device(&self) -> &str;
}

/// Create a TTS backend.
///
/// # Arguments
/// * `device` - Device to use: "mps", "cuda", "cpu", or None for auto-detect
/// * `voice_ref` - Optional path to voice reference audio for cloning
pub fn create_backend(
    device: Option<&str>,
    voice_ref: Option<PathBuf>,
) -> Result<Box<dyn TtsBackend>> {
    Ok(Box::new(chatterbox::ChatterboxBackend::new(device, voice_ref)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tts_options_default() {
        let opts = TtsOptions::default();
        assert_eq!(opts.exaggeration, 0.5);
        assert_eq!(opts.cfg, 0.5);
        assert_eq!(opts.temperature, 0.8);
        assert!(opts.voice_ref.is_none());
    }

    #[test]
    fn test_tts_options_builder() {
        let opts = TtsOptions::new()
            .with_exaggeration(0.7)
            .with_cfg(0.3)
            .with_temperature(1.0)
            .with_voice_ref("/path/to/voice.wav");

        assert_eq!(opts.exaggeration, 0.7);
        assert_eq!(opts.cfg, 0.3);
        assert_eq!(opts.temperature, 1.0);
        assert_eq!(opts.voice_ref, Some(PathBuf::from("/path/to/voice.wav")));
    }

    #[test]
    fn test_tts_options_clamping() {
        let opts = TtsOptions::new()
            .with_exaggeration(10.0) // Should clamp to 2.0
            .with_cfg(-1.0) // Should clamp to 0.0
            .with_temperature(100.0); // Should clamp to 5.0

        assert_eq!(opts.exaggeration, 2.0);
        assert_eq!(opts.cfg, 0.0);
        assert_eq!(opts.temperature, 5.0);
    }
}
