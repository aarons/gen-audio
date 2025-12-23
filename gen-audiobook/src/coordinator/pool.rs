//! Worker pool management for distributed processing.

use super::config::{WorkerConfig, WorkerDefaults, WorkersConfig};
use super::ssh::SshConnection;
use crate::worker::protocol::{TtsJob, TtsResult, WorkerStatus};
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A managed worker in the pool.
pub struct Worker {
    /// Worker configuration.
    pub config: WorkerConfig,
    /// SSH connection.
    pub connection: SshConnection,
    /// Current status.
    pub status: Option<WorkerStatus>,
    /// Jobs currently assigned to this worker.
    pub active_jobs: HashSet<String>,
    /// Whether connection is established.
    pub connected: bool,
}

impl Worker {
    /// Create a new worker.
    pub fn new(config: WorkerConfig, defaults: &WorkerDefaults) -> Self {
        let timeout = config.ssh_timeout(defaults);
        let connection = SshConnection::new(config.clone(), timeout);

        Self {
            config,
            connection,
            status: None,
            active_jobs: HashSet::new(),
            connected: false,
        }
    }

    /// Get worker name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Check if worker is ready for jobs.
    pub fn is_ready(&self) -> bool {
        self.connected && self.status.as_ref().map(|s| s.ready).unwrap_or(false)
    }

    /// Get number of active jobs.
    pub fn active_job_count(&self) -> usize {
        self.active_jobs.len()
    }

    /// Check if worker can accept more jobs.
    pub fn can_accept_job(&self, defaults: &WorkerDefaults) -> bool {
        if !self.is_ready() {
            return false;
        }
        let max = self.config.max_concurrent(defaults);
        self.active_jobs.len() < max as usize
    }

    /// Connect and get status.
    pub async fn connect(&mut self) -> Result<()> {
        // Test connection
        self.connection.test_connection().await
            .with_context(|| format!("Failed to connect to worker '{}'", self.name()))?;

        self.connected = true;

        // Get worker status
        let output = self.connection.exec("gena worker status").await
            .with_context(|| format!("Failed to get status from worker '{}'", self.name()))?;

        let status: WorkerStatus = serde_json::from_str(&output)
            .with_context(|| format!("Failed to parse status from worker '{}'", self.name()))?;

        self.status = Some(status);
        Ok(())
    }

    /// Check if voice reference is uploaded.
    pub async fn has_voice_ref(&self, hash: &str) -> Result<bool> {
        let remote_path = format!("~/.gena/worker/voices/{}.wav", hash);
        self.connection.file_exists(&remote_path).await
    }

    /// Upload voice reference file.
    pub async fn upload_voice_ref(&self, local_path: &Path, hash: &str) -> Result<()> {
        // Create voices directory
        self.connection.mkdir("~/.gena/worker/voices").await?;

        let remote_path = format!("~/.gena/worker/voices/{}.wav", hash);
        self.connection.upload(local_path, &remote_path).await
            .with_context(|| format!("Failed to upload voice reference to '{}'", self.name()))
    }

    /// Submit a job to this worker.
    pub async fn submit_job(&mut self, job: &TtsJob, job_timeout: u64) -> Result<TtsResult> {
        let job_id = job.job_id.clone();

        // Serialize job
        let job_json = serde_json::to_string(job)
            .context("Failed to serialize job")?;

        // Track active job
        self.active_jobs.insert(job_id.clone());

        // Execute job
        let result = self.execute_job(&job_json, job_timeout).await;

        // Remove from active jobs
        self.active_jobs.remove(&job_id);

        result
    }

    /// Execute job and parse result.
    async fn execute_job(&self, job_json: &str, timeout: u64) -> Result<TtsResult> {
        // Create a connection with job timeout
        let conn = SshConnection::new(self.config.clone(), timeout);

        let output = conn.exec_with_input("gena worker run", job_json.as_bytes()).await
            .with_context(|| format!("Job execution failed on worker '{}'", self.name()))?;

        let result: TtsResult = serde_json::from_slice(&output)
            .with_context(|| format!("Failed to parse job result from worker '{}'", self.name()))?;

        Ok(result)
    }

    /// Download result audio file.
    pub async fn download_audio(&self, remote_path: &str, local_path: &Path) -> Result<()> {
        self.connection.download(remote_path, local_path).await
    }

    /// Clean up remote audio file.
    pub async fn cleanup_audio(&self, remote_path: &str) -> Result<()> {
        self.connection.remove(remote_path).await
    }
}

/// Pool of workers for distributed processing.
pub struct WorkerPool {
    /// Workers in the pool.
    workers: Vec<Worker>,
    /// Default settings.
    defaults: WorkerDefaults,
    /// Voice references that have been uploaded (worker_name -> set of hashes).
    uploaded_voices: HashMap<String, HashSet<String>>,
}

