//! Worker mode for distributed TTS processing.
//!
//! This module provides the worker-side implementation for distributed audiobook
//! generation. Workers receive jobs via stdin (over SSH) and return results via stdout.
//!
//! # Usage
//!
//! ```bash
//! # Check worker status
//! gen-audio worker status
//!
//! # Execute a job (coordinator sends job via SSH)
//! ssh worker "gen-audio worker run" < job.json
//!
//! # Self-install on a new machine
//! gen-audio worker install
//! ```

pub mod executor;
pub mod protocol;

pub use executor::{
    execute_job_from_stdin,
    get_worker_status, output_dir, voices_dir,
};

use anyhow::{Context, Result};
use clap::Subcommand;

/// Worker subcommands.
#[derive(Subcommand, Debug)]
pub enum WorkerCommand {
    /// Report worker status as JSON (for coordinator health checks).
    Status,

    /// Execute a single job from stdin, output result to stdout.
    Run,

    /// Self-install gen-audio on this machine (download dependencies).
    Install {
        /// Force reinstall even if already installed.
        #[arg(long)]
        force: bool,
    },

    /// Clean up worker data (output files, voice cache).
    Clean {
        /// Also remove voice reference cache.
        #[arg(long)]
        include_voices: bool,
    },
}

/// Handle worker subcommands.
pub async fn handle_worker_command(cmd: &WorkerCommand) -> Result<()> {
    match cmd {
        WorkerCommand::Status => {
            let status = get_worker_status();
            let json = serde_json::to_string_pretty(&status)
                .context("Failed to serialize status")?;
            println!("{}", json);
            Ok(())
        }

        WorkerCommand::Run => {
            execute_job_from_stdin().await
        }

        WorkerCommand::Install { force } => {
            handle_install(*force).await
        }

        WorkerCommand::Clean { include_voices } => {
            handle_clean(*include_voices)
        }
    }
}

/// Handle worker install command.
async fn handle_install(force: bool) -> Result<()> {
    use crate::bootstrap;

    println!("Installing gen-audio worker dependencies...");

    // Check current status
    let status = bootstrap::check_status()?;

    if status == bootstrap::BootstrapStatus::Ready && !force {
        println!("Worker is already set up. Use --force to reinstall.");
        return Ok(());
    }

    // Run bootstrap
    bootstrap::ensure_bootstrapped().await?;

    // Create worker directories
    std::fs::create_dir_all(voices_dir())
        .context("Failed to create voices directory")?;
    std::fs::create_dir_all(output_dir())
        .context("Failed to create output directory")?;

    println!("Worker installation complete!");
    println!();

    // Show status
    let status = get_worker_status();
    println!("Device: {}", status.device);
    println!("Chatterbox: {}", if status.chatterbox_installed { "installed" } else { "not installed" });
    println!("Ready: {}", if status.ready { "yes" } else { "no" });

    Ok(())
}

/// Handle worker clean command.
fn handle_clean(include_voices: bool) -> Result<()> {
    // Clean output directory
    let output = output_dir();
    if output.exists() {
        let count = std::fs::read_dir(&output)
            .map(|entries| entries.count())
            .unwrap_or(0);
        std::fs::remove_dir_all(&output)
            .context("Failed to remove output directory")?;
        std::fs::create_dir_all(&output)
            .context("Failed to recreate output directory")?;
        println!("Removed {} output files", count);
    }

    // Optionally clean voices directory
    if include_voices {
        let voices = voices_dir();
        if voices.exists() {
            let count = std::fs::read_dir(&voices)
                .map(|entries| entries.count())
                .unwrap_or(0);
            std::fs::remove_dir_all(&voices)
                .context("Failed to remove voices directory")?;
            std::fs::create_dir_all(&voices)
                .context("Failed to recreate voices directory")?;
            println!("Removed {} voice reference files", count);
        }
    }

    println!("Worker cleanup complete.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::protocol::PROTOCOL_VERSION;

    #[test]
    fn test_protocol_version() {
        assert!(PROTOCOL_VERSION >= 1);
    }
}
