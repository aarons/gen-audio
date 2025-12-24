//! Text chunking for TTS processing.

use super::cleaner::clean_text;
use super::seams::split_into_sentences;
use super::TextChunk;

/// Default target chunk size in characters.
pub const DEFAULT_TARGET_SIZE: usize = 280;

/// Maximum recursion depth for splitting long sentences.
const MAX_SPLIT_DEPTH: usize = 10;

/// Split text into TTS-friendly chunks.
///
/// # Arguments
/// * `text` - The text to chunk
/// * `target_size` - Target chunk size in characters (default: 280)
/// * `max_size` - Maximum chunk size, will split longer sentences (default: 350)
///
/// # Returns
/// List of text chunks suitable for TTS processing.
pub fn chunk_text(text: &str, target_size: usize, max_size: usize) -> Vec<String> {
    // Clean the text first
    let text = clean_text(text);

    if text.is_empty() {
        return Vec::new();
    }

    // Split into sentences
    let sentences = split_into_sentences(&text);

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for sentence in sentences {
        // Split long sentences
        if sentence.len() > max_size {
            // Flush current chunk first
            if !current_chunk.is_empty() {
                chunks.push(current_chunk.trim().to_string());
                current_chunk = String::new();
            }

            // Add split sentence parts
            let parts = split_long_sentence(&sentence, target_size, 0);
            for part in parts {
                if !part.is_empty() {
                    chunks.push(part);
                }
            }
        } else if current_chunk.len() + sentence.len() + 1 <= target_size {
            // Add to current chunk
            if !current_chunk.is_empty() {
                current_chunk.push(' ');
            }
            current_chunk.push_str(&sentence);
        } else {
            // Start new chunk
            if !current_chunk.is_empty() {
                chunks.push(current_chunk.trim().to_string());
            }
            current_chunk = sentence;
        }
    }

    // Don't forget the last chunk
    if !current_chunk.is_empty() {
        let trimmed = current_chunk.trim().to_string();
        if !trimmed.is_empty() {
            chunks.push(trimmed);
        }
    }

    chunks
}

/// Split a long sentence into smaller chunks at natural break points.
fn split_long_sentence(sentence: &str, max_length: usize, depth: usize) -> Vec<String> {
    // Prevent infinite recursion
    if depth > MAX_SPLIT_DEPTH {
        // Hard split at max_length as last resort
        return hard_split(sentence, max_length);
    }

    if sentence.len() <= max_length {
        return vec![sentence.to_string()];
    }

    // Try splitting on various delimiters in order of preference
    const DELIMITERS: &[&str] = &[";", ":", ",", " - ", " — ", " – "];

    for delimiter in DELIMITERS {
        if sentence.contains(delimiter) {
            let parts: Vec<&str> = sentence.split(delimiter).collect();
            if parts.len() > 1 {
                let chunks = reassemble_parts(&parts, delimiter, max_length);
                if chunks.len() > 1 {
                    // Recursively split any still-long chunks
                    let mut final_chunks = Vec::new();
                    for chunk in chunks {
                        if chunk.len() > max_length {
                            final_chunks.extend(split_long_sentence(&chunk, max_length, depth + 1));
                        } else if !chunk.is_empty() {
                            final_chunks.push(chunk);
                        }
                    }
                    return final_chunks;
                }
            }
        }
    }

    // Try splitting on word boundaries
    let word_split = split_on_words(sentence, max_length);
    if word_split.len() > 1 {
        // Recursively split any still-long chunks (single words longer than max_length)
        let mut final_chunks = Vec::new();
        for chunk in word_split {
            if chunk.len() > max_length {
                final_chunks.extend(hard_split(&chunk, max_length));
            } else if !chunk.is_empty() {
                final_chunks.push(chunk);
            }
        }
        return final_chunks;
    }

    // Last resort: hard split
    hard_split(sentence, max_length)
}

