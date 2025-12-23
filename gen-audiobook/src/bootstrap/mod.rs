//! Bootstrap module for managing Python and FFmpeg dependencies.
//!
//! This module handles automatic downloading and installation of:
//! - Portable Python from python-build-standalone
//! - Static FFmpeg/FFprobe binaries
//! - Python virtual environment with Chatterbox TTS dependencies

pub mod download;
pub mod ffmpeg;
pub mod platform;
pub mod python;
pub mod versions;

use anyhow::{Context, Result};
use platform::Platform;
use std::io::{self, Write};
use std::path::PathBuf;
use versions::{get_data_dir, InstalledVersions, FFMPEG_VERSION, PYTHON_RELEASE_TAG, PYTHON_VERSION};

/// Paths to bootstrapped components.
pub struct BootstrapPaths {
    /// Path to the Python executable in the venv.
    pub python: PathBuf,
    /// Path to FFmpeg executable.
    pub ffmpeg: PathBuf,
    /// Path to FFprobe executable.
    pub ffprobe: PathBuf,
}

/// Bootstrap status indicating what needs to be done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapStatus {
    /// Everything is ready to use.
    Ready,
    /// Full bootstrap is needed (first run).
    NeedsFullBootstrap,
    /// Only Python packages need to be installed.
    NeedsPackages,
    /// Platform has changed, needs reinstall.
    PlatformChanged,
}

/// Check the current bootstrap status.
pub fn check_status() -> Result<BootstrapStatus> {
    let platform = Platform::detect()?;
    let data_dir = get_data_dir()?;
    let versions = InstalledVersions::load(&data_dir)?;

    // Check if platform changed
    if versions.platform.is_some() && !versions.platform_matches(&platform.to_version_string()) {
        return Ok(BootstrapStatus::PlatformChanged);
    }

    // Check if Python is installed
    if !python::is_python_installed()? {
        return Ok(BootstrapStatus::NeedsFullBootstrap);
    }

    // Check if FFmpeg is installed
    if !ffmpeg::is_ffmpeg_installed()? {
        return Ok(BootstrapStatus::NeedsFullBootstrap);
    }

    // Check if venv exists
    if !python::is_venv_ready()? {
        return Ok(BootstrapStatus::NeedsPackages);
    }

    // Check if Chatterbox is installed
    if !python::is_chatterbox_installed()? {
        return Ok(BootstrapStatus::NeedsPackages);
    }

    Ok(BootstrapStatus::Ready)
}

/// Ensure the bootstrap is complete, running it if needed.
///
/// This is the main entry point for automatic bootstrapping.
pub async fn ensure_bootstrapped() -> Result<BootstrapPaths> {
    let status = check_status()?;

    match status {
        BootstrapStatus::Ready => {
            // Already bootstrapped
        }
        BootstrapStatus::NeedsFullBootstrap => {
            // Show confirmation prompt
            if !confirm_bootstrap()? {
                anyhow::bail!("Bootstrap cancelled by user");
            }
            run_full_bootstrap().await?;
        }
        BootstrapStatus::NeedsPackages => {
            eprintln!("Python packages need to be installed...\n");
            install_packages()?;
        }
        BootstrapStatus::PlatformChanged => {
            eprintln!("Platform has changed, reinstalling dependencies...\n");
            if !confirm_bootstrap()? {
                anyhow::bail!("Bootstrap cancelled by user");
            }
            run_full_bootstrap().await?;
        }
    }

    Ok(BootstrapPaths {
        python: python::get_venv_python()?,
        ffmpeg: ffmpeg::get_ffmpeg_executable()?,
        ffprobe: ffmpeg::get_ffprobe_executable()?,
    })
}

/// Show confirmation prompt for first-run bootstrap.
fn confirm_bootstrap() -> Result<bool> {
    eprintln!();
    eprintln!("gen-audiobook requires a one-time setup (~2.1 GB download):");
    eprintln!("  - Python {} (~25 MB)", PYTHON_VERSION);
    eprintln!("  - FFmpeg {} (~30 MB)", FFMPEG_VERSION);
    eprintln!("  - Chatterbox TTS + PyTorch (~2 GB)");
    eprintln!();
    eprintln!("All files will be stored in ~/.local/share/gena/");
    eprintln!("Run 'gena uninstall' to remove everything.");
    eprintln!();
    eprint!("Continue? [Y/n] ");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    Ok(input.is_empty() || input == "y" || input == "yes")
}

