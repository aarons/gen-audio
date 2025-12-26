//! Worker types and utilities.
//!
//! TTS execution is handled by Python workers running gen-audio-worker.
//! This module provides:
//! - Protocol types (TtsJob, TtsResult, WorkerStatus)
//! - Path utilities for voice references and output files

pub mod executor;
pub mod protocol;

pub use executor::{
    get_worker_status, output_dir, voices_dir,
};

use anyhow::Result;
use clap::Subcommand;

/// Worker subcommands.
///
/// Note: The Rust worker is deprecated. TTS is now handled by Python workers.
/// Use `gen-audio workers` to manage remote workers.
#[derive(Subcommand, Debug)]
pub enum WorkerCommand {
    /// Report local worker status (deprecated - use remote workers).
    Status,

    /// Clean up local worker data (output files, voice cache).
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
            eprintln!("Note: Local Rust worker is deprecated. TTS is now handled by Python workers.");
            eprintln!("Use 'gen-audio workers status' to check remote workers.");
            eprintln!();

            let status = get_worker_status();
            let json = serde_json::to_string_pretty(&status)?;
            println!("{}", json);
            Ok(())
        }

        WorkerCommand::Clean { include_voices } => {
            handle_clean(*include_voices)
        }
    }
}

/// Handle worker clean command.
fn handle_clean(include_voices: bool) -> Result<()> {
    use anyhow::Context;

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