/// Reassemble split parts into chunks that fit within max_length.
fn reassemble_parts(parts: &[&str], delimiter: &str, max_length: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for (i, part) in parts.iter().enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Add delimiter back (except for first part)
        let with_delimiter = if i > 0 && !delimiter.trim().is_empty() {
            format!("{} {}", delimiter.trim(), part)
        } else {
            part.to_string()
        };

        if current.is_empty() {
            current = with_delimiter;
        } else if current.len() + with_delimiter.len() + 1 <= max_length {
            current.push(' ');
            current.push_str(&with_delimiter);
        } else {
            if !current.is_empty() {
                chunks.push(current.trim().to_string());
            }
            current = with_delimiter;
        }
    }

    if !current.is_empty() {
        chunks.push(current.trim().to_string());
    }

    chunks
}

/// Split text on word boundaries.
fn split_on_words(text: &str, max_length: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut chunks = Vec::new();
    let mut current = String::new();

    for word in words {
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + word.len() + 1 <= max_length {
            current.push(' ');
            current.push_str(word);
        } else {
            if !current.is_empty() {
                chunks.push(current);
            }
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

/// Hard split text at character boundaries, respecting byte length limits.
///
/// Splits on character boundaries (never breaking UTF-8 sequences) but ensures
/// each chunk is at most `max_length` bytes to be consistent with the rest of
/// the chunking code.
fn hard_split(text: &str, max_length: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for c in text.chars() {
        // Check if adding this char would exceed max_length bytes
        if current.len() + c.len_utf8() > max_length && !current.is_empty() {
            chunks.push(current);
            current = String::new();
        }
        current.push(c);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

/// Process a chapter's text into TTS-ready chunks.
///
/// # Arguments
/// * `chapter_id` - The chapter's index/ID
/// * `text` - The chapter text
/// * `target_size` - Target chunk size (default: 280)
///
/// # Returns
/// List of `TextChunk` objects.
pub fn process_chapter(chapter_id: usize, text: &str, target_size: usize) -> Vec<TextChunk> {
    let raw_chunks = chunk_text(text, target_size, target_size + 70);

    raw_chunks
        .into_iter()
        .enumerate()
        .map(|(chunk_id, text)| TextChunk::new(chapter_id, chunk_id, text))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_short_text() {
        let text = "Hello world. How are you?";
        let chunks = chunk_text(text, 280, 350);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hello world. How are you?");
    }

    #[test]
    fn test_chunk_long_text() {
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence. Fifth sentence. Sixth sentence. Seventh sentence. Eighth sentence. Ninth sentence. Tenth sentence.";
        let chunks = chunk_text(text, 50, 100);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(
                chunk.len() <= 100,
                "Chunk too long: {} chars",
                chunk.len()
            );
        }
    }

    #[test]
    fn test_chunk_empty_text() {
        let chunks = chunk_text("", 280, 350);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_whitespace_only() {
        let chunks = chunk_text("   \n\n   ", 280, 350);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_split_long_sentence() {
        let sentence = "This is a very long sentence with many parts; it has semicolons, commas, and other punctuation - all of which can serve as natural break points for splitting.";
        let parts = split_long_sentence(sentence, 50, 0);
        assert!(parts.len() > 1);
        for part in &parts {
            assert!(part.len() <= 50 || parts.len() == 1, "Part too long: {}", part);
        }
    }

    #[test]
    fn test_process_chapter() {
        let text = "Hello world. This is a test.";
        let chunks = process_chapter(0, text, 280);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chapter_id, 0);
        assert_eq!(chunks[0].chunk_id, 0);
        assert_eq!(chunks[0].text, "Hello world. This is a test.");
    }

    #[test]
    fn test_process_chapter_multiple_chunks() {
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence. Fifth sentence.";
        let chunks = process_chapter(5, text, 30);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.chapter_id == 5));
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.chunk_id, i);
        }
    }

    #[test]
    fn test_hard_split() {
        let text = "abcdefghij";
        let parts = hard_split(text, 3);
        assert_eq!(parts, vec!["abc", "def", "ghi", "j"]);
    }

    #[test]
    fn test_split_on_words() {
        let text = "one two three four five";
        let parts = split_on_words(text, 10);
        assert_eq!(parts, vec!["one two", "three four", "five"]);
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_long_word_no_delimiters() {
        // A very long word with no spaces or delimiters should hard-split
        let long_word = "a".repeat(500);
        let chunks = chunk_text(&long_word, 100, 150);

        // Should produce multiple chunks
        assert!(chunks.len() > 1, "Should split long word into multiple chunks");

        // All chunks should be within max_size
        for chunk in &chunks {
            assert!(
                chunk.len() <= 150,
                "Chunk exceeds max_size: {} chars",
                chunk.len()
            );
        }

        // Total content preserved
        let total_len: usize = chunks.iter().map(|c| c.len()).sum();
        assert_eq!(total_len, 500, "Content should be preserved");
    }

    #[test]
    fn test_consecutive_delimiters() {
        // Text with many consecutive delimiters
        let text = "a;;;b:::c,,,d - - - e";
        let chunks = chunk_text(text, 10, 20);

        // Content should be preserved (delimiters may be normalized)
        let rejoined = chunks.join(" ");
        assert!(rejoined.contains('a'), "Missing 'a'");
        assert!(rejoined.contains('b'), "Missing 'b'");
        assert!(rejoined.contains('c'), "Missing 'c'");
        assert!(rejoined.contains('d'), "Missing 'd'");
        assert!(rejoined.contains('e'), "Missing 'e'");
    }

    #[test]
    fn test_exact_boundary_conditions() {
        // Text exactly at target_size
        let text = "a".repeat(280); // Exactly DEFAULT_TARGET_SIZE
        let chunks = chunk_text(&text, 280, 350);
        assert_eq!(chunks.len(), 1, "Should fit in one chunk");
        assert_eq!(chunks[0].len(), 280);

        // Text exactly at max_size
        let text = "a".repeat(350);
        let chunks = chunk_text(&text, 280, 350);
        assert_eq!(chunks.len(), 1, "Should fit in one chunk at max_size");
    }

    #[test]
    fn test_realistic_book_content() {
        // Realistic content with dialog, abbreviations, em-dashes, ellipsis
        let text = r#""Good morning, Dr. Watson," said Holmes. "I see you've been to the U.S. recently—your tan gives you away." He paused… then continued, "The game is afoot!""#;

        let chunks = chunk_text(text, 100, 150);

        // Should produce reasonable chunks
        assert!(!chunks.is_empty(), "Should produce chunks");

        // All chunks within bounds
        for chunk in &chunks {
            assert!(
                chunk.len() <= 150,
                "Chunk exceeds max_size: {} chars",
                chunk.len()
            );
        }

        // Key content preserved (allowing for Unicode normalization)
        let rejoined = chunks.join(" ");
        assert!(rejoined.contains("Dr"), "Should preserve 'Dr'");
        assert!(rejoined.contains("Watson"), "Should preserve 'Watson'");
        assert!(rejoined.contains("Holmes"), "Should preserve 'Holmes'");
        assert!(rejoined.contains("U.S"), "Should preserve 'U.S'");
        assert!(rejoined.contains("afoot"), "Should preserve 'afoot'");

        // Problematic Unicode should be cleaned
        assert!(!rejoined.contains('\u{2014}'), "Em-dash should be normalized");
        assert!(!rejoined.contains('\u{2026}'), "Ellipsis should be normalized");
    }

    // ==================== Property Tests ====================

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn prop_no_data_loss(s in "\\PC{0,1000}") {
                let chunks = chunk_text(&s, 100, 150);

                // Count alphanumeric chars in input
                let input_alphanum: usize = s.chars().filter(|c| c.is_alphanumeric()).count();

                // Count alphanumeric chars in output
                let output_alphanum: usize = chunks
                    .iter()
                    .flat_map(|c| c.chars())
                    .filter(|c| c.is_alphanumeric())
                    .count();

                prop_assert_eq!(
                    input_alphanum,
                    output_alphanum,
                    "Alphanumeric content should be preserved"
                );
            }

            #[test]
            fn prop_chunks_within_bounds(s in "\\PC{1,500}") {
                let max_size = 150;
                let chunks = chunk_text(&s, 100, max_size);

                for chunk in &chunks {
                    prop_assert!(
                        chunk.len() <= max_size,
                        "Chunk {} chars exceeds max_size {}",
                        chunk.len(),
                        max_size
                    );
                }
            }

            #[test]
            fn prop_non_empty_input_produces_output(s in "[a-zA-Z]{1,100}") {
                // Input with at least one letter should produce non-empty output
                let chunks = chunk_text(&s, 50, 100);
                prop_assert!(!chunks.is_empty(), "Non-empty alphanumeric input should produce output");
            }
        }
    }
}
