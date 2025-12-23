//! Job scheduler for distributed TTS processing.

use super::pool::WorkerPool;
use crate::worker::protocol::{JobStatus, TtsJob, TtsJobOptions, TtsResult};
use anyhow::Result;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Progress information for the scheduler.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SchedulerProgress {
    /// Total number of jobs.
    pub total_jobs: usize,
    /// Number of completed jobs.
    pub completed: usize,
    /// Number of jobs currently in flight.
    pub in_flight: usize,
    /// Number of failed jobs.
    pub failed: usize,
    /// Per-worker statistics.
    pub workers: Vec<WorkerProgress>,
}

/// Per-worker progress.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WorkerProgress {
    /// Worker name.
    pub name: String,
    /// Jobs completed by this worker.
    pub completed: usize,
    /// Jobs currently in flight.
    pub in_flight: usize,
    /// Average time per job in milliseconds.
    pub avg_time_ms: u64,
}

/// A job that is currently in flight.
struct InFlightJob {
    /// The job.
    job: TtsJob,
    /// Worker handling this job.
    worker_name: String,
}

/// Job scheduler that distributes work across workers.
pub struct JobScheduler {
    /// Worker pool.
    pool: Arc<Mutex<WorkerPool>>,
    /// Pending jobs waiting to be assigned.
    pending: VecDeque<TtsJob>,
    /// Jobs currently in flight.
    in_flight: Vec<InFlightJob>,
    /// Completed results.
    completed: Vec<TtsResult>,
    /// Failed jobs for retry.
    failed: Vec<TtsJob>,
    /// Maximum retries per job.
    max_retries: u32,
    /// Job retry counts.
    retry_counts: std::collections::HashMap<String, u32>,
    /// Per-worker statistics.
    worker_stats: std::collections::HashMap<String, WorkerStats>,
    /// Temporary directory for downloaded audio.
    temp_dir: PathBuf,
}

/// Statistics for a single worker.
#[derive(Debug, Default)]
struct WorkerStats {
    completed: usize,
    total_time_ms: u64,
}

impl JobScheduler {
    /// Create a new scheduler.
    pub fn new(pool: WorkerPool, temp_dir: PathBuf) -> Self {
        Self {
            pool: Arc::new(Mutex::new(pool)),
            pending: VecDeque::new(),
            in_flight: Vec::new(),
            completed: Vec::new(),
            failed: Vec::new(),
            max_retries: 3,
            retry_counts: std::collections::HashMap::new(),
            worker_stats: std::collections::HashMap::new(),
            temp_dir,
        }
    }

    /// Add jobs to the queue.
    pub fn enqueue(&mut self, jobs: Vec<TtsJob>) {
        for job in jobs {
            self.pending.push_back(job);
        }
    }

    /// Get current progress.
    pub fn progress(&self) -> SchedulerProgress {
        let workers: Vec<WorkerProgress> = self
            .worker_stats
            .iter()
            .map(|(name, stats)| {
                let in_flight = self
                    .in_flight
                    .iter()
                    .filter(|j| j.worker_name == *name)
                    .count();

                let avg_time_ms = if stats.completed > 0 {
                    stats.total_time_ms / stats.completed as u64
                } else {
                    0
                };

                WorkerProgress {
                    name: name.clone(),
                    completed: stats.completed,
                    in_flight,
                    avg_time_ms,
                }
            })
            .collect();

        SchedulerProgress {
            total_jobs: self.pending.len() + self.in_flight.len() + self.completed.len() + self.failed.len(),
            completed: self.completed.len(),
            in_flight: self.in_flight.len(),
            failed: self.failed.len(),
            workers,
        }
    }

