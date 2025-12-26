//! Worker utilities.
//!
//! TTS execution is now handled by the Python gen-audio-worker.
//! This module provides path utilities for the coordinator.

use super::protocol::WorkerStatus;
use std::path::PathBuf;

/// Base directory for worker data (used by coordinator for paths).
fn worker_data_dir() -> PathBuf {
    // Match Python worker's path: ~/.gen-audio/worker/
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".gen-audio")
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
#[allow(dead_code)]
pub fn get_voice_path(hash: &str) -> PathBuf {
    voices_dir().join(format!("{}.wav", hash))
}

/// Get output path for a job.
#[allow(dead_code)]
pub fn get_output_path(job_id: &str) -> PathBuf {
    output_dir().join(format!("{}.wav", job_id))
}

/// Get worker status.
///
/// Note: This returns a stub status. Real worker status comes from
/// the Python gen-audio-worker via SSH.
pub fn get_worker_status() -> WorkerStatus {
    // This is only used for local info display
    // Real status comes from Python worker via SSH
    WorkerStatus {
        ready: false,
        device: "n/a".to_string(),
        gen_audio_version: env!("CARGO_PKG_VERSION").to_string(),
        chatterbox_installed: false,
        jobs_in_progress: 0,
        available_disk_mb: 0,
    }
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
}
