//! Worker configuration for distributed processing.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Default SSH timeout in seconds.
pub const DEFAULT_SSH_TIMEOUT_SECS: u64 = 30;

/// Default job timeout in seconds.
pub const DEFAULT_JOB_TIMEOUT_SECS: u64 = 300;

/// Configuration for all workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkersConfig {
    /// Default settings for all workers.
    #[serde(default)]
    pub defaults: WorkerDefaults,

    /// Worker definitions.
    #[serde(default)]
    pub workers: Vec<WorkerConfig>,
}

impl Default for WorkersConfig {
    fn default() -> Self {
        Self {
            defaults: WorkerDefaults::default(),
            workers: Vec::new(),
        }
    }
}

impl WorkersConfig {
    /// Load configuration from the default location.
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;

        toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", config_path.display()))
    }

    /// Save configuration to the default location.
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }

        let content = toml::to_string_pretty(self)
            .context("Failed to serialize configuration")?;

        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write {}", config_path.display()))
    }

    /// Get the configuration file path.
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cli-programs")
            .join("gena-workers.toml")
    }

    /// Get a worker by name.
    pub fn get_worker(&self, name: &str) -> Option<&WorkerConfig> {
        self.workers.iter().find(|w| w.name == name)
    }

    /// Get workers by names (comma-separated).
    #[allow(dead_code)]
    pub fn get_workers_by_names(&self, names: &str) -> Vec<&WorkerConfig> {
        names
            .split(',')
            .map(|s| s.trim())
            .filter_map(|name| self.get_worker(name))
            .collect()
    }

    /// Add a new worker.
    pub fn add_worker(&mut self, worker: WorkerConfig) {
        // Remove existing worker with same name
        self.workers.retain(|w| w.name != worker.name);
        self.workers.push(worker);
    }

    /// Remove a worker by name.
    pub fn remove_worker(&mut self, name: &str) -> bool {
        let len = self.workers.len();
        self.workers.retain(|w| w.name != name);
        self.workers.len() < len
    }
}

/// Default settings applied to all workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerDefaults {
    /// SSH connection timeout in seconds.
    #[serde(default = "default_ssh_timeout")]
    pub ssh_timeout_secs: u64,

    /// Job execution timeout in seconds.
    #[serde(default = "default_job_timeout")]
    pub job_timeout_secs: u64,

    /// Number of retry attempts for failed jobs.
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,

    /// Maximum concurrent jobs per worker.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_jobs: u32,
}

impl Default for WorkerDefaults {
    fn default() -> Self {
        Self {
            ssh_timeout_secs: default_ssh_timeout(),
            job_timeout_secs: default_job_timeout(),
            retry_attempts: default_retry_attempts(),
            max_concurrent_jobs: default_max_concurrent(),
        }
    }
}

fn default_ssh_timeout() -> u64 {
    DEFAULT_SSH_TIMEOUT_SECS
}

fn default_job_timeout() -> u64 {
    DEFAULT_JOB_TIMEOUT_SECS
}

fn default_retry_attempts() -> u32 {
    3
}

fn default_max_concurrent() -> u32 {
    1
}

/// Configuration for a single worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Unique name for this worker.
    pub name: String,

    /// SSH hostname or IP address.
    pub host: String,

    /// SSH username.
    pub user: String,

    /// SSH port (default: 22).
    #[serde(default = "default_port")]
    pub port: u16,

    /// Path to SSH private key (optional, uses SSH agent if not specified).
    pub ssh_key: Option<String>,

    /// Priority for job assignment (lower = higher priority).
    #[serde(default = "default_priority")]
    pub priority: u32,

    /// Override SSH timeout for this worker.
    pub ssh_timeout_secs: Option<u64>,

    /// Override job timeout for this worker.
    pub job_timeout_secs: Option<u64>,

    /// Override max concurrent jobs for this worker.
    pub max_concurrent_jobs: Option<u32>,
}

fn default_port() -> u16 {
    22
}

fn default_priority() -> u32 {
    1
}