    /// Run the scheduler until all jobs complete.
    pub async fn run_to_completion<F>(&mut self, mut on_progress: F) -> Result<Vec<TtsResult>>
    where
        F: FnMut(SchedulerProgress),
    {
        // Create channel for results
        let (tx, mut rx) = mpsc::channel::<(String, TtsResult)>(32);

        loop {
            // Check if we're done
            if self.pending.is_empty() && self.in_flight.is_empty() && self.failed.is_empty() {
                break;
            }

            // Try to assign pending jobs to available workers
            while !self.pending.is_empty() {
                let mut pool = self.pool.lock().await;
                if let Some(worker) = pool.get_available_worker() {
                    if let Some(job) = self.pending.pop_front() {
                        let job_id = job.job_id.clone();
                        let worker_name = worker.name().to_string();
                        let job_timeout = pool.job_timeout();

                        // Track in-flight job
                        self.in_flight.push(InFlightJob {
                            job: job.clone(),
                            worker_name: worker_name.clone(),
                        });

                        // Spawn job execution
                        let tx = tx.clone();
                        let pool_clone = Arc::clone(&self.pool);

                        tokio::spawn(async move {
                            let result = {
                                let mut pool = pool_clone.lock().await;
                                if let Some(worker) = pool.get_worker_mut(&worker_name) {
                                    worker.submit_job(&job, job_timeout).await
                                } else {
                                    Err(anyhow::anyhow!("Worker not found"))
                                }
                            };

                            let result = match result {
                                Ok(r) => r,
                                Err(e) => TtsResult::failure(&job_id, e.to_string()),
                            };

                            let _ = tx.send((worker_name, result)).await;
                        });
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Retry failed jobs if workers are available
            while !self.failed.is_empty() {
                let mut pool = self.pool.lock().await;
                if pool.get_available_worker().is_some() {
                    if let Some(job) = self.failed.pop() {
                        self.pending.push_front(job);
                    }
                } else {
                    break;
                }
            }

            // Wait for a result with timeout
            tokio::select! {
                Some((worker_name, result)) = rx.recv() => {
                    self.handle_result(worker_name, result).await?;
                    on_progress(self.progress());
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    // Periodic check
                }
            }
        }

        Ok(std::mem::take(&mut self.completed))
    }

    /// Handle a completed job result.
    async fn handle_result(&mut self, worker_name: String, result: TtsResult) -> Result<()> {
        // Find and remove the in-flight job
        let job_idx = self
            .in_flight
            .iter()
            .position(|j| j.job.job_id == result.job_id);

        let job = if let Some(idx) = job_idx {
            self.in_flight.remove(idx).job
        } else {
            return Ok(());
        };

        match result.status {
            JobStatus::Completed => {
                // Update worker stats
                let stats = self.worker_stats.entry(worker_name.clone()).or_default();
                stats.completed += 1;
                if let Some(ms) = result.duration_ms {
                    stats.total_time_ms += ms;
                }

                // Download the audio file if path is provided
                if let Some(ref remote_path) = result.audio_path {
                    let local_path = self.temp_dir.join(format!("{}.wav", result.job_id));

                    let download_result = {
                        let pool = self.pool.lock().await;
                        if let Some(worker) = pool.get_worker(&worker_name) {
                            worker.download_audio(remote_path, &local_path).await
                        } else {
                            Err(anyhow::anyhow!("Worker not found"))
                        }
                    };

                    if let Err(e) = download_result {
                        eprintln!("Warning: Failed to download audio for {}: {}", result.job_id, e);
                    }

                    // Clean up remote file
                    let _ = {
                        let pool = self.pool.lock().await;
                        if let Some(worker) = pool.get_worker(&worker_name) {
                            worker.cleanup_audio(remote_path).await
                        } else {
                            Ok(())
                        }
                    };
                }

                self.completed.push(result);
            }
            JobStatus::Failed | JobStatus::Timeout => {
                // Check if we should retry
                let retry_count = self.retry_counts.entry(job.job_id.clone()).or_insert(0);
                *retry_count += 1;

                if *retry_count < self.max_retries {
                    eprintln!(
                        "Job {} failed (attempt {}), retrying: {}",
                        job.job_id,
                        retry_count,
                        result.error.as_deref().unwrap_or("unknown")
                    );
                    self.failed.push(job);
                } else {
                    eprintln!(
                        "Job {} failed after {} attempts: {}",
                        job.job_id,
                        self.max_retries,
                        result.error.as_deref().unwrap_or("unknown")
                    );
                    self.completed.push(result);
                }
            }
        }

        Ok(())
    }

    /// Get completed results grouped by chapter.
    #[allow(dead_code)]
    pub fn results_by_chapter(&self) -> std::collections::HashMap<usize, Vec<&TtsResult>> {
        let mut by_chapter: std::collections::HashMap<usize, Vec<&TtsResult>> =
            std::collections::HashMap::new();

        for result in &self.completed {
            // Parse chapter from job_id (format: session_chXXX_ckYYYY)
            if let Some(chapter) = parse_chapter_from_job_id(&result.job_id) {
                by_chapter.entry(chapter).or_default().push(result);
            }
        }

        // Sort chunks within each chapter
        for chunks in by_chapter.values_mut() {
            chunks.sort_by_key(|r| parse_chunk_from_job_id(&r.job_id));
        }

        by_chapter
    }
}

/// Parse chapter number from job ID.
#[allow(dead_code)]
fn parse_chapter_from_job_id(job_id: &str) -> Option<usize> {
    // Format: session_chXXX_ckYYYY
    let parts: Vec<&str> = job_id.split('_').collect();
    for part in parts {
        if part.starts_with("ch") {
            return part[2..].parse().ok();
        }
    }
    None
}

/// Parse chunk number from job ID.
#[allow(dead_code)]
fn parse_chunk_from_job_id(job_id: &str) -> Option<usize> {
    let parts: Vec<&str> = job_id.split('_').collect();
    for part in parts {
        if part.starts_with("ck") {
            return part[2..].parse().ok();
        }
    }
    None
}

/// Create TTS jobs from text chunks.
pub fn create_jobs(
    session_id: &str,
    chunks: &[(usize, usize, String)], // (chapter_id, chunk_id, text)
    options: TtsJobOptions,
) -> Vec<TtsJob> {
    chunks
        .iter()
        .map(|(chapter_id, chunk_id, text)| {
            TtsJob::new(session_id, *chapter_id, *chunk_id, text, options.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_job_id() {
        let job_id = "sess123_ch001_ck0042";
        assert_eq!(parse_chapter_from_job_id(job_id), Some(1));
        assert_eq!(parse_chunk_from_job_id(job_id), Some(42));
    }

    #[test]
    fn test_create_jobs() {
        let chunks = vec![
            (0, 0, "Hello".to_string()),
            (0, 1, "World".to_string()),
            (1, 0, "Chapter 2".to_string()),
        ];

        let jobs = create_jobs("test_session", &chunks, TtsJobOptions::default());
        assert_eq!(jobs.len(), 3);
        assert_eq!(jobs[0].chapter_id, 0);
        assert_eq!(jobs[0].chunk_id, 0);
        assert_eq!(jobs[2].chapter_id, 1);
    }
}
