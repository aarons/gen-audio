//! Version constants and tracking for bootstrapped components.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Pinned Python version for reproducible builds.
pub const PYTHON_VERSION: &str = "3.11.11";

/// Python build standalone release tag.
/// From: https://github.com/astral-sh/python-build-standalone/releases
pub const PYTHON_RELEASE_TAG: &str = "20241206";

/// FFmpeg version identifier (for tracking, actual version from download).
pub const FFMPEG_VERSION: &str = "7.1";

/// Installed component versions (persisted to versions.json).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstalledVersions {
    /// Installed Python version (e.g., "3.11.9").
    pub python_version: Option<String>,

    /// Python build standalone release tag (e.g., "20240713").
    pub python_release_tag: Option<String>,

    /// Installed FFmpeg version.
    pub ffmpeg_version: Option<String>,

    /// Platform string when installed (e.g., "macOS-aarch64").
    pub platform: Option<String>,

    /// When the installation occurred.
    pub installed_at: Option<DateTime<Utc>>,
}

impl InstalledVersions {
    /// Load installed versions from the versions.json file.
    pub fn load(data_dir: &std::path::Path) -> Result<Self> {
        let versions_file = data_dir.join("bootstrap").join("versions.json");

        if !versions_file.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&versions_file)
            .context("Failed to read versions.json")?;

        serde_json::from_str(&content).context("Failed to parse versions.json")
    }

    /// Save installed versions to the versions.json file.
    pub fn save(&self, data_dir: &std::path::Path) -> Result<()> {
        let bootstrap_dir = data_dir.join("bootstrap");
        std::fs::create_dir_all(&bootstrap_dir)?;

        let versions_file = bootstrap_dir.join("versions.json");
        let content = serde_json::to_string_pretty(self)?;

        std::fs::write(&versions_file, content).context("Failed to write versions.json")
    }

    /// Check if the installed Python matches the current pinned version.
    #[allow(dead_code)]
    pub fn is_python_current(&self) -> bool {
        self.python_version.as_deref() == Some(PYTHON_VERSION)
            && self.python_release_tag.as_deref() == Some(PYTHON_RELEASE_TAG)
    }

    /// Check if the installed FFmpeg is present.
    #[allow(dead_code)]
    pub fn has_ffmpeg(&self) -> bool {
        self.ffmpeg_version.is_some()
    }

    /// Check if the platform has changed (e.g., Intel to Apple Silicon).
    pub fn platform_matches(&self, current_platform: &str) -> bool {
        self.platform.as_deref() == Some(current_platform)
    }

    /// Update Python version info.
    pub fn set_python(&mut self, version: &str, release_tag: &str) {
        self.python_version = Some(version.to_string());
        self.python_release_tag = Some(release_tag.to_string());
        self.installed_at = Some(Utc::now());
    }

    /// Update FFmpeg version info.
    pub fn set_ffmpeg(&mut self, version: &str) {
        self.ffmpeg_version = Some(version.to_string());
        self.installed_at = Some(Utc::now());
    }

    /// Set the platform string.
    pub fn set_platform(&mut self, platform: &str) {
        self.platform = Some(platform.to_string());
    }
}

/// Get the data directory for gen-audio.
pub fn get_data_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .map(|d| d.join("gen-audio"))
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

    std::fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

/// Get the bootstrap directory.
pub fn get_bootstrap_dir() -> Result<PathBuf> {
    Ok(get_data_dir()?.join("bootstrap"))
}

/// Get the Python installation directory.
pub fn get_python_dir() -> Result<PathBuf> {
    Ok(get_bootstrap_dir()?.join("python"))
}

/// Get the FFmpeg installation directory.
pub fn get_ffmpeg_dir() -> Result<PathBuf> {
    Ok(get_bootstrap_dir()?.join("ffmpeg"))
}

/// Get the venv directory.
pub fn get_venv_dir() -> Result<PathBuf> {
    Ok(get_data_dir()?.join("venv"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_versions() {
        let versions = InstalledVersions::default();
        assert!(!versions.is_python_current());
        assert!(!versions.has_ffmpeg());
    }

    #[test]
    fn test_python_current() {
        let mut versions = InstalledVersions::default();
        versions.set_python(PYTHON_VERSION, PYTHON_RELEASE_TAG);
        assert!(versions.is_python_current());
    }

    #[test]
    fn test_platform_matches() {
        let mut versions = InstalledVersions::default();
        versions.set_platform("macOS-aarch64");
        assert!(versions.platform_matches("macOS-aarch64"));
        assert!(!versions.platform_matches("Linux-x86_64"));
    }
}
