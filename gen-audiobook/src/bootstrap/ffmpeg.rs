//! FFmpeg download and extraction.

use super::download::download_file;
use super::platform::{Arch, Os, Platform};
use super::versions::{get_ffmpeg_dir, FFMPEG_VERSION};
use anyhow::{Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Get the download URL for FFmpeg.
pub fn get_ffmpeg_download_url(platform: &Platform) -> &'static str {
    match (platform.os, platform.arch) {
        (Os::MacOs, Arch::Aarch64) => {
            "https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release/ffmpeg.zip"
        }
        (Os::MacOs, Arch::X86_64) => {
            "https://ffmpeg.martin-riedl.de/redirect/latest/macos/amd64/release/ffmpeg.zip"
        }
        (Os::Linux, Arch::X86_64) => {
            "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
        }
        (Os::Linux, Arch::Aarch64) => {
            "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz"
        }
    }
}

/// Get the download URL for FFprobe (macOS only, Linux bundles it).
pub fn get_ffprobe_download_url(platform: &Platform) -> Option<&'static str> {
    match (platform.os, platform.arch) {
        (Os::MacOs, Arch::Aarch64) => {
            Some("https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release/ffprobe.zip")
        }
        (Os::MacOs, Arch::X86_64) => {
            Some("https://ffmpeg.martin-riedl.de/redirect/latest/macos/amd64/release/ffprobe.zip")
        }
        // Linux static builds include ffprobe
        (Os::Linux, _) => None,
    }
}

/// Get the path to the bootstrapped FFmpeg executable.
pub fn get_ffmpeg_executable() -> Result<PathBuf> {
    let ffmpeg_dir = get_ffmpeg_dir()?;
    Ok(ffmpeg_dir.join("ffmpeg"))
}

/// Get the path to the bootstrapped FFprobe executable.
pub fn get_ffprobe_executable() -> Result<PathBuf> {
    let ffmpeg_dir = get_ffmpeg_dir()?;
    Ok(ffmpeg_dir.join("ffprobe"))
}

/// Check if FFmpeg is installed and working.
pub fn is_ffmpeg_installed() -> Result<bool> {
    let ffmpeg_path = get_ffmpeg_executable()?;
    if !ffmpeg_path.exists() {
        return Ok(false);
    }

    let output = std::process::Command::new(&ffmpeg_path)
        .args(["-version"])
        .output()
        .context("Failed to run FFmpeg")?;

    Ok(output.status.success())
}

/// Check if FFprobe is installed and working.
pub fn is_ffprobe_installed() -> Result<bool> {
    let ffprobe_path = get_ffprobe_executable()?;
    if !ffprobe_path.exists() {
        return Ok(false);
    }

    let output = std::process::Command::new(&ffprobe_path)
        .args(["-version"])
        .output()
        .context("Failed to run FFprobe")?;

    Ok(output.status.success())
}

