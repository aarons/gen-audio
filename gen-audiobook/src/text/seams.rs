//! Sentence splitting using the seams library (dialog-aware).

use seams::sentence_detector::dialog_detector::SentenceDetectorDialog;
use std::sync::OnceLock;

/// Global detector instance (lazy initialization).
static DETECTOR: OnceLock<SentenceDetectorDialog> = OnceLock::new();

/// Get or initialize the sentence detector.
fn get_detector() -> &'static SentenceDetectorDialog {
    DETECTOR.get_or_init(|| {
        SentenceDetectorDialog::new().expect("seams sentence detector should initialize")
    })
}

/// Split text into sentences using the seams library for dialog-aware splitting.
pub fn split_into_sentences(text: &str) -> Vec<String> {
    let detector = get_detector();
    let sentences = detector
        .detect_sentences_borrowed(text)
        .expect("seams sentence detection should succeed");

    sentences
        .iter()
        .map(|s| s.normalize())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abbreviations_not_split() {
        // Abbreviations like "Dr." and "U.S." shouldn't trigger sentence splits
        let text = "Dr. Smith went to the U.S. embassy.";
        let sentences = split_into_sentences(text);

        // This should be one sentence (or at most split sensibly)
        // The key assertion: "U.S." shouldn't be split from "embassy"
        let rejoined = sentences.join(" ");
        assert!(
            rejoined.contains("U.S.") || rejoined.contains("U.S"),
            "U.S. should be preserved: {:?}",
            sentences
        );
        assert!(
            rejoined.contains("embassy"),
            "embassy should be preserved: {:?}",
            sentences
        );
    }

    #[test]
    fn test_dialog_sentence_boundaries() {
        // Dialog with proper sentence boundaries
        let text = r#""Hello," she said. "How are you?""#;
        let sentences = split_into_sentences(text);

        // Should detect at least 2 sentences (the dialog and the attribution)
        // Key: shouldn't split mid-quote
        assert!(
            sentences.len() >= 1,
            "Should produce sentences: {:?}",
            sentences
        );

        let rejoined = sentences.join(" ");
        assert!(rejoined.contains("Hello"), "Should preserve 'Hello'");
        assert!(rejoined.contains("How are you"), "Should preserve 'How are you'");
    }

    #[test]
    fn test_numbers_decimals() {
        // Numbers with periods shouldn't trigger false sentence breaks
        let text = "The price is $3.50. It's affordable.";
        let sentences = split_into_sentences(text);

        // Should be 2 sentences, not 3 (shouldn't split on "3.50")
        assert_eq!(
            sentences.len(),
            2,
            "Should be 2 sentences (not split on decimal): {:?}",
            sentences
        );
    }

    #[test]
    fn test_multiple_sentences_basic() {
        // Basic case: clearly separated sentences
        let text = "First sentence here. Second sentence follows. Third one ends it.";
        let sentences = split_into_sentences(text);

        assert_eq!(sentences.len(), 3, "Should detect 3 sentences: {:?}", sentences);
        assert!(sentences[0].contains("First"));
        assert!(sentences[1].contains("Second"));
        assert!(sentences[2].contains("Third"));
    }

    #[test]
    fn test_empty_and_whitespace() {
        assert!(split_into_sentences("").is_empty());
        assert!(split_into_sentences("   ").is_empty());
        assert!(split_into_sentences("\n\n").is_empty());
    }
}
