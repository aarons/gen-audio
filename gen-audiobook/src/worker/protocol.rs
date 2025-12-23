//! Protocol types for worker communication.
//!
//! Jobs are sent as JSON over stdin, results returned via stdout.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Current protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

/// A TTS job to be executed by a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsJob {
    /// Protocol version for compatibility checking.
    pub version: u32,
    /// Unique job identifier (session_chapter_chunk format).
    pub job_id: String,
    /// Session this job belongs to.
    pub session_id: String,
    /// Chapter number (0-indexed).
    pub chapter_id: usize,
    /// Chunk number within chapter (0-indexed).
    pub chunk_id: usize,
    /// Text to synthesize.
    pub text: String,
    /// TTS options.
    pub options: TtsJobOptions,
    /// When this job was created.
    pub created_at: DateTime<Utc>,
}

impl TtsJob {
    /// Create a new TTS job.
    pub fn new(
        session_id: impl Into<String>,
        chapter_id: usize,
        chunk_id: usize,
        text: impl Into<String>,
        options: TtsJobOptions,
    ) -> Self {
        let session_id = session_id.into();
        let job_id = format!(
            "{}_ch{:03}_ck{:04}",
            session_id, chapter_id, chunk_id
        );
        Self {
            version: PROTOCOL_VERSION,
            job_id,
            session_id,
            chapter_id,
            chunk_id,
            text: text.into(),
            options,
            created_at: Utc::now(),
        }
    }
}

/// TTS synthesis options sent with each job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsJobOptions {
    /// Expressiveness/exaggeration (0.25-2.0).
    pub exaggeration: f32,
    /// Pacing/CFG weight (0.0-1.0).
    pub cfg: f32,
    /// Temperature for randomness (0.05-5.0).
    pub temperature: f32,
    /// SHA256 hash of voice reference file (if using voice cloning).
    /// Worker uses this to locate the pre-uploaded voice file.
    pub voice_ref_hash: Option<String>,
}

impl Default for TtsJobOptions {
    fn default() -> Self {
        Self {
            exaggeration: 0.5,
            cfg: 0.5,
            temperature: 0.8,
            voice_ref_hash: None,
        }
    }
}

/// Result of a TTS job execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsResult {
    /// Protocol version.
    pub version: u32,
    /// Job ID this result corresponds to.
    pub job_id: String,
    /// Job completion status.
    pub status: JobStatus,
    /// Time taken to synthesize (milliseconds).
    pub duration_ms: Option<u64>,
    /// Size of generated audio file (bytes).
    pub audio_size_bytes: Option<u64>,
    /// Path to generated audio on worker filesystem.
    pub audio_path: Option<String>,
    /// Error message if job failed.
    pub error: Option<String>,
    /// When this job completed.
    pub completed_at: DateTime<Utc>,
}

impl TtsResult {
    /// Create a successful result.
    pub fn success(
        job_id: impl Into<String>,
        duration_ms: u64,
        audio_size_bytes: u64,
        audio_path: impl Into<String>,
    ) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            job_id: job_id.into(),
            status: JobStatus::Completed,
            duration_ms: Some(duration_ms),
            audio_size_bytes: Some(audio_size_bytes),
            audio_path: Some(audio_path.into()),
            error: None,
            completed_at: Utc::now(),
        }
    }

    /// Create a failed result.
    pub fn failure(job_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            job_id: job_id.into(),
            status: JobStatus::Failed,
            duration_ms: None,
            audio_size_bytes: None,
            audio_path: None,
            error: Some(error.into()),
            completed_at: Utc::now(),
        }
    }

    /// Create a timeout result.
    #[allow(dead_code)]
    pub fn timeout(job_id: impl Into<String>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            job_id: job_id.into(),
            status: JobStatus::Timeout,
            duration_ms: None,
            audio_size_bytes: None,
            audio_path: None,
            error: Some("Job timed out".to_string()),
            completed_at: Utc::now(),
        }
    }
}

/// Status of a completed job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Job completed successfully.
    Completed,
    /// Job failed with an error.
    Failed,
    /// Job exceeded time limit.
    Timeout,
}

/// Worker status response for health checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatus {
    /// Whether the worker is ready to accept jobs.
    pub ready: bool,
    /// Device being used (cuda, mps, cpu).
    pub device: String,
    /// Version of gena installed on worker.
    pub gena_version: String,
    /// Whether Chatterbox TTS is installed and working.
    pub chatterbox_installed: bool,
    /// Number of jobs currently being processed.
    pub jobs_in_progress: usize,
    /// Available disk space in MB.
    pub available_disk_mb: u64,
}

impl WorkerStatus {
    /// Create a ready status.
    pub fn ready(device: impl Into<String>, available_disk_mb: u64) -> Self {
        Self {
            ready: true,
            device: device.into(),
            gena_version: env!("CARGO_PKG_VERSION").to_string(),
            chatterbox_installed: true,
            jobs_in_progress: 0,
            available_disk_mb,
        }
    }

    /// Create a not-ready status with reason.
    pub fn not_ready(_reason: impl Into<String>) -> Self {
        Self {
            ready: false,
            device: "unknown".to_string(),
            gena_version: env!("CARGO_PKG_VERSION").to_string(),
            chatterbox_installed: false,
            jobs_in_progress: 0,
            available_disk_mb: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_id_format() {
        let job = TtsJob::new(
            "abc123_20240115",
            1,
            42,
            "Hello world",
            TtsJobOptions::default(),
        );
        assert_eq!(job.job_id, "abc123_20240115_ch001_ck0042");
    }

    #[test]
    fn test_result_serialization() {
        let result = TtsResult::success(
            "test_job",
            1234,
            56789,
            "/tmp/audio.wav",
        );
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"status\":\"completed\""));
    }

    #[test]
    fn test_worker_status() {
        let status = WorkerStatus::ready("cuda", 50000);
        assert!(status.ready);
        assert_eq!(status.device, "cuda");
    }
}
