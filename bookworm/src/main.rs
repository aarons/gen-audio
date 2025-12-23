mod epub;
mod llm;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use llm::LlmClient;
use llm_client::{Config, ModelPreset};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SYSTEM_PROMPT: &str = r#"You are a filename cleaner for ebook files. Given epub file information, extract and format as:

For series books: Series Name Series Number - Book Title - Author Name
For standalone books: Book Title - Author Name

You will receive:
- The current filename
- EPUB metadata (if available): title, author, series info

Rules:
- Prefer metadata over filename when available
- Use proper title case
- Author name: "First Last" format (e.g., "Jim Butcher" not "Butcher, Jim")
- For multiple authors, use only the primary author
- Remove publisher info, ISBNs, hashes, "Anna's Archive", years, etc.
- If series info isn't clear from either source, omit it
- Keep titles concise

Return ONLY the cleaned filename, nothing else. No quotes, no explanation, no file extension."#;

#[derive(Parser, Debug)]
#[command(
    name = "bookworm",
    about = "Clean and standardize epub filenames using AI",
    long_about = "Iterates through epub files and renames them to a clean, standardized format using an LLM"
)]
#[command(version)]
struct Args {
    /// Directory to search for epub files (defaults to current directory)
    #[arg(long)]
    dir: Option<PathBuf>,

    /// Search subdirectories recursively
    #[arg(short, long)]
    recursive: bool,

    /// Enable debug mode for verbose output
    #[arg(short, long, default_value_t = false)]
    debug: bool,

    /// Model preset to use (overrides default from config)
    #[arg(short, long)]
    model: Option<String>,

    /// Configuration subcommand
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Set the default model preset
    SetDefault {
        /// Name of the preset to use as default
        preset: String,
    },
    /// List available presets
    List,
    /// Show current configuration
    Show,
    /// Add a new preset
    AddPreset {
        /// Preset name
        name: String,
        /// Provider (claude-cli, anthropic, openrouter, cerebras)
        #[arg(short, long)]
        provider: String,
        /// Model identifier
        #[arg(short = 'M', long)]
        model: String,
    },
}

/// Handle config subcommands
fn handle_config_command(action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::SetDefault { preset } => {
            let mut config = Config::load()?;
            // Verify preset exists
            config.get_preset(preset)?;
            config
                .defaults
                .insert("bookworm".to_string(), preset.clone());
            config.save()?;
            println!("Default preset for bookworm set to: {}", preset);
        }
        ConfigAction::List => {
            let config = Config::load()?;
            let current_default = config.get_default_for_program("bookworm");
            println!("Available presets:");
            for (name, preset) in &config.presets {
                let default_marker = if name == current_default {
                    " (default)"
                } else {
                    ""
                };
                println!(
                    "  {} - {} / {}{}",
                    name, preset.provider, preset.model, default_marker
                );
            }
        }
        ConfigAction::Show => {
            let config = Config::load()?;
            let path = Config::config_path()?;
            println!("Config file: {}", path.display());
            println!();
            println!("{:#?}", config);
        }
        ConfigAction::AddPreset {
            name,
            provider,
            model,
        } => {
            let mut config = Config::load()?;
            config.presets.insert(
                name.clone(),
                ModelPreset {
                    provider: provider.clone(),
                    model: model.clone(),
                },
            );
            config.save()?;
            println!("Added preset: {}", name);
        }
    }
    Ok(())
}

/// Find all epub files in the given directory
fn find_epub_files(dir: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if recursive {
        for entry in WalkDir::new(dir).follow_links(true) {
            let entry = entry.context("Failed to read directory entry")?;
            if entry.file_type().is_file() && is_epub(entry.path()) {
                files.push(entry.path().to_path_buf());
            }
        }
    } else {
        for entry in std::fs::read_dir(dir).context("Failed to read directory")? {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();
            if path.is_file() && is_epub(&path) {
                files.push(path);
            }
        }
    }

    Ok(files)
}

/// Check if a path is an epub file (case-insensitive)
fn is_epub(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("epub"))
        .unwrap_or(false)
}

