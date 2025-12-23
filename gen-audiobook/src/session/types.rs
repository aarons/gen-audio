//! Session data types for audiobook generation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Status of a single text chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkStatus {
    /// The chapter this chunk belongs to
    pub chapter_id: usize,
    /// The chunk index within the chapter
    pub chunk_id: usize,
    /// Path to the generated audio file (if completed)
    pub audio_path: Option<PathBuf>,
    /// Whether this chunk has been successfully processed
    pub completed: bool,
    /// Error message if processing failed
    pub error: Option<String>,
}

impl ChunkStatus {
    /// Create a new pending chunk status.
    pub fn new(chapter_id: usize, chunk_id: usize) -> Self {
        Self {
            chapter_id,
            chunk_id,
            audio_path: None,
            completed: false,
            error: None,
        }
    }

    /// Mark this chunk as completed with the given audio path.
    pub fn mark_completed(&mut self, audio_path: PathBuf) {
        self.audio_path = Some(audio_path);
        self.completed = true;
        self.error = None;
    }

    /// Mark this chunk as failed with the given error.
    pub fn mark_failed(&mut self, error: String) {
        self.error = Some(error);
        // Don't set completed = true for failed chunks
    }
}

/// Represents a generation session with checkpoint data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    pub session_id: String,
    /// Path to the source EPUB file
    pub book_path: PathBuf,
    /// SHA256 hash of the book (first 1MB)
    pub book_hash: String,
    /// Book title
    pub title: String,
    /// Book author
    pub author: String,
    /// Total number of chapters
    pub total_chapters: usize,
    /// Total number of text chunks
    pub total_chunks: usize,
    /// Status of each chunk
    pub chunks: Vec<ChunkStatus>,
    /// Current chapter being processed
    pub current_chapter: usize,
    /// Current chunk being processed
    pub current_chunk: usize,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last updated
    pub updated_at: DateTime<Utc>,
    /// Whether all chunks have been processed
    pub completed: bool,
}

impl Session {
    /// Create a new session.
    pub fn new(
        session_id: String,
        book_path: PathBuf,
        book_hash: String,
        title: String,
        author: String,
        chunks: Vec<ChunkStatus>,
    ) -> Self {
        let total_chapters = chunks
            .iter()
            .map(|c| c.chapter_id)
            .max()
            .map(|max| max + 1)
            .unwrap_or(0);
        let total_chunks = chunks.len();
        let now = Utc::now();

        Self {
            session_id,
            book_path,
            book_hash,
            title,
            author,
            total_chapters,
            total_chunks,
            chunks,
            current_chapter: 0,
            current_chunk: 0,
            created_at: now,
            updated_at: now,
            completed: false,
        }
    }

    /// Get the number of completed chunks.
    pub fn completed_count(&self) -> usize {
        self.chunks.iter().filter(|c| c.completed).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_status_new() {
        let status = ChunkStatus::new(0, 1);
        assert_eq!(status.chapter_id, 0);
        assert_eq!(status.chunk_id, 1);
        assert!(status.audio_path.is_none());
        assert!(!status.completed);
        assert!(status.error.is_none());
    }

    #[test]
    fn test_chunk_status_mark_completed() {
        let mut status = ChunkStatus::new(0, 0);
        status.mark_completed(PathBuf::from("/tmp/audio.wav"));
        assert!(status.completed);
        assert_eq!(
            status.audio_path,
            Some(PathBuf::from("/tmp/audio.wav"))
        );
    }

    #[test]
    fn test_chunk_status_mark_failed() {
        let mut status = ChunkStatus::new(0, 0);
        status.mark_failed("TTS failed".to_string());
        assert!(!status.completed);
        assert_eq!(status.error, Some("TTS failed".to_string()));
    }

    #[test]
    fn test_session_new() {
        let chunks = vec![
            ChunkStatus::new(0, 0),
            ChunkStatus::new(0, 1),
            ChunkStatus::new(1, 0),
        ];
        let session = Session::new(
            "test_session".to_string(),
            PathBuf::from("/tmp/book.epub"),
            "abc123".to_string(),
            "Test Book".to_string(),
            "Author".to_string(),
            chunks,
        );

        assert_eq!(session.session_id, "test_session");
        assert_eq!(session.total_chapters, 2);
        assert_eq!(session.total_chunks, 3);
        assert_eq!(session.completed_count(), 0);
    }
}
