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
    fn test_seams_detector_available() {
        let detector = get_detector();
        // Just verify we can call a method on it
        let _ = detector.detect_sentences_borrowed("Test.");
    }

    #[test]
    fn test_split_into_sentences() {
        let text = "Hello. World.";
        let sentences = split_into_sentences(text);
        assert_eq!(sentences.len(), 2);
    }

    #[test]
    fn test_seams_library_basic() {
        let text = "First sentence. Second sentence.";
        let sentences = split_into_sentences(text);
        assert_eq!(sentences.len(), 2);
        assert!(sentences[0].contains("First"));
        assert!(sentences[1].contains("Second"));
    }
}
