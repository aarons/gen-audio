//! Session persistence: loading, saving, and managing sessions.

use super::types::{ChunkStatus, Session};
use crate::text::TextChunk;
use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};

/// Get the base data directory for gena.
fn get_data_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .map(|d| d.join("gena"))
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

    Ok(data_dir)
}

/// Get the sessions directory path.
pub fn get_sessions_dir() -> Result<PathBuf> {
    let sessions_dir = get_data_dir()?.join("sessions");
    fs::create_dir_all(&sessions_dir)?;
    Ok(sessions_dir)
}

/// Get the temp directory for a session's audio files.
pub fn get_temp_dir(session_id: &str) -> Result<PathBuf> {
    let temp_dir = get_data_dir()?.join("temp").join(session_id);
    fs::create_dir_all(&temp_dir)?;
    Ok(temp_dir)
}

/// Compute a hash of the book file for session identification.
///
/// Uses SHA256 of the first 1MB for speed with large files.
pub fn compute_book_hash(book_path: &Path) -> Result<String> {
    let file = File::open(book_path).context("Failed to open book file for hashing")?;
    let mut reader = BufReader::new(file);

    // Read first 1MB
    let mut buffer = vec![0u8; 1024 * 1024];
    let bytes_read = reader.read(&mut buffer)?;
    buffer.truncate(bytes_read);

    // Compute SHA256
    let mut hasher = Sha256::new();
    hasher.update(&buffer);
    let result = hasher.finalize();

    // Return first 16 hex characters
    Ok(format!("{:x}", result)[..16].to_string())
}

/// Create a new generation session.
pub fn create_session(
    book_path: &Path,
    title: &str,
    author: &str,
    chunks: &[TextChunk],
) -> Result<Session> {
    let book_hash = compute_book_hash(book_path)?;
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let session_id = format!("{}_{}", book_hash, timestamp);

    // Create chunk status entries
    let chunk_statuses: Vec<ChunkStatus> = chunks
        .iter()
        .map(|c| ChunkStatus::new(c.chapter_id, c.chunk_id))
        .collect();

    let session = Session::new(
        session_id,
        book_path.to_path_buf(),
        book_hash,
        title.to_string(),
        author.to_string(),
        chunk_statuses,
    );

    // Save immediately
    save_session(&session)?;

    Ok(session)
}

/// Save session state to disk.
pub fn save_session(session: &Session) -> Result<()> {
    let sessions_dir = get_sessions_dir()?;
    let session_file = sessions_dir.join(format!("{}.json", session.session_id));

    // Create updated session with new timestamp
    let mut session = session.clone();
    session.updated_at = Utc::now();

    let file = File::create(&session_file).context("Failed to create session file")?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &session).context("Failed to write session JSON")?;

    Ok(())
}

/// Find the most recent incomplete session for a book.
pub fn find_session_for_book(book_path: &Path) -> Result<Option<Session>> {
    let book_hash = compute_book_hash(book_path)?;
    let sessions_dir = get_sessions_dir()?;

    // Find all sessions for this book
    let mut matching_sessions: Vec<Session> = Vec::new();

    for entry in fs::read_dir(&sessions_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(file) = File::open(&path) {
                let reader = BufReader::new(file);
                if let Ok(session) = serde_json::from_reader::<_, Session>(reader) {
                    if session.book_hash == book_hash && !session.completed {
                        matching_sessions.push(session);
                    }
                }
            }
        }
    }

    if matching_sessions.is_empty() {
        return Ok(None);
    }

    // Return the most recent one
    matching_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(matching_sessions.into_iter().next())
}

/// Mark a chunk as completed with its audio file path.
pub fn mark_chunk_complete(
    session: &mut Session,
    chapter_id: usize,
    chunk_id: usize,
    audio_path: &Path,
) -> Result<()> {
    // Find and update the chunk
    for chunk in &mut session.chunks {
        if chunk.chapter_id == chapter_id && chunk.chunk_id == chunk_id {
            chunk.mark_completed(audio_path.to_path_buf());
            break;
        }
    }

    // Update current position to next incomplete chunk
    if let Some((next_ch, next_chunk)) = get_next_chunk(session) {
        session.current_chapter = next_ch;
        session.current_chunk = next_chunk;
    } else {
        session.completed = true;
    }

    save_session(session)?;
    Ok(())
}

/// Mark a chunk as having an error.
pub fn mark_chunk_error(
    session: &mut Session,
    chapter_id: usize,
    chunk_id: usize,
    error: &str,
) -> Result<()> {
    for chunk in &mut session.chunks {
        if chunk.chapter_id == chapter_id && chunk.chunk_id == chunk_id {
            chunk.mark_failed(error.to_string());
            break;
        }
    }

    save_session(session)?;
    Ok(())
}

/// Get the next incomplete chunk's (chapter_id, chunk_id).
pub fn get_next_chunk(session: &Session) -> Option<(usize, usize)> {
    session
        .chunks
        .iter()
        .find(|c| !c.completed)
        .map(|c| (c.chapter_id, c.chunk_id))
}

