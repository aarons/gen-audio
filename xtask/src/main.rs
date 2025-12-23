//! Development task runner for gen-audiobook.
//!
//! This tool provisions Python for PyO3 and runs cargo commands with the
//! correct environment variables set.
//!
//! # Usage
//!
//! ```bash
//! # Run tests with provisioned Python
//! cargo xtask test
//!
//! # Build with provisioned Python
//! cargo xtask build
//!
//! # Just provision Python for development
//! cargo xtask setup
//!
//! # Run any cargo command with correct environment
//! cargo xtask cargo check
//! ```

mod provision;

use anyhow::{Context, Result};
use std::env;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        print_usage();
        return Ok(ExitCode::SUCCESS);
    }

    match args[0].as_str() {
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(ExitCode::SUCCESS)
        }
        "setup" => {
            provision::provision_python()?;
            eprintln!("\nPython is ready for development.");
            eprintln!("You can now run: cargo xtask test");
            Ok(ExitCode::SUCCESS)
        }
        "test" => {
            let python = provision::provision_python()?;
            run_cargo_with_python(&python, &["test", "-p", "gen-audiobook"])
        }
        "build" => {
            let python = provision::provision_python()?;
            let mut cargo_args = vec!["build", "-p", "gen-audiobook"];
            // Pass through additional args like --release
            for arg in &args[1..] {
                cargo_args.push(arg);
            }
            run_cargo_with_python(&python, &cargo_args)
        }
        "cargo" => {
            if args.len() < 2 {
                eprintln!("Usage: cargo xtask cargo <cargo-args...>");
                return Ok(ExitCode::FAILURE);
            }
            let python = provision::provision_python()?;
            let cargo_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
            run_cargo_with_python(&python, &cargo_args)
        }
        cmd => {
            eprintln!("Unknown command: {}", cmd);
            print_usage();
            Ok(ExitCode::FAILURE)
        }
    }
}

fn print_usage() {
    eprintln!(
        r#"xtask - Development tasks for gen-audiobook

USAGE:
    cargo xtask <COMMAND>

COMMANDS:
    setup     Provision Python for development
    test      Run gen-audiobook tests
    build     Build gen-audiobook (pass --release for release build)
    cargo     Run arbitrary cargo command with Python environment
    help      Show this help message

EXAMPLES:
    cargo xtask test              # Run tests
    cargo xtask build --release   # Build release binary
    cargo xtask cargo check       # Run cargo check
"#
    );
}

/// Run a cargo command with PYO3_PYTHON and library paths set.
fn run_cargo_with_python(python: &std::path::Path, args: &[&str]) -> Result<ExitCode> {
    // Get the library directory (python/lib contains libpython3.11.dylib)
    let lib_dir = python
        .parent() // bin
        .and_then(|p| p.parent()) // python
        .map(|p| p.join("lib"))
        .context("Failed to determine Python lib directory")?;

    eprintln!("Running: cargo {}", args.join(" "));
    eprintln!("With PYO3_PYTHON={}", python.display());
    eprintln!("With LIBRARY_PATH={}", lib_dir.display());
    eprintln!();

    // Build the library path, preserving any existing LIBRARY_PATH
    let library_path = if let Ok(existing) = env::var("LIBRARY_PATH") {
        format!("{}:{}", lib_dir.display(), existing)
    } else {
        lib_dir.display().to_string()
    };

    let status = Command::new("cargo")
        .args(args)
        .env("PYO3_PYTHON", python)
        .env("LIBRARY_PATH", &library_path)
        // Also set for runtime linking on macOS
        .env("DYLD_LIBRARY_PATH", &library_path)
        .status()
        .context("Failed to run cargo")?;

    if status.success() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(status.code().unwrap_or(1) as u8))
    }
}