/// Generate a unique path by adding numeric suffix if needed
fn get_unique_path(target: &Path) -> PathBuf {
    if !target.exists() {
        return target.to_path_buf();
    }

    let stem = target.file_stem().and_then(OsStr::to_str).unwrap_or("");
    let ext = target.extension().and_then(OsStr::to_str).unwrap_or("epub");
    let parent = target.parent().unwrap_or(Path::new("."));

    for i in 1u32.. {
        let new_name = format!("{} ({}).{}", stem, i, ext);
        let candidate = parent.join(new_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    // Fallback (should never reach here)
    target.to_path_buf()
}

/// Clean the filename by removing invalid characters
fn sanitize_filename(name: &str) -> String {
    // Remove or replace characters that are invalid in filenames
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Remove punctuation from filename unless it appears in source materials
fn cleanup_punctuation(
    llm_output: &str,
    original_filename: &str,
    metadata: Option<&epub::EpubMetadata>,
) -> String {
    use std::collections::HashSet;

    // Build whitelist of allowed punctuation from source materials
    let mut allowed_punctuation: HashSet<char> = HashSet::new();

    // Always allow hyphen (used as format separator)
    allowed_punctuation.insert('-');

    // Collect punctuation from original filename
    for c in original_filename.chars() {
        if c.is_ascii_punctuation() {
            allowed_punctuation.insert(c);
        }
    }

    // Collect punctuation from metadata fields
    if let Some(meta) = metadata {
        for field in [&meta.title, &meta.author, &meta.series, &meta.series_index] {
            if let Some(value) = field {
                for c in value.chars() {
                    if c.is_ascii_punctuation() {
                        allowed_punctuation.insert(c);
                    }
                }
            }
        }
    }

    // Filter output: keep char if it's not punctuation OR if it's allowed punctuation
    llm_output
        .chars()
        .filter(|c| !c.is_ascii_punctuation() || allowed_punctuation.contains(c))
        .collect()
}

/// Process a single epub file
async fn process_file(
    llm: &LlmClient,
    file_path: &Path,
    debug: bool,
) -> Result<Option<(PathBuf, PathBuf)>> {
    let original_name = file_path
        .file_stem()
        .and_then(OsStr::to_str)
        .context("Invalid filename")?;

    if debug {
        eprintln!("Processing: {}", original_name);
    }

    // Build context for LLM
    let mut prompt = format!("Filename: {}", original_name);

    // Try to extract metadata from EPUB
    let metadata = match epub::extract_metadata(file_path) {
        Ok(metadata) => {
            if let Some(context) = metadata.to_context_string() {
                prompt.push_str("\n\nEPUB Metadata:\n");
                prompt.push_str(&context);
            }
            Some(metadata)
        }
        Err(e) if debug => {
            eprintln!("  Warning: Could not read EPUB metadata: {}", e);
            None
        }
        Err(_) => None, // Silently continue with filename only
    };

    if debug {
        eprintln!("  Prompt:\n{}", prompt);
    }

    // Ask LLM for cleaned name
    let cleaned_name = llm.complete(&prompt, SYSTEM_PROMPT).await?;
    let cleaned_name = cleanup_punctuation(cleaned_name.trim(), original_name, metadata.as_ref());
    let cleaned_name = sanitize_filename(&cleaned_name);

    if cleaned_name.is_empty() {
        anyhow::bail!("LLM returned empty filename");
    }

    // Check if name is already clean (same as original)
    if cleaned_name == original_name {
        if debug {
            eprintln!("  Skipping (already clean)");
        }
        return Ok(None);
    }

    // Build new path in same directory
    let parent = file_path.parent().unwrap_or(Path::new("."));
    let new_filename = format!("{}.epub", cleaned_name);
    let new_path = parent.join(&new_filename);

    // Handle conflicts
    let final_path = get_unique_path(&new_path);

    // Rename the file
    std::fs::rename(file_path, &final_path).context("Failed to rename file")?;

    Ok(Some((file_path.to_path_buf(), final_path)))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle config subcommands first (before LLM initialization)
    if let Some(Commands::Config { action }) = &args.command {
        return handle_config_command(action);
    }

    // Determine target directory
    let dir = args.dir.unwrap_or_else(|| PathBuf::from("."));
    let dir = dir
        .canonicalize()
        .context(format!("Invalid directory: {}", dir.display()))?;

    // Find epub files
    let files = find_epub_files(&dir, args.recursive)?;

    if files.is_empty() {
        println!("No epub files found in {}", dir.display());
        return Ok(());
    }

    println!("Processing {} epub file(s)...\n", files.len());

    // Initialize LLM client
    let llm = LlmClient::new(args.model.as_deref(), args.debug)?;

    let mut renamed_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;

    for file_path in &files {
        let original_name = file_path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("unknown");

        match process_file(&llm, file_path, args.debug).await {
            Ok(Some((_, new_path))) => {
                let new_name = new_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("unknown");
                println!("\"{}\"", original_name);
                println!("  -> \"{}\"\n", new_name);
                renamed_count += 1;
            }
            Ok(None) => {
                skipped_count += 1;
            }
            Err(e) => {
                eprintln!("Error processing \"{}\": {:#}\n", original_name, e);
                error_count += 1;
            }
        }
    }

    // Summary
    println!("---");
    println!(
        "Renamed: {}, Skipped: {}, Errors: {}",
        renamed_count, skipped_count, error_count
    );

    Ok(())
}
