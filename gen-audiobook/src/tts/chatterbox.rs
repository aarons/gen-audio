//! Chatterbox TTS backend using PyO3 to embed Python.
//!
//! This backend uses Chatterbox TTS from Resemble AI for high-quality voice synthesis.
//! It supports voice cloning from reference audio, expressiveness control, and GPU acceleration.

use super::{TtsBackend, TtsOptions};
use crate::setup;
use anyhow::{Context, Result};
use async_trait::async_trait;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::{Path, PathBuf};
use std::sync::Once;

/// Initialize Python runtime once.
static PYTHON_INIT: Once = Once::new();

/// Chatterbox TTS backend using PyO3.
pub struct ChatterboxBackend {
    /// Device to use (mps, cuda, cpu)
    device: String,
    /// Path to voice reference audio (optional)
    voice_ref: Option<PathBuf>,
    /// Sample rate (retrieved from model)
    sample_rate: u32,
}

impl ChatterboxBackend {
    /// Create a new Chatterbox backend.
    ///
    /// # Arguments
    /// * `device` - Device to use: "mps", "cuda", "cpu", or None for auto-detect
    /// * `voice_ref` - Optional path to voice reference audio for cloning
    pub fn new(device: Option<&str>, voice_ref: Option<PathBuf>) -> Result<Self> {
        // Check if venv is ready
        if !setup::is_venv_ready()? {
            anyhow::bail!(
                "Python virtual environment not ready. Please run 'gen-audio setup' first."
            );
        }

        if !setup::is_chatterbox_installed()? {
            anyhow::bail!(
                "Chatterbox not installed. Please run 'gen-audio setup' first."
            );
        }

        // Initialize Python once
        // Note: PYTHONHOME is set by ensure_python_home() in main.rs via re-exec
        PYTHON_INIT.call_once(|| {
            // Get venv path for later use
            let venv_site_packages = setup::get_python_path().ok().and_then(|python_path| {
                // venv Python: .../venv/bin/python -> site-packages: .../venv/lib/python3.11/site-packages
                let venv_dir = python_path.parent()?.parent()?;
                let site_packages = venv_dir.join("lib").join("python3.11").join("site-packages");
                if site_packages.exists() {
                    Some(site_packages)
                } else {
                    None
                }
            });

            pyo3::prepare_freethreaded_python();

            // Add venv site-packages to sys.path after Python initializes
            if let Some(site_packages) = venv_site_packages {
                let _ = Python::with_gil(|py| -> PyResult<()> {
                    let sys = py.import("sys")?;
                    let path = sys.getattr("path")?;
                    path.call_method1("insert", (0, site_packages.to_string_lossy().as_ref()))?;
                    Ok(())
                });
            }
        });

        // Auto-detect device if not specified
        let device = match device {
            Some(d) => d.to_string(),
            None => Self::detect_device()?,
        };

        Ok(Self {
            device,
            voice_ref,
            sample_rate: 24000, // Chatterbox default
        })
    }

    /// Auto-detect the best available device.
    fn detect_device() -> Result<String> {
        Python::with_gil(|py| {
            // Import torch
            let torch = py.import("torch").context("Failed to import torch")?;

            // Check MPS (Apple Silicon)
            let backends = torch.getattr("backends")?;
            let mps = backends.getattr("mps")?;
            if mps.call_method0("is_available")?.extract::<bool>()? {
                return Ok("mps".to_string());
            }

            // Check CUDA
            let cuda = torch.getattr("cuda")?;
            if cuda.call_method0("is_available")?.extract::<bool>()? {
                return Ok("cuda".to_string());
            }

            // Default to CPU
            Ok("cpu".to_string())
        })
    }