/// Run the full bootstrap process.
async fn run_full_bootstrap() -> Result<()> {
    let platform = Platform::detect()?;
    let data_dir = get_data_dir()?;
    let mut versions = InstalledVersions::load(&data_dir)?;

    eprintln!();

    // Step 1: Download Python
    eprintln!("[1/4] Downloading Python {}...", PYTHON_VERSION);
    python::install_python(&platform).await?;
    versions.set_python(PYTHON_VERSION, PYTHON_RELEASE_TAG);
    versions.set_platform(&platform.to_version_string());
    versions.save(&data_dir)?;

    // Step 2: Create venv
    eprintln!();
    eprintln!("[2/4] Setting up Python environment...");
    python::create_venv()?;

    // Step 3: Download FFmpeg
    eprintln!();
    eprintln!("[3/4] Downloading FFmpeg {}...", FFMPEG_VERSION);
    ffmpeg::install_ffmpeg(&platform).await?;
    versions.set_ffmpeg(FFMPEG_VERSION);
    versions.save(&data_dir)?;

    // Step 4: Install packages
    eprintln!();
    eprintln!("[4/4] Installing Chatterbox TTS... (this may take several minutes)");
    install_packages()?;

    eprintln!();
    eprintln!("Setup complete! Starting conversion...");
    eprintln!();

    Ok(())
}

/// Install Python packages into the venv.
fn install_packages() -> Result<()> {
    python::install_packages(|msg| {
        eprintln!("  {}", msg);
    })?;

    // Verify Chatterbox is installed
    if !python::is_chatterbox_installed()? {
        anyhow::bail!("Chatterbox installation verification failed");
    }

    Ok(())
}

/// Remove all bootstrap data (for uninstall command).
pub fn clean_all(include_models: bool) -> Result<CleanupStats> {
    let data_dir = get_data_dir()?;
    let mut stats = CleanupStats::default();

    // Calculate sizes before deletion
    if data_dir.exists() {
        stats.gena_size = dir_size(&data_dir).unwrap_or(0);
    }

    // Remove gena data directory
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).context("Failed to remove gena data directory")?;
        stats.gena_removed = true;
    }

    // Optionally remove HuggingFace model cache for Chatterbox
    if include_models {
        if let Some(cache_dir) = dirs::cache_dir() {
            let hf_cache = cache_dir.join("huggingface").join("hub");
            if hf_cache.exists() {
                // Find and remove Chatterbox models
                for entry in std::fs::read_dir(&hf_cache)? {
                    let entry = entry?;
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();

                    if name_str.contains("ResembleAI") || name_str.contains("chatterbox") {
                        let size = dir_size(&entry.path()).unwrap_or(0);
                        std::fs::remove_dir_all(entry.path())?;
                        stats.models_size += size;
                        stats.models_removed = true;
                    }
                }
            }
        }
    }

    Ok(stats)
}

/// Statistics about cleanup operation.
#[derive(Default)]
pub struct CleanupStats {
    pub gena_removed: bool,
    pub gena_size: u64,
    pub models_removed: bool,
    pub models_size: u64,
}

impl CleanupStats {
    pub fn total_size(&self) -> u64 {
        self.gena_size + self.models_size
    }
}

/// Calculate directory size recursively.
fn dir_size(path: &std::path::Path) -> Result<u64> {
    let mut size = 0;

    if path.is_file() {
        return Ok(std::fs::metadata(path)?.len());
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            size += dir_size(&entry.path())?;
        } else {
            size += metadata.len();
        }
    }

    Ok(size)
}

/// Get information about the current bootstrap state.
pub fn get_info() -> Result<String> {
    let platform = Platform::detect()?;
    let data_dir = get_data_dir()?;
    let versions = InstalledVersions::load(&data_dir)?;
    let status = check_status()?;

    let mut info = String::new();

    info.push_str(&format!("Platform: {}\n", platform));
    info.push_str(&format!("Data directory: {:?}\n", data_dir));
    info.push_str(&format!("Bootstrap status: {:?}\n", status));
    info.push_str("\n");

    if let Some(ref v) = versions.python_version {
        info.push_str(&format!("Python version: {}\n", v));
    } else {
        info.push_str("Python: not installed\n");
    }

    if let Some(ref v) = versions.ffmpeg_version {
        info.push_str(&format!("FFmpeg version: {}\n", v));
    } else {
        info.push_str("FFmpeg: not installed\n");
    }

    info.push_str("\n");
    info.push_str(&python::get_env_info()?);

    info.push_str("\n");
    info.push_str(&format!(
        "FFmpeg installed: {}\n",
        ffmpeg::is_ffmpeg_installed().unwrap_or(false)
    ));
    info.push_str(&format!(
        "FFprobe installed: {}\n",
        ffmpeg::is_ffprobe_installed().unwrap_or(false)
    ));

    Ok(info)
}
