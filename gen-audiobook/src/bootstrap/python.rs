//! Python download, extraction, and virtual environment setup.

use super::download::download_file;
use super::platform::Platform;
use super::versions::{
    get_python_dir, get_venv_dir, PYTHON_RELEASE_TAG, PYTHON_VERSION,
};
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;

/// Python packages required for Chatterbox TTS.
pub const REQUIRED_PACKAGES: &[&str] = &[
    "torch",
    "torchaudio",
    "soundfile",
    "chatterbox-tts @ git+https://github.com/resemble-ai/chatterbox.git",
];

/// Get the download URL for the portable Python build.
pub fn get_python_download_url(platform: &Platform) -> String {
    format!(
        "https://github.com/astral-sh/python-build-standalone/releases/download/{tag}/cpython-{version}+{tag}-{arch}-install_only.tar.gz",
        tag = PYTHON_RELEASE_TAG,
        version = PYTHON_VERSION,
        arch = platform.python_platform_string(),
    )
}

/// Get the path to the Python executable in the bootstrap directory.
pub fn get_python_executable() -> Result<PathBuf> {
    let python_dir = get_python_dir()?;
    Ok(python_dir.join("python").join("bin").join("python3"))
}

/// Get the path to the Python executable in the venv.
pub fn get_venv_python() -> Result<PathBuf> {
    let venv_dir = get_venv_dir()?;
    Ok(venv_dir.join("bin").join("python"))
}

/// Get the path to pip in the venv.
pub fn get_venv_pip() -> Result<PathBuf> {
    let venv_dir = get_venv_dir()?;
    Ok(venv_dir.join("bin").join("pip"))
}

/// Check if the bootstrapped Python is installed and working.
pub fn is_python_installed() -> Result<bool> {
    let python_path = get_python_executable()?;
    if !python_path.exists() {
        return Ok(false);
    }

    let output = Command::new(&python_path)
        .args(["--version"])
        .output()
        .context("Failed to run Python")?;

    Ok(output.status.success())
}

/// Check if the venv exists and has Python.
pub fn is_venv_ready() -> Result<bool> {
    let python_path = get_venv_python()?;
    if !python_path.exists() {
        return Ok(false);
    }

    let output = Command::new(&python_path)
        .args(["--version"])
        .output()
        .context("Failed to run venv Python")?;

    Ok(output.status.success())
}

/// Check if Chatterbox is installed in the venv.
pub fn is_chatterbox_installed() -> Result<bool> {
    let python_path = get_venv_python()?;
    if !python_path.exists() {
        return Ok(false);
    }

    let output = Command::new(&python_path)
        .args(["-c", "import chatterbox; print('ok')"])
        .output()
        .context("Failed to check Chatterbox")?;

    Ok(output.status.success())
}

/// Download and install the portable Python build.
pub async fn install_python(platform: &Platform) -> Result<PathBuf> {
    let python_dir = get_python_dir()?;
    let url = get_python_download_url(platform);

    // Create temp file for download
    let temp_dir = tempfile::tempdir()?;
    let archive_path = temp_dir.path().join("python.tar.gz");

    // Download
    download_file(
        &url,
        &archive_path,
        &format!("Downloading Python {}...", PYTHON_VERSION),
    )
    .await?;

    // Extract
    eprintln!("  Extracting Python...");
    extract_tar_gz(&archive_path, &python_dir)?;

    // Verify installation
    let python_path = get_python_executable()?;
    if !python_path.exists() {
        anyhow::bail!(
            "Python installation failed: executable not found at {:?}",
            python_path
        );
    }

    // Verify it runs
    let output = Command::new(&python_path)
        .args(["--version"])
        .output()
        .context("Failed to run installed Python")?;

    if !output.status.success() {
        anyhow::bail!("Python installation verification failed");
    }

    let version = String::from_utf8_lossy(&output.stdout);
    eprintln!("  Installed {}", version.trim());

    Ok(python_path)
}

/// Create a virtual environment using the bootstrapped Python.
pub fn create_venv() -> Result<()> {
    let python_path = get_python_executable()?;
    let venv_path = get_venv_dir()?;

    eprintln!("  Creating virtual environment...");

    // Remove existing venv if present
    if venv_path.exists() {
        std::fs::remove_dir_all(&venv_path)?;
    }

    let output = Command::new(&python_path)
        .args(["-m", "venv"])
        .arg(&venv_path)
        .output()
        .context("Failed to create virtual environment")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to create venv: {}", stderr);
    }

    eprintln!("  Virtual environment created.");
    Ok(())
}

/// Install a package using pip.
pub fn pip_install(package: &str, upgrade: bool) -> Result<()> {
    let pip_path = get_venv_pip()?;

    let mut args = vec!["install"];
    if upgrade {
        args.push("--upgrade");
    }
    args.push(package);

    let output = Command::new(&pip_path)
        .args(&args)
        .output()
        .context(format!("Failed to install {}", package))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("pip install {} failed: {}", package, stderr);
    }

    Ok(())
}

/// Install all required packages into the venv.
pub fn install_packages(progress_callback: impl Fn(&str)) -> Result<()> {
    // Upgrade pip first
    progress_callback("Upgrading pip...");
    pip_install("pip", true)?;

    // Install each package
    for (i, package) in REQUIRED_PACKAGES.iter().enumerate() {
        let package_name = package.split_whitespace().next().unwrap_or(package);
        progress_callback(&format!(
            "Installing {} ({}/{})...",
            package_name,
            i + 1,
            REQUIRED_PACKAGES.len()
        ));
        pip_install(package, false)?;
    }

    Ok(())
}

/// Extract a .tar.gz archive.
fn extract_tar_gz(archive_path: &Path, destination: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    // Create destination directory
    std::fs::create_dir_all(destination)?;

    archive
        .unpack(destination)
        .context("Failed to extract tar.gz archive")?;

    Ok(())
}

/// Get environment info for diagnostics.
pub fn get_env_info() -> Result<String> {
    let python_dir = get_python_dir()?;
    let python_path = get_python_executable()?;
    let venv_dir = get_venv_dir()?;
    let venv_python = get_venv_python()?;

    let mut info = String::new();
    info.push_str(&format!("Python dir: {:?}\n", python_dir));
    info.push_str(&format!("Python exists: {}\n", python_path.exists()));
    info.push_str(&format!("Venv dir: {:?}\n", venv_dir));
    info.push_str(&format!("Venv Python exists: {}\n", venv_python.exists()));

    if venv_python.exists() {
        if let Ok(output) = Command::new(&venv_python).args(["--version"]).output() {
            let version = String::from_utf8_lossy(&output.stdout);
            info.push_str(&format!("Python version: {}", version));
        }

        info.push_str(&format!(
            "Chatterbox installed: {}\n",
            is_chatterbox_installed().unwrap_or(false)
        ));
    }

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::platform::{Arch, Os};

    #[test]
    fn test_python_download_url() {
        let platform = Platform {
            os: Os::MacOs,
            arch: Arch::Aarch64,
        };
        let url = get_python_download_url(&platform);
        assert!(url.contains("aarch64-apple-darwin"));
        assert!(url.contains("install_only"));
        assert!(url.contains(PYTHON_VERSION));
    }

    #[test]
    fn test_python_paths() {
        let python_exec = get_python_executable().unwrap();
        assert!(python_exec.ends_with("python3"));

        let venv_python = get_venv_python().unwrap();
        assert!(venv_python.ends_with("python"));
    }
}