    /// Generate audio using Chatterbox.
    fn generate_audio_sync(
        &self,
        text: &str,
        output_path: &Path,
        options: &TtsOptions,
    ) -> Result<()> {
        Python::with_gil(|py| {
            // Enable MPS fallback
            let os = py.import("os")?;
            let environ = os.getattr("environ")?;
            environ.set_item("PYTORCH_ENABLE_MPS_FALLBACK", "1")?;

            // Import chatterbox
            let chatterbox_tts = py.import("chatterbox.tts")?;
            let chatterbox_class = chatterbox_tts.getattr("ChatterboxTTS")?;

            // Load model
            let kwargs = PyDict::new(py);
            kwargs.set_item("device", &self.device)?;
            let model = chatterbox_class.call_method("from_pretrained", (), Some(&kwargs))?;

            // Prepare generation kwargs
            let gen_kwargs = PyDict::new(py);
            gen_kwargs.set_item("text", text)?;

            // Voice reference for cloning
            let voice_path = options
                .voice_ref
                .as_ref()
                .or(self.voice_ref.as_ref());
            if let Some(voice) = voice_path {
                gen_kwargs.set_item("audio_prompt_path", voice.to_string_lossy().as_ref())?;
            }

            // TTS parameters
            gen_kwargs.set_item("exaggeration", options.exaggeration)?;
            gen_kwargs.set_item("cfg_weight", options.cfg)?;
            gen_kwargs.set_item("temperature", options.temperature)?;

            // Generate audio
            let wav = model.call_method("generate", (), Some(&gen_kwargs))?;

            // Get sample rate from model
            let sample_rate: u32 = model.getattr("sr")?.extract()?;

            // Save audio using soundfile
            let soundfile = py.import("soundfile")?;

            // Convert tensor to numpy
            let wav_cpu = wav.call_method0("cpu")?;
            let wav_np = wav_cpu.call_method0("numpy")?;

            // Handle dimensions - soundfile expects (samples, channels)
            let ndim: i32 = wav_np.getattr("ndim")?.extract()?;
            let wav_np = if ndim == 2 {
                wav_np.getattr("T")?
            } else {
                wav_np
            };

            // Ensure output directory exists
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Save to file
            let write_kwargs = PyDict::new(py);
            soundfile.call_method(
                "write",
                (output_path.to_string_lossy().as_ref(), wav_np, sample_rate),
                Some(&write_kwargs),
            )?;

            // Cleanup memory
            self.cleanup_memory(py)?;

            Ok(())
        })
    }

    /// Cleanup GPU memory to mitigate leaks.
    fn cleanup_memory(&self, py: Python<'_>) -> Result<()> {
        // Import gc and collect
        let gc = py.import("gc")?;
        gc.call_method0("collect")?;

        // Clear MPS cache if using MPS
        if self.device == "mps" {
            let torch = py.import("torch")?;
            let mps = torch.getattr("mps")?;
            if mps.hasattr("empty_cache")? {
                mps.call_method0("empty_cache")?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl TtsBackend for ChatterboxBackend {
    async fn synthesize(
        &self,
        text: &str,
        output_path: &Path,
        options: &TtsOptions,
    ) -> Result<()> {
        // Clone data for the blocking task
        let text = text.to_string();
        let output_path = output_path.to_path_buf();
        let options = options.clone();
        let device = self.device.clone();
        let voice_ref = self.voice_ref.clone();
        let sample_rate = self.sample_rate;

        // Run in a blocking task to not block the tokio runtime
        tokio::task::spawn_blocking(move || {
            let backend = ChatterboxBackend {
                device,
                voice_ref,
                sample_rate,
            };
            backend.generate_audio_sync(&text, &output_path, &options)
        })
        .await
        .context("Task join error")??;

        Ok(())
    }

    async fn synthesize_with_retry(
        &self,
        text: &str,
        output_path: &Path,
        options: &TtsOptions,
        max_retries: u32,
    ) -> Result<()> {
        let mut last_error = None;

        for attempt in 0..max_retries {
            match self.synthesize(text, output_path, options).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    eprintln!(
                        "Generation failed (attempt {}/{}): {}",
                        attempt + 1,
                        max_retries,
                        e
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retry attempts failed")))
    }

    fn device(&self) -> &str {
        &self.device
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chatterbox_backend_creation_without_venv() {
        // This test verifies the backend correctly fails when venv is not ready
        // In a CI environment without the venv, this should fail gracefully
        let result = ChatterboxBackend::new(None, None);
        // Either succeeds (venv exists) or fails with setup message
        match result {
            Ok(_) => (), // venv is ready
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("setup") || msg.contains("venv"),
                    "Error should mention setup: {}",
                    msg
                );
            }
        }
    }
}