/// Get progress as (completed, total, percentage).
pub fn get_progress(session: &Session) -> (usize, usize, f64) {
    let completed = session.completed_count();
    let total = session.total_chunks;
    let percentage = if total > 0 {
        completed as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    (completed, total, percentage)
}

/// Get all audio files for a chapter in order.
pub fn get_chapter_audio_files(session: &Session, chapter_id: usize) -> Vec<PathBuf> {
    let mut chapter_chunks: Vec<_> = session
        .chunks
        .iter()
        .filter(|c| c.chapter_id == chapter_id && c.completed && c.audio_path.is_some())
        .collect();

    chapter_chunks.sort_by_key(|c| c.chunk_id);
    chapter_chunks
        .into_iter()
        .filter_map(|c| c.audio_path.clone())
        .collect()
}

/// Clean up session data after successful audiobook generation.
///
/// Removes the session JSON file and temp audio directory.
pub fn cleanup_session(session: &Session) -> Result<()> {
    // Remove session JSON file
    let sessions_dir = get_sessions_dir()?;
    let session_file = sessions_dir.join(format!("{}.json", session.session_id));
    if session_file.exists() {
        fs::remove_file(&session_file).context("Failed to remove session file")?;
    }

    // Remove temp directory with audio chunks
    let temp_dir = get_data_dir()?.join("temp").join(&session.session_id);
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).context("Failed to remove temp directory")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_compute_book_hash() {
        let temp_dir = TempDir::new().unwrap();
        let book_path = temp_dir.path().join("test.epub");
        fs::write(&book_path, b"test content").unwrap();

        let hash = compute_book_hash(&book_path).unwrap();
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_compute_book_hash_consistent() {
        let temp_dir = TempDir::new().unwrap();
        let book_path = temp_dir.path().join("test.epub");
        fs::write(&book_path, b"consistent content").unwrap();

        let hash1 = compute_book_hash(&book_path).unwrap();
        let hash2 = compute_book_hash(&book_path).unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_get_next_chunk() {
        let chunks = vec![
            ChunkStatus::new(0, 0),
            ChunkStatus::new(0, 1),
            ChunkStatus::new(1, 0),
        ];
        let mut session = Session::new(
            "test".to_string(),
            PathBuf::from("/tmp/test.epub"),
            "abc".to_string(),
            "Test".to_string(),
            "Author".to_string(),
            chunks,
        );

        // First incomplete chunk
        assert_eq!(get_next_chunk(&session), Some((0, 0)));

        // Mark first as complete
        session.chunks[0].mark_completed(PathBuf::from("/tmp/0.wav"));
        assert_eq!(get_next_chunk(&session), Some((0, 1)));

        // Mark all complete
        session.chunks[1].mark_completed(PathBuf::from("/tmp/1.wav"));
        session.chunks[2].mark_completed(PathBuf::from("/tmp/2.wav"));
        assert_eq!(get_next_chunk(&session), None);
    }

    #[test]
    fn test_get_progress() {
        let mut chunks = vec![
            ChunkStatus::new(0, 0),
            ChunkStatus::new(0, 1),
            ChunkStatus::new(1, 0),
            ChunkStatus::new(1, 1),
        ];
        chunks[0].mark_completed(PathBuf::from("/tmp/0.wav"));

        let session = Session::new(
            "test".to_string(),
            PathBuf::from("/tmp/test.epub"),
            "abc".to_string(),
            "Test".to_string(),
            "Author".to_string(),
            chunks,
        );

        let (completed, total, pct) = get_progress(&session);
        assert_eq!(completed, 1);
        assert_eq!(total, 4);
        assert!((pct - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_get_chapter_audio_files() {
        let mut chunks = vec![
            ChunkStatus::new(0, 0),
            ChunkStatus::new(0, 1),
            ChunkStatus::new(0, 2),
            ChunkStatus::new(1, 0),
        ];
        chunks[0].mark_completed(PathBuf::from("/tmp/ch0_0.wav"));
        chunks[2].mark_completed(PathBuf::from("/tmp/ch0_2.wav"));
        // Note: chunk 1 is not complete
        chunks[3].mark_completed(PathBuf::from("/tmp/ch1_0.wav"));

        let session = Session::new(
            "test".to_string(),
            PathBuf::from("/tmp/test.epub"),
            "abc".to_string(),
            "Test".to_string(),
            "Author".to_string(),
            chunks,
        );

        let chapter_0_files = get_chapter_audio_files(&session, 0);
        assert_eq!(chapter_0_files.len(), 2);
        assert_eq!(chapter_0_files[0], PathBuf::from("/tmp/ch0_0.wav"));
        assert_eq!(chapter_0_files[1], PathBuf::from("/tmp/ch0_2.wav"));

        let chapter_1_files = get_chapter_audio_files(&session, 1);
        assert_eq!(chapter_1_files.len(), 1);
    }
}
