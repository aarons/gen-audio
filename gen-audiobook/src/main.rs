//! gena - Convert EPUB files to audiobooks using Chatterbox TTS

mod audio;
mod bootstrap;
mod config;
mod coordinator;
mod epub;
mod session;
mod setup;
mod text;
mod tts;
mod worker;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::GenaConfig;
use indicatif::{ProgressBar, ProgressStyle};
use session::Session;
use std::path::PathBuf;
use text::TextChunk;
use tts::TtsOptions;

#[derive(Parser, Debug)]
#[command(name = "gena")]
#[command(about = "Convert EPUB files to audiobooks using Chatterbox TTS", long_about = None)]
#[command(version)]
struct Args {
    /// Path to the EPUB file
    epub_file: Option<PathBuf>,

    /// Output file path (default: <epub-name>.m4b)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Path to voice reference audio for voice cloning
    #[arg(long)]
    voice: Option<PathBuf>,

    /// Start fresh, ignore existing session
    #[arg(long)]
    no_resume: bool,

    /// Chapter range to process (e.g., "0-10")
    #[arg(long)]
    chapters: Option<String>,

    /// Device to use (mps, cuda, cpu). Auto-detects if not specified.
    #[arg(long)]
    device: Option<String>,

    /// Expressiveness/exaggeration (0.25-2.0, default 0.5)
    #[arg(long, default_value = "0.5")]
    exaggeration: f32,

    /// Pacing/CFG weight (0.0-1.0, default 0.5)
    #[arg(long, default_value = "0.5")]
    cfg: f32,

    /// Temperature for randomness (0.05-5.0, default 0.8)
    #[arg(long, default_value = "0.8")]
    temperature: f32,

    /// Enable debug output
    #[arg(short, long, default_value_t = false)]
    debug: bool,

    /// Use distributed processing with remote workers
    #[arg(long)]
    distributed: bool,

    /// Specific workers to use (comma-separated names)
    #[arg(long)]
    workers: Option<String>,

