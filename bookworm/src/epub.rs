// EPUB metadata extraction

use anyhow::Result;
use std::path::Path;

/// Metadata extracted from an EPUB file
pub struct EpubMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub series: Option<String>,
    pub series_index: Option<String>,
}

/// Extract metadata from an EPUB file
pub fn extract_metadata(path: &Path) -> Result<EpubMetadata> {
    let doc = epub::doc::EpubDoc::new(path)
        .map_err(|e| anyhow::anyhow!("Failed to open EPUB: {}", e))?;

    Ok(EpubMetadata {
        title: doc.mdata("title").map(|m| m.value.clone()),
        author: doc.mdata("creator").map(|m| m.value.clone()),
        // Calibre stores series info in these fields
        series: doc.mdata("calibre:series").map(|m| m.value.clone()),
        series_index: doc.mdata("calibre:series_index").map(|m| m.value.clone()),
    })
}

impl EpubMetadata {
    /// Convert metadata to a string for LLM context
    pub fn to_context_string(&self) -> Option<String> {
        let parts: Vec<String> = [
            self.title.as_ref().map(|t| format!("Title: {}", t)),
            self.author.as_ref().map(|a| format!("Author: {}", a)),
            self.series.as_ref().map(|s| format!("Series: {}", s)),
            self.series_index
                .as_ref()
                .map(|i| format!("Series Index: {}", i)),
        ]
        .into_iter()
        .flatten()
        .collect();

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
    }
}
