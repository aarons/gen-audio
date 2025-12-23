//! Audio assembly module for creating M4B audiobooks with chapters.

pub mod assembler;
mod metadata;

pub use assembler::assemble_m4b;