    /// Subcommands
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Set up Python environment for Chatterbox TTS
    Setup {
        /// Upgrade existing packages
        #[arg(long)]
        upgrade: bool,

        /// Remove bootstrap cache and re-download everything
        #[arg(long)]
        clean: bool,
    },
    /// Remove all gena dependencies from the system
    Uninstall {
        /// Also remove Chatterbox models from HuggingFace cache
        #[arg(long)]
        include_models: bool,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Show environment info for debugging
    Info,
    /// Worker mode for distributed processing (runs on remote GPU machines)
    Worker {
        #[command(subcommand)]
        action: worker::WorkerCommand,
    },
    /// Manage remote workers for distributed processing
    Workers {
        #[command(subcommand)]
        action: coordinator::WorkersCommand,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set default voice reference
    SetVoice {
        /// Path to voice reference audio
        path: PathBuf,
    },
    /// Set default exaggeration
    SetExaggeration {
        /// Value (0.25-2.0)
        value: f32,
    },
    /// Set default CFG/pacing
    SetCfg {
        /// Value (0.0-1.0)
        value: f32,
    },
    /// Set default temperature
    SetTemperature {
        /// Value (0.05-5.0)
        value: f32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle subcommands that don't need bootstrap
    match &args.command {
        Some(Commands::Setup { upgrade, clean }) => {
            return handle_setup_command(*upgrade, *clean).await;
        }
        Some(Commands::Uninstall { include_models }) => {
            return handle_uninstall_command(*include_models);
        }
        Some(Commands::Config { action }) => {
            return handle_config_command(action);
        }
        Some(Commands::Info) => {
            return show_info();
        }
        Some(Commands::Worker { action }) => {
            return worker::handle_worker_command(action).await;
        }
        Some(Commands::Workers { action }) => {
            return coordinator::handle_workers_command(action).await;
        }
        None => {}
    }

    // Auto-bootstrap if needed
    let paths = bootstrap::ensure_bootstrapped().await?;

    // Set Python path for PyO3
    unsafe {
        std::env::set_var("PYO3_PYTHON", &paths.python);
    }

    // Require EPUB file for conversion
    let epub_path = args
        .epub_file
        .clone()
        .ok_or_else(|| anyhow::anyhow!("EPUB file path is required. Run 'gena --help' for usage."))?;

    if !epub_path.exists() {
        anyhow::bail!("EPUB file not found: {}", epub_path.display());
    }

    // Load configuration
    let config = GenaConfig::load().context("Failed to load configuration")?;

    // Determine output path (M4B for audiobook with chapters)
    let output_path = args.output.clone().unwrap_or_else(|| {
        let stem = epub_path.file_stem().unwrap_or_default();
        epub_path.with_file_name(format!("{}.m4b", stem.to_string_lossy()))
    });

    // Build TTS options from args and config
    let voice_ref = args.voice.clone().or(config.voice_ref);
    let tts_options = TtsOptions::new()
        .with_exaggeration(args.exaggeration)
        .with_cfg(args.cfg)
        .with_temperature(args.temperature);
    let tts_options = if let Some(ref voice) = voice_ref {
        tts_options.with_voice_ref(voice.clone())
    } else {
        tts_options
    };

    if args.debug {
        eprintln!("EPUB: {}", epub_path.display());
        eprintln!("Output: {}", output_path.display());
        eprintln!("Voice ref: {:?}", voice_ref);
        eprintln!("Device: {:?}", args.device);
        eprintln!("Exaggeration: {}", args.exaggeration);
        eprintln!("CFG: {}", args.cfg);
        eprintln!("Temperature: {}", args.temperature);
    }

    // Parse EPUB
    eprintln!("Parsing EPUB: {}", epub_path.display());
    let book = epub::parse_epub(&epub_path).context("Failed to parse EPUB")?;

    eprintln!(
        "Book: \"{}\" by {}",
        book.title,
        book.author.as_deref().unwrap_or("Unknown")
    );
    eprintln!(
        "Chapters: {}, Words: ~{}",
        book.chapters.len(),
        book.total_words()
    );

    if book.chapters.is_empty() {
        anyhow::bail!("No chapters found in EPUB");
    }

    // Parse chapter range if specified
    let (start_chapter, end_chapter) = parse_chapter_range(&args.chapters, book.chapters.len())?;

    // Check for existing session
    let mut session = if !args.no_resume {
        session::find_session_for_book(&epub_path)?
    } else {
        None
    };

    // If resuming, show progress
    if let Some(ref s) = session {
        let (completed, total, pct) = session::get_progress(s);
        eprintln!(
            "Resuming session: {}/{} chunks ({:.1}% complete)",
            completed, total, pct
        );
    }

    // Create new session if needed
    let chunks: Vec<TextChunk>;
    if session.is_none() {
        // Process chapters into chunks
        eprintln!("Processing text into chunks...");
        chunks = process_book_chapters(&book, start_chapter, end_chapter);
        eprintln!("Total chunks: {}", chunks.len());

        // Create session
        session = Some(session::create_session(
            &epub_path,
            &book.title,
            book.author.as_deref().unwrap_or("Unknown"),
            &chunks,
        )?);
    } else {
        // For resume, we need to reconstruct chunks from book
        chunks = process_book_chapters(&book, start_chapter, end_chapter);
    }

    let mut session = session.unwrap();

    // Get temp directory for audio chunks
    let temp_dir = session::get_temp_dir(&session.session_id)?;

    // Check if we should use distributed processing
    let use_distributed = args.distributed || args.workers.is_some();

    if use_distributed {
        // Distributed processing with remote workers
        process_distributed(
            &mut session,
            &chunks,
            &args,
            voice_ref.as_ref(),
            &temp_dir,
        )
        .await?;
    } else {
        // Local processing
        process_local(
            &mut session,
            &chunks,
            &tts_options,
            args.device.as_deref(),
            voice_ref,
            &temp_dir,
        )
        .await?;
    }

    // Save cover image to temp file if available
    let cover_path = if let Some(ref cover_bytes) = book.cover_image {
        let temp_dir = session::get_temp_dir(&session.session_id)?;
        let cover_file = temp_dir.join(detect_cover_filename(cover_bytes));
        std::fs::write(&cover_file, cover_bytes)?;
        Some(cover_file)
    } else {
        None
    };

    // Assemble M4B with chapter markers
    eprintln!("\nAssembling audiobook...");
    assemble_audiobook(
        &session,
        &book,
        &output_path,
        start_chapter,
        end_chapter,
        cover_path.as_deref(),
    )?;

    // Get output file size
    let metadata = std::fs::metadata(&output_path)?;
    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

    eprintln!("Output: {} ({:.1} MB)", output_path.display(), size_mb);

    // Cleanup session on success
    session::cleanup_session(&session)?;

    Ok(())
}

/// Process chunks using local TTS backend.
async fn process_local(
    session: &mut Session,
    chunks: &[TextChunk],
    tts_options: &TtsOptions,
    device: Option<&str>,
    voice_ref: Option<PathBuf>,
    temp_dir: &PathBuf,
) -> Result<()> {
    // Initialize TTS backend
    eprintln!("Initializing Chatterbox TTS...");
    let backend = tts::create_backend(device, voice_ref)?;
    eprintln!("Using device: {}", backend.device());

    // Create progress bar
    let (completed, total, _) = session::get_progress(session);
    let pb = ProgressBar::new(total as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_position(completed as u64);

    // Generate audio for each chunk
    while let Some((chapter_id, chunk_id)) = session::get_next_chunk(session) {
        // Find the chunk text
        let chunk_text = chunks
            .iter()
            .find(|c| c.chapter_id == chapter_id && c.chunk_id == chunk_id)
            .map(|c| c.text.as_str())
            .unwrap_or("");

        if chunk_text.is_empty() {
            // Skip empty chunks
            session::mark_chunk_complete(
                session,
                chapter_id,
                chunk_id,
                &temp_dir.join("empty.wav"),
            )?;
            pb.inc(1);
            continue;
        }

        let audio_path = temp_dir.join(format!("ch{:03}_chunk{:04}.wav", chapter_id, chunk_id));

        pb.set_message(format!("Chapter {} chunk {}", chapter_id + 1, chunk_id + 1));

        match backend
            .synthesize_with_retry(chunk_text, &audio_path, tts_options, 3)
            .await
        {
            Ok(()) => {
                session::mark_chunk_complete(session, chapter_id, chunk_id, &audio_path)?;
            }
            Err(e) => {
                session::mark_chunk_error(session, chapter_id, chunk_id, &e.to_string())?;
                eprintln!("\nError generating chunk {}-{}: {}", chapter_id, chunk_id, e);
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message("Audio generation complete!");
    Ok(())
}

/// Process chunks using distributed workers.
async fn process_distributed(
    session: &mut Session,
    chunks: &[TextChunk],
    args: &Args,
    voice_ref: Option<&PathBuf>,
    temp_dir: &PathBuf,
) -> Result<()> {
    use coordinator::{
        create_jobs, JobScheduler, WorkerPool, WorkersConfig,
    };
    use worker::protocol::TtsJobOptions;

    eprintln!("Initializing distributed processing...");

    // Load worker configuration
    let workers_config = WorkersConfig::load()?;

    if workers_config.workers.is_empty() {
        anyhow::bail!(
            "No workers configured. Add workers with: gena workers add <name> <host> -u <user>"
        );
    }

    // Create worker pool
    let mut pool = if let Some(ref names) = args.workers {
        let worker_names: Vec<&str> = names.split(',').map(|s| s.trim()).collect();
        WorkerPool::with_workers(&workers_config, &worker_names)
    } else {
        WorkerPool::new(&workers_config)
    };

    if pool.is_empty() {
        anyhow::bail!("No matching workers found");
    }

    eprintln!("Connecting to {} worker(s)...", pool.len());

    // Connect to all workers
    let connection_results = pool.connect_all().await;
    for (name, result) in &connection_results {
        match result {
            Ok(()) => {
                if let Some(worker) = pool.get_worker(name) {
                    if let Some(ref status) = worker.status {
                        eprintln!("  {} ({}): ready", name, status.device);
                    }
                }
            }
            Err(e) => {
                eprintln!("  {}: FAILED - {}", name, e);
            }
        }
    }

    let ready_count = pool.ready_workers().len();
    if ready_count == 0 {
        anyhow::bail!("No workers are ready");
    }

    eprintln!("{} worker(s) ready", ready_count);

    // Upload voice reference if provided
    if let Some(voice_path) = voice_ref {
        let hash = coordinator::compute_file_hash(&voice_path.to_path_buf())?;
        eprintln!("Uploading voice reference ({})...", &hash[..8]);
        pool.ensure_voice_ref(voice_path, &hash).await?;
    }

    // Get pending chunks
    let pending_chunks: Vec<(usize, usize, String)> = chunks
        .iter()
        .filter(|c| {
            !session
                .chunks
                .iter()
                .any(|s| s.chapter_id == c.chapter_id && s.chunk_id == c.chunk_id && s.completed)
        })
        .filter(|c| !c.text.is_empty())
        .map(|c| (c.chapter_id, c.chunk_id, c.text.clone()))
        .collect();

    if pending_chunks.is_empty() {
        eprintln!("All chunks already processed!");
        return Ok(());
    }

    eprintln!("Processing {} chunks...", pending_chunks.len());

    // Create TTS job options
    let voice_hash = voice_ref.map(|p| coordinator::compute_file_hash(&p.to_path_buf()))
        .transpose()?;

    let job_options = TtsJobOptions {
        exaggeration: args.exaggeration,
        cfg: args.cfg,
        temperature: args.temperature,
        voice_ref_hash: voice_hash,
    };

    // Create jobs
    let jobs = create_jobs(&session.session_id, &pending_chunks, job_options);

    // Create scheduler
    let mut scheduler = JobScheduler::new(pool, temp_dir.clone());
    scheduler.enqueue(jobs);

    // Create progress bar
    let total = pending_chunks.len();
    let pb = ProgressBar::new(total as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Run scheduler
    let results = scheduler
        .run_to_completion(|progress| {
            pb.set_position(progress.completed as u64);
            if !progress.workers.is_empty() {
                let worker_info: Vec<String> = progress
                    .workers
                    .iter()
                    .map(|w| format!("{}:{}", w.name, w.completed))
                    .collect();
                pb.set_message(worker_info.join(" "));
            }
        })
        .await?;

    pb.finish_with_message("Distributed processing complete!");

    // Update session with results
    for result in &results {
        // Parse chapter and chunk from job_id
        if let (Some(chapter_id), Some(chunk_id)) = (
            parse_chapter_from_job_id(&result.job_id),
            parse_chunk_from_job_id(&result.job_id),
        ) {
            match result.status {
                worker::protocol::JobStatus::Completed => {
                    let audio_path = temp_dir.join(format!("{}.wav", result.job_id));
                    session::mark_chunk_complete(session, chapter_id, chunk_id, &audio_path)?;
                }
                _ => {
                    let error = result.error.as_deref().unwrap_or("Unknown error");
                    session::mark_chunk_error(session, chapter_id, chunk_id, error)?;
                }
            }
        }
    }

    // Report summary
    let successful = results
        .iter()
        .filter(|r| r.status == worker::protocol::JobStatus::Completed)
        .count();
    let failed = results.len() - successful;

    eprintln!("\nCompleted: {}, Failed: {}", successful, failed);

    Ok(())
}

/// Parse chapter number from job ID.
fn parse_chapter_from_job_id(job_id: &str) -> Option<usize> {
    let parts: Vec<&str> = job_id.split('_').collect();
    for part in parts {
        if part.starts_with("ch") {
            return part[2..].parse().ok();
        }
    }
    None
}

/// Parse chunk number from job ID.
fn parse_chunk_from_job_id(job_id: &str) -> Option<usize> {
    let parts: Vec<&str> = job_id.split('_').collect();
    for part in parts {
        if part.starts_with("ck") {
            return part[2..].parse().ok();
        }
    }
    None
}

/// Parse chapter range string like "0-10" or "5".
fn parse_chapter_range(range: &Option<String>, total: usize) -> Result<(usize, usize)> {
    match range {
        None => Ok((0, total)),
        Some(r) => {
            if r.contains('-') {
                let parts: Vec<&str> = r.split('-').collect();
                if parts.len() != 2 {
                    anyhow::bail!("Invalid chapter range format. Use 'start-end' (e.g., '0-10')");
                }
                let start: usize = parts[0].parse().context("Invalid start chapter")?;
                let end: usize = parts[1].parse().context("Invalid end chapter")?;
                Ok((start.min(total), (end + 1).min(total)))
            } else {
                let chapter: usize = r.parse().context("Invalid chapter number")?;
                Ok((chapter.min(total), (chapter + 1).min(total)))
            }
        }
    }
}

/// Process book chapters into text chunks.
fn process_book_chapters(
    book: &epub::Book,
    start_chapter: usize,
    end_chapter: usize,
) -> Vec<TextChunk> {
    let mut all_chunks = Vec::new();

    for (i, chapter) in book.chapters[start_chapter..end_chapter].iter().enumerate() {
        let chapter_id = start_chapter + i;

        // Prepend chapter title if available
        let text = if let Some(ref title) = chapter.title {
            format!("{}. {}", title, chapter.content)
        } else {
            chapter.content.clone()
        };

        let chunks = text::process_chapter(chapter_id, &text, text::chunker::DEFAULT_TARGET_SIZE);
        all_chunks.extend(chunks);
    }

    all_chunks
}

/// Assemble the final M4B audiobook.
fn assemble_audiobook(
    session: &Session,
    book: &epub::Book,
    output_path: &PathBuf,
    start_chapter: usize,
    end_chapter: usize,
    cover_image: Option<&std::path::Path>,
) -> Result<()> {
    // Collect all completed audio files
    let mut all_audio_files: Vec<PathBuf> = Vec::new();
    let mut chapter_boundaries: Vec<(String, usize)> = Vec::new();

    let mut current_chunk_index = 0;

    for chapter_id in start_chapter..end_chapter {
        // Record chapter boundary
        let chapter_title = book.chapters[chapter_id]
            .title
            .clone()
            .unwrap_or_else(|| format!("Chapter {}", chapter_id + 1));
        chapter_boundaries.push((chapter_title, current_chunk_index));

        // Get audio files for this chapter
        let chapter_files = session::get_chapter_audio_files(session, chapter_id);
        current_chunk_index += chapter_files.len();
        all_audio_files.extend(chapter_files);
    }

    if all_audio_files.is_empty() {
        anyhow::bail!("No audio files generated");
    }

    // Convert to references for the assembler
    let file_refs: Vec<&std::path::Path> = all_audio_files.iter().map(|p| p.as_path()).collect();

    // Assemble M4B
    audio::assemble_m4b(
        &file_refs,
        &chapter_boundaries,
        output_path,
        &book.title,
        book.author.as_deref().unwrap_or("Unknown"),
        cover_image,
    )?;

    Ok(())
}

/// Detect cover image format and return appropriate filename.
fn detect_cover_filename(data: &[u8]) -> &'static str {
    // Check magic bytes for common image formats
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "cover.jpg"
    } else if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "cover.png"
    } else if data.starts_with(b"GIF") {
        "cover.gif"
    } else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" {
        "cover.webp"
    } else {
        // Default to JPEG as it's most common
        "cover.jpg"
    }
}

fn handle_config_command(action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let config = GenaConfig::load()?;
            println!("Configuration file: {:?}", GenaConfig::config_path()?);
            println!();
            if let Some(voice) = &config.voice_ref {
                println!("voice_ref = \"{}\"", voice.display());
            } else {
                println!("voice_ref = (none)");
            }
            println!("exaggeration = {}", config.exaggeration);
            println!("cfg = {}", config.cfg);
            println!("temperature = {}", config.temperature);
            if let Some(device) = &config.device {
                println!("device = \"{}\"", device);
            } else {
                println!("device = (auto-detect)");
            }
        }
        ConfigAction::SetVoice { path } => {
            let mut config = GenaConfig::load()?;
            config.voice_ref = Some(path.clone());
            config.save()?;
            println!("Default voice reference set to: {}", path.display());
        }
        ConfigAction::SetExaggeration { value } => {
            let mut config = GenaConfig::load()?;
            config.exaggeration = value.clamp(0.25, 2.0);
            config.save()?;
            println!("Default exaggeration set to: {}", config.exaggeration);
        }
        ConfigAction::SetCfg { value } => {
            let mut config = GenaConfig::load()?;
            config.cfg = value.clamp(0.0, 1.0);
            config.save()?;
            println!("Default CFG set to: {}", config.cfg);
        }
        ConfigAction::SetTemperature { value } => {
            let mut config = GenaConfig::load()?;
            config.temperature = value.clamp(0.05, 5.0);
            config.save()?;
            println!("Default temperature set to: {}", config.temperature);
        }
    }
    Ok(())
}

fn show_info() -> Result<()> {
    println!("gena environment info:\n");
    println!("{}", bootstrap::get_info()?);
    Ok(())
}

async fn handle_setup_command(upgrade: bool, clean: bool) -> Result<()> {
    if clean {
        eprintln!("Removing bootstrap cache...");
        let stats = bootstrap::clean_all(false)?;
        if stats.gena_removed {
            eprintln!(
                "Removed {} of data.",
                bootstrap::download::format_bytes(stats.gena_size)
            );
        }
        eprintln!("Run 'gena <book.epub>' to re-bootstrap.\n");
        return Ok(());
    }

    // Run bootstrap (will prompt if first run)
    let _paths = bootstrap::ensure_bootstrapped().await?;

    if upgrade {
        eprintln!("\nUpgrading packages...");
        bootstrap::python::install_packages(|msg| {
            eprintln!("  {}", msg);
        })?;
        eprintln!("Packages upgraded successfully.");
    }

    Ok(())
}

fn handle_uninstall_command(include_models: bool) -> Result<()> {
    eprintln!("Removing gena dependencies...\n");

    let stats = bootstrap::clean_all(include_models)?;

    if stats.gena_removed {
        eprintln!(
            "Removed gena data directory ({}).",
            bootstrap::download::format_bytes(stats.gena_size)
        );
    } else {
        eprintln!("No gena data directory found.");
    }

    if include_models {
        if stats.models_removed {
            eprintln!(
                "Removed Chatterbox models from cache ({}).",
                bootstrap::download::format_bytes(stats.models_size)
            );
        } else {
            eprintln!("No Chatterbox models found in cache.");
        }
    }

    eprintln!(
        "\nTotal space reclaimed: {}",
        bootstrap::download::format_bytes(stats.total_size())
    );

    Ok(())
}
