//! Sentence splitting using SEAMS (narrative-aware) or regex fallback.

use std::process::{Command, Stdio};

/// Split text into sentences.
///
/// Uses SEAMS for narrative-aware splitting if available,
/// falls back to regex-based splitting otherwise.
pub fn split_into_sentences(text: &str) -> Vec<String> {
    // Try SEAMS first (better for dialog-heavy text)
    if let Some(sentences) = split_sentences_seams(text) {
        return sentences;
    }

    // Fall back to regex
    split_sentences_regex(text)
}

/// Split text into sentences using SEAMS (narrative-aware).
///
/// SEAMS preserves dialog structure and attribution across paragraph breaks,
/// making it ideal for fiction/audiobook text.
///
/// Returns `None` if SEAMS is not available or fails.
fn split_sentences_seams(text: &str) -> Option<Vec<String>> {
    // Check if seams is available
    if !is_seams_available() {
        return None;
    }

    let result = Command::new("seams")
        .arg("--debug-stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let mut child = match result {
        Ok(child) => child,
        Err(_) => return None,
    };

    // Write input to stdin
    if let Some(ref mut stdin) = child.stdin {
        use std::io::Write;
        if stdin.write_all(text.as_bytes()).is_err() {
            return None;
        }
    }
    // Drop stdin to signal EOF
    child.stdin.take();

    // Wait for output
    let output = match child.wait_with_output() {
        Ok(output) if output.status.success() => output,
        _ => return None,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse SEAMS output: index<TAB>sentence<TAB>(coords)
    let sentences: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            if line.is_empty() {
                return None;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let sentence = parts[1].trim();
                if !sentence.is_empty() {
                    return Some(sentence.to_string());
                }
            }
            None
        })
        .collect();

    if sentences.is_empty() {
        None
    } else {
        Some(sentences)
    }
}

/// Check if the seams command is available.
fn is_seams_available() -> bool {
    Command::new("seams")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Split text into sentences using regex (fallback method).
///
/// This is less accurate than SEAMS but works without external dependencies.
fn split_sentences_regex(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    // Simple state machine for sentence boundary detection
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        current.push(c);

        // Check for sentence-ending punctuation
        if c == '.' || c == '!' || c == '?' {
            // Look ahead to see if this is actually end of sentence
            let is_end = is_sentence_end(&chars, i);

            if is_end {
                // Trim and add the sentence
                let sentence = current.trim().to_string();
                if !sentence.is_empty() {
                    sentences.push(sentence);
                }
                current = String::new();

                // Skip whitespace after sentence
                i += 1;
                while i < len && chars[i].is_whitespace() {
                    i += 1;
                }
                continue;
            }
        }

        i += 1;
    }

    // Don't forget the last sentence
    let sentence = current.trim().to_string();
    if !sentence.is_empty() {
        sentences.push(sentence);
    }

    sentences
}

/// Check if position `i` is likely the end of a sentence.
fn is_sentence_end(chars: &[char], i: usize) -> bool {
    // Must be at sentence-ending punctuation
    let c = chars[i];
    if c != '.' && c != '!' && c != '?' {
        return false;
    }

    let len = chars.len();

    // Check what comes after
    let mut j = i + 1;

    // Skip any closing quotes or parentheses
    while j < len && (chars[j] == '"' || chars[j] == '\'' || chars[j] == ')' || chars[j] == ']') {
        j += 1;
    }

    // If at end of text, it's a sentence end
    if j >= len {
        return true;
    }

    // Must be followed by whitespace
    if !chars[j].is_whitespace() {
        return false;
    }

    // Skip whitespace
    while j < len && chars[j].is_whitespace() {
        j += 1;
    }

    // If at end of text, it's a sentence end
    if j >= len {
        return true;
    }

    // Next non-whitespace character should be uppercase, quote, or number
    let next = chars[j];
    if next.is_uppercase() || next == '"' || next == '\'' || next == '(' || next == '[' {
        // Check for common abbreviations that look like sentence ends
        if c == '.' && is_likely_abbreviation(chars, i) {
            return false;
        }
        return true;
    }

    false
}

/// Check if the period at position `i` is likely part of an abbreviation.
fn is_likely_abbreviation(chars: &[char], i: usize) -> bool {
    // Look backwards to find the word before the period
    let mut j = i;
    while j > 0 && chars[j - 1].is_alphabetic() {
        j -= 1;
    }

    if j == i {
        return false; // No word before period
    }

    let word: String = chars[j..i].iter().collect();
    let word_lower = word.to_lowercase();

    // Common abbreviations
    const ABBREVIATIONS: &[&str] = &[
        "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "inc", "ltd", "co", "corp",
        "jan", "feb", "mar", "apr", "jun", "jul", "aug", "sep", "oct", "nov", "dec", "mon", "tue",
        "wed", "thu", "fri", "sat", "sun", "ave", "blvd", "rd", "dept", "govt", "approx", "est",
        "no", "vol", "rev", "ed", "gen", "col", "lt", "capt", "sgt", "pvt", "fig", "pp", "cf",
        "ie", "eg", "al", "ph",
    ];

    ABBREVIATIONS.contains(&word_lower.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_basic_sentences() {
        let text = "Hello world. How are you? I'm fine!";
        let sentences = split_sentences_regex(text);
        assert_eq!(sentences.len(), 3);
        assert_eq!(sentences[0], "Hello world.");
        assert_eq!(sentences[1], "How are you?");
        assert_eq!(sentences[2], "I'm fine!");
    }

    #[test]
    fn test_split_with_quotes() {
        let text = "\"Hello,\" she said. \"How are you?\"";
        let sentences = split_sentences_regex(text);
        // Regex fallback may produce different splits than SEAMS for dialog
        assert!(sentences.len() >= 1);
        // Ensure all content is preserved
        let rejoined: String = sentences.join(" ");
        assert!(rejoined.contains("Hello"));
        assert!(rejoined.contains("she said"));
        assert!(rejoined.contains("How are you"));
    }

    #[test]
    fn test_split_abbreviations() {
        let text = "Dr. Smith went to see Mr. Jones. They talked.";
        let sentences = split_sentences_regex(text);
        assert_eq!(sentences.len(), 2);
        assert_eq!(sentences[0], "Dr. Smith went to see Mr. Jones.");
        assert_eq!(sentences[1], "They talked.");
    }

    #[test]
    fn test_split_no_ending_punctuation() {
        let text = "This sentence has no ending punctuation";
        let sentences = split_sentences_regex(text);
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0], "This sentence has no ending punctuation");
    }

    #[test]
    fn test_split_multiple_spaces() {
        let text = "First sentence.   Second sentence.";
        let sentences = split_sentences_regex(text);
        assert_eq!(sentences.len(), 2);
    }

    #[test]
    fn test_seams_available() {
        // This test just checks the function doesn't panic
        let _ = is_seams_available();
    }

    #[test]
    fn test_split_into_sentences() {
        // This should work regardless of whether SEAMS is installed
        let text = "Hello. World.";
        let sentences = split_into_sentences(text);
        assert_eq!(sentences.len(), 2);
    }
}
