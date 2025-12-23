//! Audio file assembly using FFmpeg.

use super::metadata::{build_chapter_info, create_ffmpeg_metadata};
use crate::bootstrap::ffmpeg as bootstrap_ffmpeg;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Get the FFmpeg command, preferring bootstrapped version.
fn ffmpeg_command() -> Command {
    if let Ok(path) = bootstrap_ffmpeg::get_ffmpeg_executable() {
        if path.exists() {
            return Command::new(path);
        }
    }
    // Fallback to system ffmpeg
    Command::new("ffmpeg")
}

/// Get the FFprobe command, preferring bootstrapped version.
fn ffprobe_command() -> Command {
    if let Ok(path) = bootstrap_ffmpeg::get_ffprobe_executable() {
        if path.exists() {
            return Command::new(path);
        }
    }
    // Fallback to system ffprobe
    Command::new("ffprobe")
}

/// Get duration of an audio file in milliseconds using ffprobe.
pub fn get_audio_duration_ms(audio_path: &Path) -> Result<u64> {
    let output = ffprobe_command()
        .args([
            "-v",
            "quiet",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(audio_path)
        .output()
        .context("Failed to run ffprobe")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffprobe failed: {}", stderr);
    }

    let duration_str = String::from_utf8_lossy(&output.stdout);
    let duration_secs: f64 = duration_str
        .trim()
        .parse()
        .context("Failed to parse duration")?;

    Ok((duration_secs * 1000.0) as u64)
}

/// Concatenate multiple audio files into one.
///
/// Uses FFmpeg's concat demuxer for lossless concatenation of same-format files.
pub fn concatenate_audio_files(audio_files: &[&Path], output_path: &Path) -> Result<()> {
    if audio_files.is_empty() {
        anyhow::bail!("No audio files provided");
    }

    if audio_files.len() == 1 {
        // Just copy the single file
        std::fs::copy(audio_files[0], output_path)?;
        return Ok(());
    }

    // Create a temporary file list for ffmpeg
    let temp_dir = TempDir::new()?;
    let list_file = temp_dir.path().join("concat_list.txt");

    let mut list_content = String::new();
    for path in audio_files {
        // Escape single quotes in path
        let path_str = path.to_string_lossy().replace('\'', "'\\''");
        list_content.push_str(&format!("file '{}'\n", path_str));
    }
    std::fs::write(&list_file, &list_content)?;

    let output = ffmpeg_command()
        .args(["-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list_file)
        .args(["-c", "copy"])
        .arg(output_path)
        .output()
        .context("Failed to run ffmpeg concat")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffmpeg concat failed: {}", stderr);
    }

    Ok(())
}

/// Assemble audio chunks into a single M4B audiobook with chapter markers.
///
/// # Arguments
/// * `all_audio_files` - List of all audio chunk files in order
/// * `chapter_boundaries` - List of (chapter_title, first_chunk_index) tuples
/// * `output_path` - Path for the output M4B file
/// * `title` - Book title
/// * `author` - Book author
/// * `cover_image` - Optional path to cover image
pub fn assemble_m4b(
    all_audio_files: &[&Path],
    chapter_boundaries: &[(String, usize)],
    output_path: &Path,
    title: &str,
    author: &str,
    cover_image: Option<&Path>,
) -> Result<()> {
    if all_audio_files.is_empty() {
        anyhow::bail!("No audio files provided");
    }

    let temp_dir = TempDir::new()?;

    // Calculate chunk durations
    let mut chunk_durations = Vec::with_capacity(all_audio_files.len());
    for file in all_audio_files {
        chunk_durations.push(get_audio_duration_ms(file)?);
    }

    // Build chapter info
    let chapters = build_chapter_info(&chunk_durations, chapter_boundaries);

    // Concatenate all audio files
    let all_audio_wav = temp_dir.path().join("all_audio.wav");
    concatenate_audio_files(all_audio_files, &all_audio_wav)?;

    // Create metadata file
    let metadata_file = temp_dir.path().join("metadata.txt");
    create_ffmpeg_metadata(title, author, &chapters, &metadata_file)?;

    // Build ffmpeg command for final M4B
    let mut cmd = ffmpeg_command();
    cmd.args(["-y", "-i"])
        .arg(&all_audio_wav)
        .args(["-i"])
        .arg(&metadata_file);

    // Add cover image if provided
    if let Some(cover) = cover_image {
        if cover.exists() {
            cmd.args(["-i"]).arg(cover);
            cmd.args([
                "-map",
                "0:a",
                "-map",
                "2:v",
                "-c:v",
                "copy",
                "-disposition:v:0",
                "attached_pic",
            ]);
        } else {
            cmd.args(["-map", "0:a"]);
        }
    } else {
        cmd.args(["-map", "0:a"]);
    }

    // Add metadata mapping and encoding settings
    cmd.args([
        "-map_metadata",
        "1",
        "-c:a",
        "aac",
        "-b:a",
        "128k",
        "-f",
        "mp4",
    ])
    .arg(output_path);

    let output = cmd.output().context("Failed to run ffmpeg M4B creation")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffmpeg M4B creation failed: {}", stderr);
    }

    Ok(())
}

/// Check if FFmpeg is available (bootstrapped or system).
pub fn is_ffmpeg_available() -> bool {
    ffmpeg_command()
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if FFprobe is available (bootstrapped or system).
pub fn is_ffprobe_available() -> bool {
    ffprobe_command()
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_available() {
        // This test just checks the function doesn't panic
        let _ = is_ffmpeg_available();
    }

    #[test]
    fn test_ffprobe_available() {
        // This test just checks the function doesn't panic
        let _ = is_ffprobe_available();
    }

    // Note: Full integration tests for audio assembly would require actual audio files
    // and FFmpeg to be installed. These are better suited for integration tests.
}
