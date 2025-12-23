//! Text cleaning and sanitization for TTS processing.

/// Characters that can cause TTS issues and their replacements.
const PROBLEMATIC_CHARS: &[(char, &str)] = &[
    ('\u{2018}', "'"),  // Left single quote
    ('\u{2019}', "'"),  // Right single quote
    ('\u{201c}', "\""), // Left double quote
    ('\u{201d}', "\""), // Right double quote
    ('\u{2013}', "-"),  // En dash
    ('\u{2014}', "-"),  // Em dash
    ('\u{2026}', "..."), // Ellipsis
    ('\u{00a0}', " "),  // Non-breaking space
    ('\u{200b}', ""),   // Zero-width space
    ('\u{200c}', ""),   // Zero-width non-joiner
    ('\u{200d}', ""),   // Zero-width joiner
    ('\u{feff}', ""),   // BOM
    ('\u{2011}', "-"),  // Non-breaking hyphen
    ('\u{2012}', "-"),  // Figure dash
    ('\u{2015}', "-"),  // Horizontal bar
    ('\u{2032}', "'"),  // Prime (feet)
    ('\u{2033}', "\""), // Double prime (inches)
    ('\u{2039}', "<"),  // Single left-pointing angle quote
    ('\u{203a}', ">"),  // Single right-pointing angle quote
    ('\u{00ab}', "\""), // Left-pointing double angle quote
    ('\u{00bb}', "\""), // Right-pointing double angle quote
];

/// Clean text for TTS processing.
///
/// This function:
/// - Replaces problematic Unicode characters (smart quotes, dashes, etc.)
/// - Removes control characters (except newlines)
/// - Normalizes whitespace
/// - Fixes double periods that cause TTS noise
pub fn clean_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    // First pass: replace problematic characters
    for c in text.chars() {
        let replacement = PROBLEMATIC_CHARS
            .iter()
            .find(|(ch, _)| *ch == c)
            .map(|(_, r)| *r);

        if let Some(r) = replacement {
            result.push_str(r);
        } else if is_allowed_char(c) {
            result.push(c);
        }
        // Skip disallowed characters (control chars except newline/tab)
    }

    // Second pass: normalize whitespace and fix double periods
    let result = normalize_whitespace(&result);
    let result = fix_multiple_periods(&result);

    result
}

/// Check if a character is allowed in TTS text.
fn is_allowed_char(c: char) -> bool {
    // Allow printable characters, newlines, and tabs
    if c == '\n' || c == '\t' {
        return true;
    }

    // Reject control characters (U+0000 to U+001F, U+007F)
    if c.is_control() {
        return false;
    }

    true
}

/// Normalize whitespace in text.
fn normalize_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_was_space = false;
    let mut newline_count = 0;

    for c in text.chars() {
        if c == '\n' {
            newline_count += 1;
            prev_was_space = false;

            // Collapse more than 2 consecutive newlines
            if newline_count <= 2 {
                result.push('\n');
            }
        } else if c == ' ' || c == '\t' {
            newline_count = 0;
            // Collapse multiple spaces/tabs into one space
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            newline_count = 0;
            prev_was_space = false;
            result.push(c);
        }
    }

    result.trim().to_string()
}

/// Replace multiple consecutive periods with a single period.
/// This helps prevent TTS noise from "..." or ".."
fn fix_multiple_periods(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut period_count = 0;

    for c in text.chars() {
        if c == '.' {
            period_count += 1;
            // Only emit one period for consecutive periods
            if period_count == 1 {
                result.push('.');
            }
        } else {
            period_count = 0;
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_smart_quotes() {
        let text = "\u{201c}Hello,\u{201d} said John. \u{2018}It\u{2019}s nice.\u{2019}";
        let cleaned = clean_text(text);
        assert_eq!(cleaned, "\"Hello,\" said John. 'It's nice.'");
    }

    #[test]
    fn test_clean_dashes() {
        let text = "one–two—three";
        let cleaned = clean_text(text);
        assert_eq!(cleaned, "one-two-three");
    }

    #[test]
    fn test_clean_ellipsis() {
        let text = "Wait… what?";
        let cleaned = clean_text(text);
        assert_eq!(cleaned, "Wait. what?");
    }

    #[test]
    fn test_clean_multiple_periods() {
        let text = "What.. is... this....";
        let cleaned = clean_text(text);
        assert_eq!(cleaned, "What. is. this.");
    }

    #[test]
    fn test_clean_whitespace() {
        let text = "Hello   world\n\n\n\nNew paragraph";
        let cleaned = clean_text(text);
        assert_eq!(cleaned, "Hello world\n\nNew paragraph");
    }

    #[test]
    fn test_clean_control_chars() {
        let text = "Hello\x00World\x07Test";
        let cleaned = clean_text(text);
        assert_eq!(cleaned, "HelloWorldTest");
    }

    #[test]
    fn test_clean_zero_width_chars() {
        let text = "Hello\u{200b}World\u{feff}Test";
        let cleaned = clean_text(text);
        assert_eq!(cleaned, "HelloWorldTest");
    }

    #[test]
    fn test_preserves_newlines() {
        let text = "Line 1\nLine 2";
        let cleaned = clean_text(text);
        assert_eq!(cleaned, "Line 1\nLine 2");
    }
}
