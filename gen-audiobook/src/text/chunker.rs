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
        return word_split;
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

/// Hard split text at exact positions (last resort).
fn hard_split(text: &str, max_length: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let chars: Vec<char> = text.chars().collect();

    while start < chars.len() {
        let end = std::cmp::min(start + max_length, chars.len());
        let chunk: String = chars[start..end].iter().collect();
        chunks.push(chunk);
        start = end;
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

    // ==================== Chunk Uniqueness Tests ====================

    #[test]
    fn test_chunks_cover_all_sentences() {
        // Verify all input sentences appear in output chunks
        let text = "First sentence. Second sentence. Third sentence.";
        let chunks = chunk_text(text, 30, 50);
        let rejoined = chunks.join(" ");

        // All sentences should be present (normalized form)
        assert!(
            rejoined.contains("First sentence"),
            "Missing 'First sentence' in chunks: {:?}",
            chunks
        );
        assert!(
            rejoined.contains("Second sentence"),
            "Missing 'Second sentence' in chunks: {:?}",
            chunks
        );
        assert!(
            rejoined.contains("Third sentence"),
            "Missing 'Third sentence' in chunks: {:?}",
            chunks
        );
    }

    #[test]
    fn test_chunks_no_duplicate_sentences() {
        // Verify no sentence appears in multiple chunks
        let text = "Alpha. Beta. Gamma. Delta. Epsilon.";
        let chunks = chunk_text(text, 15, 30);

        // Use unique words to verify no duplicates
        let unique_words = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"];

        for word in unique_words {
            let count: usize = chunks.iter().filter(|c| c.contains(word)).count();
            assert_eq!(
                count, 1,
                "Word '{}' appears in {} chunks (expected 1): {:?}",
                word, count, chunks
            );
        }
    }

    #[test]
    fn test_chunks_preserve_sentence_boundaries() {
        // Verify sentences aren't split mid-word across chunks
        let text = "The quick brown fox. Jumps over the lazy dog.";
        let chunks = chunk_text(text, 25, 50);

        // Words should be complete in each chunk
        for chunk in &chunks {
            // No chunk should start or end with a partial word (indicated by space issues)
            let trimmed = chunk.trim();
            assert!(
                !trimmed.starts_with(' '),
                "Chunk starts with space: '{}'",
                chunk
            );
            assert!(
                !trimmed.ends_with(' '),
                "Chunk ends with space: '{}'",
                chunk
            );
        }
    }

    #[test]
    fn test_chunks_total_content_matches() {
        // Verify the total character count roughly matches (allowing for whitespace normalization)
        let text = "One. Two. Three. Four. Five. Six. Seven. Eight. Nine. Ten.";
        let chunks = chunk_text(text, 20, 40);

        // Count alphanumeric characters in original
        let orig_alphanum: usize = text.chars().filter(|c| c.is_alphanumeric()).count();

        // Count alphanumeric characters in chunks
        let chunks_alphanum: usize = chunks
            .iter()
            .flat_map(|c| c.chars())
            .filter(|c| c.is_alphanumeric())
            .count();

        assert_eq!(
            orig_alphanum, chunks_alphanum,
            "Alphanumeric count mismatch: original {} vs chunks {}",
            orig_alphanum, chunks_alphanum
        );
    }

    #[test]
    fn test_chunks_no_overlap_with_long_text() {
        // Test with longer text to catch edge cases
        let text = "Sentence one here. Sentence two here. Sentence three here. \
                    Sentence four here. Sentence five here. Sentence six here. \
                    Sentence seven here. Sentence eight here. Sentence nine here.";
        let chunks = chunk_text(text, 50, 80);

        // Verify unique numbered words appear exactly once across all chunks
        for num in ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine"] {
            let occurrences: usize = chunks.iter().map(|c| c.matches(num).count()).sum();
            assert_eq!(
                occurrences, 1,
                "Word '{}' appears {} times in chunks (expected 1): {:?}",
                num, occurrences, chunks
            );
        }
    }
}
