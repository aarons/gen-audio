//! FFmpeg metadata generation for M4B chapter markers.

use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Information about a chapter for M4B metadata.
#[derive(Debug, Clone)]
pub struct ChapterInfo {
    /// Chapter title
    pub title: String,
    /// Start position in milliseconds
    pub start_ms: u64,
    /// End position in milliseconds
    pub end_ms: u64,
}

impl ChapterInfo {
    /// Create a new chapter info.
    pub fn new(title: impl Into<String>, start_ms: u64, end_ms: u64) -> Self {
        Self {
            title: title.into(),
            start_ms,
            end_ms,
        }
    }
}

/// Create an FFmpeg metadata file for M4B chapters.
///
/// The FFMETADATA1 format is FFmpeg's native metadata format for chapter markers.
///
/// # Arguments
/// * `title` - Book title
/// * `author` - Book author
/// * `chapters` - List of chapter information
/// * `output_path` - Path to write the metadata file
pub fn create_ffmpeg_metadata(
    title: &str,
    author: &str,
    chapters: &[ChapterInfo],
    output_path: &Path,
) -> Result<()> {
    let mut file = File::create(output_path).context("Failed to create metadata file")?;

    // Write header and global metadata
    writeln!(file, ";FFMETADATA1")?;
    writeln!(file, "title={}", escape_metadata_value(title))?;
    writeln!(file, "artist={}", escape_metadata_value(author))?;
    writeln!(file, "album={}", escape_metadata_value(title))?;
    writeln!(file, "genre=Audiobook")?;
    writeln!(file)?;

    // Write chapter markers
    for chapter in chapters {
        writeln!(file, "[CHAPTER]")?;
        writeln!(file, "TIMEBASE=1/1000")?;
        writeln!(file, "START={}", chapter.start_ms)?;
        writeln!(file, "END={}", chapter.end_ms)?;
        writeln!(file, "title={}", escape_metadata_value(&chapter.title))?;
        writeln!(file)?;
    }

    Ok(())
}

/// Escape special characters in metadata values.
///
/// FFmpeg metadata values need to escape: = ; # \ and newlines
fn escape_metadata_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for c in value.chars() {
        match c {
            '=' | ';' | '#' | '\\' => {
                escaped.push('\\');
                escaped.push(c);
            }
            '\n' => {
                escaped.push_str("\\n");
            }
            '\r' => {
                // Skip carriage returns
            }
            _ => {
                escaped.push(c);
            }
        }
    }

    escaped
}

/// Build chapter info from audio file durations and chapter boundaries.
///
/// # Arguments
/// * `chunk_durations_ms` - Duration of each chunk in milliseconds
/// * `chapter_boundaries` - List of (chapter_title, first_chunk_index) tuples
pub fn build_chapter_info(
    chunk_durations_ms: &[u64],
    chapter_boundaries: &[(String, usize)],
) -> Vec<ChapterInfo> {
    let mut chapters = Vec::new();

    for (i, (title, first_chunk)) in chapter_boundaries.iter().enumerate() {
        // Find end chunk (start of next chapter or end of file)
        let end_chunk = if i + 1 < chapter_boundaries.len() {
            chapter_boundaries[i + 1].1
        } else {
            chunk_durations_ms.len()
        };

        // Calculate start and end times
        let start_ms: u64 = chunk_durations_ms[..*first_chunk].iter().sum();
        let end_ms: u64 = chunk_durations_ms[..end_chunk].iter().sum();

        chapters.push(ChapterInfo::new(title.clone(), start_ms, end_ms));
    }

    chapters
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_chapter_info_new() {
        let chapter = ChapterInfo::new("Chapter 1", 0, 60000);
        assert_eq!(chapter.title, "Chapter 1");
        assert_eq!(chapter.start_ms, 0);
        assert_eq!(chapter.end_ms, 60000);
    }

    #[test]
    fn test_escape_metadata_value() {
        assert_eq!(escape_metadata_value("Simple"), "Simple");
        assert_eq!(escape_metadata_value("Test=Value"), "Test\\=Value");
        assert_eq!(escape_metadata_value("Test;Value"), "Test\\;Value");
        assert_eq!(escape_metadata_value("Test#Value"), "Test\\#Value");
        assert_eq!(escape_metadata_value("Test\\Value"), "Test\\\\Value");
        assert_eq!(escape_metadata_value("Line1\nLine2"), "Line1\\nLine2");
    }

    #[test]
    fn test_create_ffmpeg_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let metadata_path = temp_dir.path().join("metadata.txt");

        let chapters = vec![
            ChapterInfo::new("Chapter 1", 0, 60000),
            ChapterInfo::new("Chapter 2", 60000, 120000),
        ];

        create_ffmpeg_metadata("My Book", "John Author", &chapters, &metadata_path).unwrap();

        let content = std::fs::read_to_string(&metadata_path).unwrap();
        assert!(content.contains(";FFMETADATA1"));
        assert!(content.contains("title=My Book"));
        assert!(content.contains("artist=John Author"));
        assert!(content.contains("[CHAPTER]"));
        assert!(content.contains("START=0"));
        assert!(content.contains("END=60000"));
        assert!(content.contains("title=Chapter 1"));
    }

    #[test]
    fn test_build_chapter_info() {
        let chunk_durations = vec![1000, 2000, 3000, 4000, 5000];
        let boundaries = vec![
            ("Chapter 1".to_string(), 0),
            ("Chapter 2".to_string(), 2),
            ("Chapter 3".to_string(), 4),
        ];

        let chapters = build_chapter_info(&chunk_durations, &boundaries);

        assert_eq!(chapters.len(), 3);

        // Chapter 1: chunks 0-1, duration 1000+2000 = 3000
        assert_eq!(chapters[0].title, "Chapter 1");
        assert_eq!(chapters[0].start_ms, 0);
        assert_eq!(chapters[0].end_ms, 3000);

        // Chapter 2: chunks 2-3, duration 3000+4000 = 7000
        assert_eq!(chapters[1].title, "Chapter 2");
        assert_eq!(chapters[1].start_ms, 3000);
        assert_eq!(chapters[1].end_ms, 10000);

        // Chapter 3: chunk 4, duration 5000
        assert_eq!(chapters[2].title, "Chapter 3");
        assert_eq!(chapters[2].start_ms, 10000);
        assert_eq!(chapters[2].end_ms, 15000);
    }
}