/// Download and install FFmpeg.
pub async fn install_ffmpeg(platform: &Platform) -> Result<(PathBuf, PathBuf)> {
    let ffmpeg_dir = get_ffmpeg_dir()?;
    std::fs::create_dir_all(&ffmpeg_dir)?;

    // Download FFmpeg
    let ffmpeg_url = get_ffmpeg_download_url(platform);
    let temp_dir = tempfile::tempdir()?;

    match platform.os {
        Os::MacOs => {
            // macOS: Separate downloads for ffmpeg and ffprobe (zip files)
            let ffmpeg_archive = temp_dir.path().join("ffmpeg.zip");
            download_file(
                ffmpeg_url,
                &ffmpeg_archive,
                &format!("Downloading FFmpeg {}...", FFMPEG_VERSION),
            )
            .await?;

            eprintln!("  Extracting FFmpeg...");
            extract_zip_single_binary(&ffmpeg_archive, &ffmpeg_dir.join("ffmpeg"))?;

            // Download ffprobe separately
            if let Some(ffprobe_url) = get_ffprobe_download_url(platform) {
                let ffprobe_archive = temp_dir.path().join("ffprobe.zip");
                download_file(ffprobe_url, &ffprobe_archive, "Downloading FFprobe...")
                    .await?;

                eprintln!("  Extracting FFprobe...");
                extract_zip_single_binary(&ffprobe_archive, &ffmpeg_dir.join("ffprobe"))?;
            }
        }
        Os::Linux => {
            // Linux: Single tar.xz with both ffmpeg and ffprobe
            let archive_path = temp_dir.path().join("ffmpeg.tar.xz");
            download_file(
                ffmpeg_url,
                &archive_path,
                &format!("Downloading FFmpeg {}...", FFMPEG_VERSION),
            )
            .await?;

            eprintln!("  Extracting FFmpeg...");
            extract_ffmpeg_tar_xz(&archive_path, &ffmpeg_dir)?;
        }
    }

    // Set executable permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let ffmpeg_path = ffmpeg_dir.join("ffmpeg");
        let ffprobe_path = ffmpeg_dir.join("ffprobe");

        if ffmpeg_path.exists() {
            let mut perms = std::fs::metadata(&ffmpeg_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&ffmpeg_path, perms)?;
        }

        if ffprobe_path.exists() {
            let mut perms = std::fs::metadata(&ffprobe_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&ffprobe_path, perms)?;
        }
    }

    let ffmpeg_path = get_ffmpeg_executable()?;
    let ffprobe_path = get_ffprobe_executable()?;

    // Verify installation
    if !ffmpeg_path.exists() {
        anyhow::bail!("FFmpeg installation failed: binary not found");
    }

    let output = std::process::Command::new(&ffmpeg_path)
        .args(["-version"])
        .output()
        .context("Failed to run installed FFmpeg")?;

    if !output.status.success() {
        anyhow::bail!("FFmpeg installation verification failed");
    }

    let version_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("unknown")
        .to_string();
    eprintln!("  Installed {}", version_line);

    Ok((ffmpeg_path, ffprobe_path))
}

/// Extract a single binary from a zip file (macOS FFmpeg distribution).
fn extract_zip_single_binary(archive_path: &Path, destination: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Find the binary in the archive (usually just the binary name)
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        // Skip directories and non-binary files
        if entry.is_dir() {
            continue;
        }

        // The macOS zip typically contains just the binary
        if name.contains("ffmpeg") || name.contains("ffprobe") {
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            std::fs::write(destination, content)?;
            return Ok(());
        }
    }

    // If no matching file found, just extract the first file
    if archive.len() > 0 {
        let mut entry = archive.by_index(0)?;
        if !entry.is_dir() {
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            std::fs::write(destination, content)?;
            return Ok(());
        }
    }

    anyhow::bail!("No binary found in zip archive")
}

/// Extract ffmpeg and ffprobe from Linux static build tar.xz.
fn extract_ffmpeg_tar_xz(archive_path: &Path, destination: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path)?;
    let decoder = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let path_str = path.to_string_lossy();

        // Extract ffmpeg and ffprobe binaries
        if path_str.ends_with("/ffmpeg") || path_str.ends_with("/ffprobe") {
            let filename = path.file_name().unwrap();
            let dest_path = destination.join(filename);

            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            std::fs::write(&dest_path, content)?;
        }
    }

    // Verify both binaries were extracted
    if !destination.join("ffmpeg").exists() {
        anyhow::bail!("ffmpeg binary not found in archive");
    }
    if !destination.join("ffprobe").exists() {
        anyhow::bail!("ffprobe binary not found in archive");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_urls() {
        let macos_arm = Platform {
            os: Os::MacOs,
            arch: Arch::Aarch64,
        };
        let url = get_ffmpeg_download_url(&macos_arm);
        assert!(url.contains("arm64"));

        let linux_x64 = Platform {
            os: Os::Linux,
            arch: Arch::X86_64,
        };
        let url = get_ffmpeg_download_url(&linux_x64);
        assert!(url.contains("amd64"));
    }

    #[test]
    fn test_ffprobe_urls() {
        let macos = Platform {
            os: Os::MacOs,
            arch: Arch::Aarch64,
        };
        assert!(get_ffprobe_download_url(&macos).is_some());

        let linux = Platform {
            os: Os::Linux,
            arch: Arch::X86_64,
        };
        assert!(get_ffprobe_download_url(&linux).is_none());
    }
}
