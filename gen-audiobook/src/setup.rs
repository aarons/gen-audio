//! Setup module - delegates to bootstrap module.
//!
//! This module is kept for backwards compatibility but all functionality
//! has been moved to the bootstrap module.

use crate::bootstrap;
use anyhow::Result;
use std::path::PathBuf;

/// Get the path to the Python executable in the venv.
pub fn get_python_path() -> Result<PathBuf> {
    bootstrap::python::get_venv_python()
}

/// Check if the virtual environment exists and has Python.
pub fn is_venv_ready() -> Result<bool> {
    bootstrap::python::is_venv_ready()
}

/// Check if Chatterbox is installed in the venv.
pub fn is_chatterbox_installed() -> Result<bool> {
    bootstrap::python::is_chatterbox_installed()
}

/// Get environment info for diagnostics.
pub fn get_env_info() -> Result<String> {
    bootstrap::python::get_env_info()
}

/// Check if setup is needed.
pub fn check_setup_needed() -> Result<bool> {
    Ok(bootstrap::check_status()? != bootstrap::BootstrapStatus::Ready)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_python_path() {
        let path = get_python_path().unwrap();
        assert!(path.ends_with("python"));
    }

    #[test]
    fn test_is_venv_ready() {
        // This test just checks the function doesn't panic
        let _ = is_venv_ready();
    }
}
