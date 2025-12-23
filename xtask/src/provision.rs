//! Python provisioning for development.
//!
//! Downloads a portable Python build from python-build-standalone if needed,
//! storing it in the target directory for use with PyO3.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Python version to download
const PYTHON_VERSION: &str = "3.11.11";
const PYTHON_RELEASE_TAG: &str = "20241206";

/// Get the workspace root directory.
pub fn workspace_root() -> Result<PathBuf> {
    let output = Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format=plain"])
        .output()
        .context("Failed to run cargo locate-project")?;

    if !output.status.success() {
        anyhow::bail!("cargo locate-project failed");
    }

    let cargo_toml = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 in cargo output")?
        .trim()
        .to_string();

    Ok(PathBuf::from(cargo_toml)
        .parent()
        .context("No parent directory")?
        .to_path_buf())
}

/// Get the directory where provisioned Python should be stored.
pub fn python_dev_dir() -> Result<PathBuf> {
    Ok(workspace_root()?.join("target").join("python-dev"))
}

/// Get the path to the provisioned Python executable.
pub fn python_executable() -> Result<PathBuf> {
    Ok(python_dev_dir()?.join("python").join("bin").join("python3"))
}

/// Check if Python is already provisioned and working.
pub fn is_python_provisioned() -> Result<bool> {
    let python = python_executable()?;
    if !python.exists() {
        return Ok(false);
    }

    // Verify it runs
    let output = Command::new(&python).arg("--version").output();
    match output {
        Ok(o) => Ok(o.status.success()),
        Err(_) => Ok(false),
    }
}

/// Provision Python for development.
///
/// Downloads and extracts python-build-standalone if not already present.
pub fn provision_python() -> Result<PathBuf> {
    let python = python_executable()?;

    if is_python_provisioned()? {
        eprintln!("Python already provisioned at: {}", python.display());
        return Ok(python);
    }

    let install_dir = python_dev_dir()?;
    std::fs::create_dir_all(&install_dir)?;

    let url = get_python_url();
    eprintln!("Downloading Python {} from {}...", PYTHON_VERSION, url);

    // Download using curl
    let archive_path = install_dir.join("python.tar.gz");
    download_file(&url, &archive_path)?;

    // Extract
    eprintln!("Extracting Python...");
    extract_tar_gz(&archive_path, &install_dir)?;

    // Cleanup
    let _ = std::fs::remove_file(&archive_path);

    // Verify
    if !python.exists() {
        anyhow::bail!("Python executable not found after extraction");
    }

    let output = Command::new(&python).arg("--version").output()?;
    if !output.status.success() {
        anyhow::bail!("Provisioned Python doesn't work");
    }

    let version = String::from_utf8_lossy(&output.stdout);
    eprintln!("Python provisioned successfully: {}", version.trim());

    Ok(python)
}

/// Get the download URL for python-build-standalone.
fn get_python_url() -> String {
    let platform = get_platform_string();
    format!(
        "https://github.com/astral-sh/python-build-standalone/releases/download/{tag}/cpython-{version}+{tag}-{platform}-install_only.tar.gz",
        tag = PYTHON_RELEASE_TAG,
        version = PYTHON_VERSION,
        platform = platform,
    )
}

/// Get the platform string for python-build-standalone.
fn get_platform_string() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "aarch64-apple-darwin";

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "x86_64-apple-darwin";

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-unknown-linux-gnu";

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "aarch64-unknown-linux-gnu";

    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    )))]
    compile_error!("Unsupported platform for Python provisioning");
}

/// Download a file using curl.
fn download_file(url: &str, dest: &Path) -> Result<()> {
    let status = Command::new("curl")
        .args([
            "-fSL", // fail silently, show errors, follow redirects
            "--progress-bar",
            "-o",
        ])
        .arg(dest)
        .arg(url)
        .status()
        .context("Failed to run curl")?;

    if !status.success() {
        anyhow::bail!("curl download failed");
    }

    Ok(())
}

/// Extract a tar.gz archive.
fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .context("Failed to run tar")?;

    if !status.success() {
        anyhow::bail!("tar extraction failed");
    }

    Ok(())
}
