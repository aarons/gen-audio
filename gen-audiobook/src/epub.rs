// EPUB parsing and text extraction

use anyhow::Result;
use std::path::Path;

/// Represents a chapter extracted from an EPUB
#[derive(Debug, Clone)]
pub struct Chapter {
    /// Chapter title (if available)
    pub title: Option<String>,
    /// Plain text content
    pub content: String,
}

/// Parsed EPUB book
#[derive(Debug)]
pub struct Book {
    /// Book title
    pub title: String,
    /// Book author(s)
    pub author: Option<String>,
    /// Chapters in reading order
    pub chapters: Vec<Chapter>,
    /// Cover image data (if available)
    pub cover_image: Option<Vec<u8>>,
}

impl Book {
    /// Total word count across all chapters (approximate)
    pub fn total_words(&self) -> usize {
        self.chapters
            .iter()
            .map(|c| c.content.split_whitespace().count())
            .sum()
    }
}

/// Parse an EPUB file and extract text content
pub fn parse_epub(path: &Path) -> Result<Book> {
    let mut doc =
        epub::doc::EpubDoc::new(path).map_err(|e| anyhow::anyhow!("Failed to open EPUB: {}", e))?;

    // Get title - mdata returns Option<&MetadataItem>, need to get the value
    let title = doc
        .mdata("title")
        .map(|m| m.value.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let author = doc.mdata("creator").map(|m| m.value.clone());

    // Extract cover image
    let cover_image = extract_cover_image(&mut doc);

    let mut chapters = Vec::new();
    let spine = doc.spine.clone();

    for spine_item in spine.iter() {
        // Get the resource content using the idref
        if let Some((content_bytes, _mime)) = doc.get_resource(&spine_item.idref) {
            let html = String::from_utf8_lossy(&content_bytes).to_string();

            // Extract title from HTML if possible
            let chapter_title = extract_title_from_html(&html);

            // Convert HTML to plain text
            let plain_text = html_to_text(&html);

            // Skip empty chapters
            if plain_text.trim().is_empty() {
                continue;
            }

            chapters.push(Chapter {
                title: chapter_title,
                content: plain_text,
            });
        }
    }

    Ok(Book {
        title,
        author,
        chapters,
        cover_image,
    })
}

/// Extract cover image from EPUB document
fn extract_cover_image(doc: &mut epub::doc::EpubDoc<std::io::BufReader<std::fs::File>>) -> Option<Vec<u8>> {
    // Try the get_cover() method first (standard EPUB cover)
    if let Some((cover_bytes, _mime)) = doc.get_cover() {
        return Some(cover_bytes);
    }

    // Fallback: look for cover in metadata
    if let Some(cover_id) = doc.mdata("cover").map(|m| m.value.clone()) {
        if let Some((cover_bytes, _mime)) = doc.get_resource(&cover_id) {
            return Some(cover_bytes);
        }
    }

    None
}

/// Extract title from HTML content (looks for h1, h2, or title tags)
fn extract_title_from_html(html: &str) -> Option<String> {
    // Simple regex-free extraction - look for common title patterns
    let html_lower = html.to_lowercase();

    // Try h1
    if let Some(start) = html_lower.find("<h1") {
        if let Some(tag_end) = html_lower[start..].find('>') {
            let content_start = start + tag_end + 1;
            if let Some(end) = html_lower[content_start..].find("</h1>") {
                let title_html = &html[content_start..content_start + end];
                let title = strip_html_tags(title_html);
                if !title.trim().is_empty() {
                    return Some(title.trim().to_string());
                }
            }
        }
    }

    // Try h2
    if let Some(start) = html_lower.find("<h2") {
        if let Some(tag_end) = html_lower[start..].find('>') {
            let content_start = start + tag_end + 1;
            if let Some(end) = html_lower[content_start..].find("</h2>") {
                let title_html = &html[content_start..content_start + end];
                let title = strip_html_tags(title_html);
                if !title.trim().is_empty() {
                    return Some(title.trim().to_string());
                }
            }
        }
    }

    None
}

/// Strip HTML tags from a string
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    result
}

/// Convert HTML to plain text
fn html_to_text(html: &str) -> String {
    // Use html2text for conversion
    let text = html2text::from_read(html.as_bytes(), 1000);

    // Clean up the text
    clean_text(&text)
}

/// Clean up extracted text
fn clean_text(text: &str) -> String {
    let mut result = String::new();
    let mut prev_was_newline = false;

    for line in text.lines() {
        let trimmed = line.trim();

        // Skip empty lines but preserve paragraph breaks
        if trimmed.is_empty() {
            if !prev_was_newline && !result.is_empty() {
                result.push_str("\n\n");
                prev_was_newline = true;
            }
            continue;
        }

        prev_was_newline = false;

        // Add space if needed
        if !result.is_empty() && !result.ends_with('\n') {
            result.push(' ');
        }

        result.push_str(trimmed);
    }

    // Decode common HTML entities
    result
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "...")
        .replace("&rsquo;", "'")
        .replace("&lsquo;", "'")
        .replace("&rdquo;", "\"")
        .replace("&ldquo;", "\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
        assert_eq!(
            strip_html_tags("<h1>Title</h1><p>Content</p>"),
            "TitleContent"
        );
        assert_eq!(strip_html_tags("<a href=\"test\">Link</a>"), "Link");
    }

    #[test]
    fn test_extract_title_h1() {
        let html = "<html><body><h1>Chapter One</h1><p>Content here</p></body></html>";
        assert_eq!(
            extract_title_from_html(html),
            Some("Chapter One".to_string())
        );
    }

    #[test]
    fn test_extract_title_h2() {
        let html = "<html><body><h2>Section Title</h2><p>Content</p></body></html>";
        assert_eq!(
            extract_title_from_html(html),
            Some("Section Title".to_string())
        );
    }

    #[test]
    fn test_clean_text() {
        let text = "Hello &amp; goodbye &mdash; see you!";
        let cleaned = clean_text(text);
        assert!(cleaned.contains("&"));
        assert!(cleaned.contains("—"));
    }
}