impl WorkerConfig {
    /// Create a new worker configuration.
    pub fn new(name: impl Into<String>, host: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            host: host.into(),
            user: user.into(),
            port: default_port(),
            ssh_key: None,
            priority: default_priority(),
            ssh_timeout_secs: None,
            job_timeout_secs: None,
            max_concurrent_jobs: None,
        }
    }

    /// Set SSH port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set SSH key path.
    pub fn with_ssh_key(mut self, path: impl Into<String>) -> Self {
        self.ssh_key = Some(path.into());
        self
    }

    /// Set priority.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Get effective SSH timeout (worker override or default).
    pub fn ssh_timeout(&self, defaults: &WorkerDefaults) -> u64 {
        self.ssh_timeout_secs.unwrap_or(defaults.ssh_timeout_secs)
    }

    /// Get effective job timeout.
    #[allow(dead_code)]
    pub fn job_timeout(&self, defaults: &WorkerDefaults) -> u64 {
        self.job_timeout_secs.unwrap_or(defaults.job_timeout_secs)
    }

    /// Get effective max concurrent jobs.
    pub fn max_concurrent(&self, defaults: &WorkerDefaults) -> u32 {
        self.max_concurrent_jobs.unwrap_or(defaults.max_concurrent_jobs)
    }

    /// Expand ~ in SSH key path.
    pub fn expanded_ssh_key(&self) -> Option<PathBuf> {
        self.ssh_key.as_ref().map(|path| {
            if path.starts_with("~/") {
                dirs::home_dir()
                    .map(|home| home.join(&path[2..]))
                    .unwrap_or_else(|| PathBuf::from(path))
            } else {
                PathBuf::from(path)
            }
        })
    }

    /// Get SSH connection string (user@host).
    pub fn ssh_target(&self) -> String {
        format!("{}@{}", self.user, self.host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_config_new() {
        let config = WorkerConfig::new("test", "192.168.1.1", "ubuntu");
        assert_eq!(config.name, "test");
        assert_eq!(config.host, "192.168.1.1");
        assert_eq!(config.user, "ubuntu");
        assert_eq!(config.port, 22);
    }

    #[test]
    fn test_worker_config_builder() {
        let config = WorkerConfig::new("test", "example.com", "user")
            .with_port(2222)
            .with_ssh_key("~/.ssh/id_ed25519")
            .with_priority(2);

        assert_eq!(config.port, 2222);
        assert_eq!(config.ssh_key, Some("~/.ssh/id_ed25519".to_string()));
        assert_eq!(config.priority, 2);
    }

    #[test]
    fn test_expanded_ssh_key() {
        let config = WorkerConfig::new("test", "host", "user")
            .with_ssh_key("~/.ssh/test_key");

        let expanded = config.expanded_ssh_key().unwrap();
        assert!(expanded.to_string_lossy().contains(".ssh/test_key"));
        assert!(!expanded.to_string_lossy().starts_with("~"));
    }

    #[test]
    fn test_workers_config_add_remove() {
        let mut config = WorkersConfig::default();

        config.add_worker(WorkerConfig::new("worker1", "host1", "user1"));
        assert_eq!(config.workers.len(), 1);

        config.add_worker(WorkerConfig::new("worker2", "host2", "user2"));
        assert_eq!(config.workers.len(), 2);

        // Adding worker with same name replaces it
        config.add_worker(WorkerConfig::new("worker1", "newhost", "newuser"));
        assert_eq!(config.workers.len(), 2);
        assert_eq!(config.get_worker("worker1").unwrap().host, "newhost");

        assert!(config.remove_worker("worker1"));
        assert_eq!(config.workers.len(), 1);
        assert!(!config.remove_worker("worker1"));
    }

    #[test]
    fn test_parse_toml() {
        let toml = r#"
[defaults]
ssh_timeout_secs = 60
job_timeout_secs = 600

[[workers]]
name = "gpu1"
host = "192.168.1.50"
user = "ubuntu"
ssh_key = "~/.ssh/id_ed25519"
priority = 1

[[workers]]
name = "gpu2"
host = "ssh.vast.ai"
user = "root"
port = 12345
"#;

        let config: WorkersConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.defaults.ssh_timeout_secs, 60);
        assert_eq!(config.workers.len(), 2);
        assert_eq!(config.workers[0].name, "gpu1");
        assert_eq!(config.workers[1].port, 12345);
    }
}
