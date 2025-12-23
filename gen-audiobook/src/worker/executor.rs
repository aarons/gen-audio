//! Job execution logic for worker mode.

use super::protocol::{TtsJob, TtsResult, WorkerStatus};
use crate::setup;
use crate::tts::{self, TtsOptions};
use anyhow::{Context, Result};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::time::Instant;

/// Base directory for worker data.
fn worker_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gena")
        .join("worker")
}

/// Directory for voice reference files.
pub fn voices_dir() -> PathBuf {
    worker_data_dir().join("voices")
}

/// Directory for output audio files.
pub fn output_dir() -> PathBuf {
    worker_data_dir().join("output")
}

/// Get path to voice reference by hash.
pub fn get_voice_path(hash: &str) -> PathBuf {
    voices_dir().join(format!("{}.wav", hash))
}

/// Get output path for a job.
pub fn get_output_path(job_id: &str) -> PathBuf {
    output_dir().join(format!("{}.wav", job_id))
}

/// Execute a job from stdin and write result to stdout.
pub async fn execute_job_from_stdin() -> Result<()> {
    // Read job JSON from stdin
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("Failed to read job from stdin")?;

    let job: TtsJob = serde_json::from_str(&input)
        .context("Failed to parse job JSON")?;

    // Execute the job
    let result = execute_job(&job).await;

    // Write result to stdout
    let output = serde_json::to_string(&result)
        .context("Failed to serialize result")?;
    io::stdout()
        .write_all(output.as_bytes())
        .context("Failed to write result to stdout")?;

    Ok(())
}

/// Execute a TTS job and return the result.
pub async fn execute_job(job: &TtsJob) -> TtsResult {
    let start = Instant::now();

    // Resolve voice reference
    let voice_ref = job
        .options
        .voice_ref_hash
        .as_ref()
        .map(|hash| get_voice_path(hash))
        .filter(|path| path.exists());

    // Check if voice ref was expected but not found
    if job.options.voice_ref_hash.is_some() && voice_ref.is_none() {
        return TtsResult::failure(
            &job.job_id,
            format!(
                "Voice reference not found: {}",
                job.options.voice_ref_hash.as_ref().unwrap()
            ),
        );
    }

    // Create TTS options
    let options = TtsOptions {
        voice_ref,
        exaggeration: job.options.exaggeration,
        cfg: job.options.cfg,
        temperature: job.options.temperature,
    };

    // Ensure output directory exists
    let output_path = get_output_path(&job.job_id);
    if let Some(parent) = output_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return TtsResult::failure(
                &job.job_id,
                format!("Failed to create output directory: {}", e),
            );
        }
    }

    // Create backend and synthesize
    let backend = match tts::create_backend(None, None) {
        Ok(b) => b,
        Err(e) => {
            return TtsResult::failure(
                &job.job_id,
                format!("Failed to create TTS backend: {}", e),
            );
        }
    };

    // Synthesize with retry
    match backend
        .synthesize_with_retry(&job.text, &output_path, &options, 3)
        .await
    {
        Ok(()) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let audio_size = std::fs::metadata(&output_path)
                .map(|m| m.len())
                .unwrap_or(0);

            TtsResult::success(
                &job.job_id,
                duration_ms,
                audio_size,
                output_path.to_string_lossy(),
            )
        }
        Err(e) => TtsResult::failure(&job.job_id, e.to_string()),
    }
}

/// Get worker status for health checks.
pub fn get_worker_status() -> WorkerStatus {
    // Check if setup is ready
    let venv_ready = setup::is_venv_ready().unwrap_or(false);
    let chatterbox_installed = setup::is_chatterbox_installed().unwrap_or(false);

    if !venv_ready || !chatterbox_installed {
        return WorkerStatus::not_ready("Setup incomplete");
    }

    // Detect device
    let device = detect_device().unwrap_or_else(|_| "cpu".to_string());

    // Get available disk space
    let available_disk_mb = get_available_disk_mb().unwrap_or(0);

    WorkerStatus::ready(device, available_disk_mb)
}

/// Detect the best available device.
fn detect_device() -> Result<String> {
    use pyo3::prelude::*;

    Python::with_gil(|py| {
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

        Ok("cpu".to_string())
    })
}

/// Get available disk space in MB.
fn get_available_disk_mb() -> Result<u64> {
    let output_dir = output_dir();
    let _dir = if output_dir.exists() {
        output_dir
    } else {
        // Use home directory as fallback
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    };

    // TODO: Use statvfs for accurate disk space on Unix
    // For now, return a reasonable default
    Ok(10000) // 10GB default
}

/// Clean up old output files.
pub fn cleanup_output(job_id: &str) -> Result<()> {
    let path = get_output_path(job_id);
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("Failed to remove {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths() {
        let voice_path = get_voice_path("abc123");
        assert!(voice_path.to_string_lossy().contains("voices"));
        assert!(voice_path.to_string_lossy().ends_with("abc123.wav"));

        let output_path = get_output_path("job_001");
        assert!(output_path.to_string_lossy().contains("output"));
        assert!(output_path.to_string_lossy().ends_with("job_001.wav"));
    }

    #[test]
    fn test_worker_status() {
        // Just verify it doesn't panic
        let status = get_worker_status();
        assert!(!status.gena_version.is_empty());
    }
}
