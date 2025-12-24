//! Coordinator for distributed TTS processing.
//!
//! The coordinator manages workers and distributes jobs across them.

pub mod config;
pub mod pool;
pub mod scheduler;
pub mod ssh;

pub use config::{WorkerConfig, WorkersConfig};
pub use pool::WorkerPool;
pub use scheduler::{create_jobs, JobScheduler};
pub use ssh::SshConnection;

use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::PathBuf;

/// Workers management subcommands.
#[derive(Subcommand, Debug)]
pub enum WorkersCommand {
    /// List configured workers.
    List,

    /// Add a new worker.
    Add {
        /// Unique name for this worker.
        name: String,
        /// SSH hostname or IP address.
        host: String,
        /// SSH username.
        #[arg(short, long, default_value = "root")]
        user: String,
        /// SSH port.
        #[arg(short, long, default_value = "22")]
        port: u16,
        /// Path to SSH private key.
        #[arg(short = 'k', long)]
        ssh_key: Option<String>,
        /// Priority (lower = higher priority).
        #[arg(long, default_value = "1")]
        priority: u32,
    },

    /// Remove a worker.
    Remove {
        /// Name of worker to remove.
        name: String,
    },

    /// Test connection to worker(s).
    Test {
        /// Name of worker to test (tests all if not specified).
        name: Option<String>,
    },

    /// Set up a worker remotely (install gen-audio and dependencies).
    Setup {
        /// Name of worker to set up.
        name: String,
    },
}

/// Handle workers subcommand.
pub async fn handle_workers_command(cmd: &WorkersCommand) -> Result<()> {
    match cmd {
        WorkersCommand::List => {
            list_workers()
        }
        WorkersCommand::Add {
            name,
            host,
            user,
            port,
            ssh_key,
            priority,
        } => {
            add_worker(name, host, user, *port, ssh_key.clone(), *priority)
        }
        WorkersCommand::Remove { name } => {
            remove_worker(name)
        }
        WorkersCommand::Test { name } => {
            test_workers(name.as_deref()).await
        }
        WorkersCommand::Setup { name } => {
            setup_worker(name).await
        }
    }
}

/// List configured workers.
fn list_workers() -> Result<()> {
    let config = WorkersConfig::load()?;

    if config.workers.is_empty() {
        println!("No workers configured.");
        println!();
        println!("Add a worker with:");
        println!("  gen-audio workers add <name> <host> -u <user>");
        return Ok(());
    }

    println!("Configured workers:");
    println!();

    for worker in &config.workers {
        println!("  {} (priority {})", worker.name, worker.priority);
        println!("    Host: {}@{}:{}", worker.user, worker.host, worker.port);
        if let Some(ref key) = worker.ssh_key {
            println!("    SSH key: {}", key);
        }
        println!();
    }

    Ok(())
}

/// Add a new worker.
fn add_worker(
    name: &str,
    host: &str,
    user: &str,
    port: u16,
    ssh_key: Option<String>,
    priority: u32,
) -> Result<()> {
    let mut config = WorkersConfig::load()?;

    let mut worker = WorkerConfig::new(name, host, user)
        .with_port(port)
        .with_priority(priority);

    if let Some(key) = ssh_key {
        worker = worker.with_ssh_key(key);
    }

    config.add_worker(worker);
    config.save()?;

    println!("Added worker '{}'", name);
    Ok(())
}

/// Remove a worker.
fn remove_worker(name: &str) -> Result<()> {
    let mut config = WorkersConfig::load()?;

    if config.remove_worker(name) {
        config.save()?;
        println!("Removed worker '{}'", name);
    } else {
        println!("Worker '{}' not found", name);
    }

    Ok(())
}

/// Test connection to workers.
async fn test_workers(name: Option<&str>) -> Result<()> {
    let config = WorkersConfig::load()?;

    let workers_to_test: Vec<&WorkerConfig> = if let Some(name) = name {
        config
            .get_worker(name)
            .map(|w| vec![w])
            .unwrap_or_default()
    } else {
        config.workers.iter().collect()
    };

    if workers_to_test.is_empty() {
        if name.is_some() {
            println!("Worker '{}' not found", name.unwrap());
        } else {
            println!("No workers configured");
        }
        return Ok(());
    }

    for worker_config in workers_to_test {
        print!("Testing {}... ", worker_config.name);

        let mut pool = WorkerPool::with_workers(&config, &[&worker_config.name]);
        let results = pool.connect_all().await;

        for (name, result) in results {
            match result {
                Ok(()) => {
                    if let Some(worker) = pool.get_worker(&name) {
                        if let Some(ref status) = worker.status {
                            println!(
                                "OK (device: {}, ready: {})",
                                status.device,
                                if status.ready { "yes" } else { "no" }
                            );
                        } else {
                            println!("OK (no status)");
                        }
                    }
                }
                Err(e) => {
                    println!("FAILED: {}", e);
                }
            }
        }
    }

    Ok(())
}

/// Set up a worker remotely.
async fn setup_worker(name: &str) -> Result<()> {
    let config = WorkersConfig::load()?;

    let worker_config = config
        .get_worker(name)
        .ok_or_else(|| anyhow::anyhow!("Worker '{}' not found", name))?;

    println!("Setting up worker '{}'...", name);
    println!();

    let conn = SshConnection::new(worker_config.clone(), 60);

    // Test connection
    print!("Testing connection... ");
    conn.test_connection().await?;
    println!("OK");

    // Check for NVIDIA GPU
    print!("Checking for NVIDIA GPU... ");
    let gpu_check = conn.exec("nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null || echo 'NOT_FOUND'").await?;
    let gpu = gpu_check.trim();
    if gpu == "NOT_FOUND" {
        println!("NOT FOUND (will use CPU)");
    } else {
        println!("Found: {}", gpu);
    }

    // Check if gen-audio is installed
    print!("Checking for gen-audio... ");
    let install_check = conn.exec("which gen-audio 2>/dev/null || echo 'NOT_FOUND'").await?;
    if install_check.trim() == "NOT_FOUND" {
        println!("NOT INSTALLED");
        println!();
        println!("To install gen-audio on the worker, SSH in and run:");
        println!("  cargo install --git https://github.com/your/repo gen-audiobook");
        println!("  gen-audio worker install");
    } else {
        println!("OK");

        // Run worker install
        print!("Running worker install... ");
        let install_result = conn.exec("gen-audio worker install 2>&1").await;
        match install_result {
            Ok(output) => {
                println!("OK");
                if !output.is_empty() {
                    println!("{}", output);
                }
            }
            Err(e) => {
                println!("FAILED: {}", e);
            }
        }
    }

    Ok(())
}

/// Compute SHA256 hash of a file.
pub fn compute_file_hash(path: &PathBuf) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash)[..16].to_string())
}
