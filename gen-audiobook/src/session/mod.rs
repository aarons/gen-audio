//! Session management module for audiobook generation with checkpoint/resume support.

mod persistence;
mod types;

pub use persistence::{
    cleanup_session, create_session, find_session_for_book, get_chapter_audio_files,
    get_next_chunk, get_progress, get_temp_dir, mark_chunk_complete, mark_chunk_error,
};
pub use types::Session;
