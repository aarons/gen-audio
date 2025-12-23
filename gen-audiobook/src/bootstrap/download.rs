//! HTTP download with progress reporting and retry logic.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

/// Download-related errors.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum DownloadError {
    #[error("Download failed after {attempts} attempts: {message}")]
    Failed { attempts: u32, message: String },

    #[error("HTTP error: {status} for {url}")]
    HttpError { status: u16, url: String },

    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Configuration for retry behavior.
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2.0,
        }
    }
}

/// Download a file with progress bar display.
pub async fn download_file(url: &str, destination: &Path, description: &str) -> Result<()> {
    download_file_with_retry(url, destination, description, &RetryConfig::default()).await
}

/// Download a file with progress bar and retry logic.
pub async fn download_file_with_retry(
    url: &str,
    destination: &Path,
    description: &str,
    config: &RetryConfig,
) -> Result<()> {
    let mut attempt = 0;
    let mut delay = config.initial_delay;

    loop {
        attempt += 1;

        match download_file_once(url, destination, description).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                if attempt >= config.max_attempts {
                    return Err(e).context(format!(
                        "Download failed after {} attempts",
                        config.max_attempts
                    ));
                }

                eprintln!(
                    "  Download failed (attempt {}/{}): {}",
                    attempt, config.max_attempts, e
                );
                eprintln!("  Retrying in {:?}...", delay);

                tokio::time::sleep(delay).await;
                delay = Duration::from_secs_f32(
                    (delay.as_secs_f32() * config.backoff_factor).min(config.max_delay.as_secs_f32()),
                );
            }
        }
    }
}

/// Perform a single download attempt.
async fn download_file_once(url: &str, destination: &Path, description: &str) -> Result<()> {
    // Create parent directory if needed
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Build HTTP client with reasonable timeouts
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(600)) // 10 minutes for large files
        .build()
        .context("Failed to create HTTP client")?;

    // Start request
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to connect")?;

    let status = response.status();
    if !status.is_success() {
        return Err(DownloadError::HttpError {
            status: status.as_u16(),
            url: url.to_string(),
        }
        .into());
    }

    let total_size = response.content_length();

    // Create progress bar
    let pb = if let Some(size) = total_size {
        let pb = ProgressBar::new(size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  {msg}\n  {bar:40.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_message(description.to_string());
        pb
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("  {msg} {bytes} ({bytes_per_sec})")
                .unwrap(),
        );
        pb.set_message(description.to_string());
        pb
    };

    // Open destination file
    let mut file = std::fs::File::create(destination)
        .context("Failed to create destination file")?;

    // Download with streaming
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading response")?;
        file.write_all(&chunk).context("Failed to write to file")?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    pb.finish_and_clear();

    Ok(())
}

/// Format bytes for human-readable display.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_bytes(1536 * 1024), "1.5 MB");
    }

    #[test]
    fn test_default_retry_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay, Duration::from_secs(1));
    }
}
