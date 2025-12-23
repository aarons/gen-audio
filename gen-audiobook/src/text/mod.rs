//! Text processing module for TTS: chunking, cleaning, and sentence splitting.

pub mod chunker;
mod cleaner;
mod seams;

pub use chunker::process_chapter;

/// A chunk of text ready for TTS processing.
#[derive(Debug, Clone)]
pub struct TextChunk {
    /// The chapter this chunk belongs to
    pub chapter_id: usize,
    /// The chunk index within the chapter
    pub chunk_id: usize,
    /// The text content
    pub text: String,
}

impl TextChunk {
    /// Create a new text chunk.
    pub fn new(chapter_id: usize, chunk_id: usize, text: String) -> Self {
        Self {
            chapter_id,
            chunk_id,
            text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_chunk_creation() {
        let chunk = TextChunk::new(0, 1, "Hello world".to_string());
        assert_eq!(chunk.chapter_id, 0);
        assert_eq!(chunk.chunk_id, 1);
        assert_eq!(chunk.text, "Hello world");
    }
}