impl WorkerPool {
    /// Create a new worker pool from configuration.
    pub fn new(config: &WorkersConfig) -> Self {
        let workers = config
            .workers
            .iter()
            .map(|c| Worker::new(c.clone(), &config.defaults))
            .collect();

        Self {
            workers,
            defaults: config.defaults.clone(),
            uploaded_voices: HashMap::new(),
        }
    }

    /// Create a pool with specific workers by name.
    pub fn with_workers(config: &WorkersConfig, names: &[&str]) -> Self {
        let workers = names
            .iter()
            .filter_map(|name| config.get_worker(name))
            .map(|c| Worker::new(c.clone(), &config.defaults))
            .collect();

        Self {
            workers,
            defaults: config.defaults.clone(),
            uploaded_voices: HashMap::new(),
        }
    }

    /// Get number of workers.
    pub fn len(&self) -> usize {
        self.workers.len()
    }

    /// Check if pool is empty.
    pub fn is_empty(&self) -> bool {
        self.workers.is_empty()
    }

    /// Connect to all workers and get their status.
    pub async fn connect_all(&mut self) -> Vec<(String, Result<()>)> {
        let mut results = Vec::new();

        for worker in &mut self.workers {
            let result = worker.connect().await;
            results.push((worker.name().to_string(), result));
        }

        results
    }

    /// Get list of ready workers.
    pub fn ready_workers(&self) -> Vec<&Worker> {
        self.workers.iter().filter(|w| w.is_ready()).collect()
    }

    /// Get list of workers that can accept jobs.
    #[allow(dead_code)]
    pub fn available_workers(&self) -> Vec<&Worker> {
        self.workers
            .iter()
            .filter(|w| w.can_accept_job(&self.defaults))
            .collect()
    }

    /// Get an available worker (round-robin by priority).
    pub fn get_available_worker(&mut self) -> Option<&mut Worker> {
        // Sort by (priority, active_jobs) to prefer lower priority and less loaded workers
        self.workers.sort_by_key(|w| (w.config.priority, w.active_job_count()));

        self.workers
            .iter_mut()
            .find(|w| w.can_accept_job(&self.defaults))
    }

    /// Get a worker by name.
    pub fn get_worker(&self, name: &str) -> Option<&Worker> {
        self.workers.iter().find(|w| w.name() == name)
    }

    /// Get a mutable worker by name.
    pub fn get_worker_mut(&mut self, name: &str) -> Option<&mut Worker> {
        self.workers.iter_mut().find(|w| w.name() == name)
    }

    /// Ensure voice reference is uploaded to all ready workers.
    pub async fn ensure_voice_ref(&mut self, local_path: &Path, hash: &str) -> Result<()> {
        for worker in &mut self.workers {
            if !worker.is_ready() {
                continue;
            }

            // Check if already uploaded in this session
            let worker_voices = self.uploaded_voices
                .entry(worker.name().to_string())
                .or_default();

            if worker_voices.contains(hash) {
                continue;
            }

            // Check if already exists on worker
            if worker.has_voice_ref(hash).await.unwrap_or(false) {
                worker_voices.insert(hash.to_string());
                continue;
            }

            // Upload
            worker.upload_voice_ref(local_path, hash).await?;
            worker_voices.insert(hash.to_string());
        }

        Ok(())
    }

    /// Get job timeout setting.
    pub fn job_timeout(&self) -> u64 {
        self.defaults.job_timeout_secs
    }

    /// Get summary of pool status.
    #[allow(dead_code)]
    pub fn status_summary(&self) -> PoolStatus {
        let total = self.workers.len();
        let connected = self.workers.iter().filter(|w| w.connected).count();
        let ready = self.workers.iter().filter(|w| w.is_ready()).count();
        let active_jobs: usize = self.workers.iter().map(|w| w.active_job_count()).sum();

        PoolStatus {
            total,
            connected,
            ready,
            active_jobs,
        }
    }
}

/// Summary of pool status.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PoolStatus {
    pub total: usize,
    pub connected: usize,
    pub ready: usize,
    pub active_jobs: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_creation() {
        let config = WorkersConfig {
            defaults: WorkerDefaults::default(),
            workers: vec![
                WorkerConfig::new("worker1", "host1", "user1"),
                WorkerConfig::new("worker2", "host2", "user2"),
            ],
        };

        let pool = WorkerPool::new(&config);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn test_pool_status() {
        let config = WorkersConfig {
            defaults: WorkerDefaults::default(),
            workers: vec![
                WorkerConfig::new("worker1", "host1", "user1"),
            ],
        };

        let pool = WorkerPool::new(&config);
        let status = pool.status_summary();
        assert_eq!(status.total, 1);
        assert_eq!(status.connected, 0);
        assert_eq!(status.ready, 0);
    }
}
