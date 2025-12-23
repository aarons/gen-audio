//! Platform detection for bootstrap downloads.

use thiserror::Error;

/// Errors related to platform detection.
#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("Unsupported operating system: {0}")]
    UnsupportedOs(String),

    #[error("Unsupported architecture: {0}")]
    UnsupportedArch(String),
}

/// Supported operating systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Linux,
}

impl Os {
    /// Get the OS string for display.
    pub fn as_str(&self) -> &'static str {
        match self {
            Os::MacOs => "macOS",
            Os::Linux => "Linux",
        }
    }
}

/// Supported CPU architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

impl Arch {
    /// Get the architecture string for display.
    pub fn as_str(&self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        }
    }
}

/// Platform target for downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    /// Detect the current platform.
    pub fn detect() -> Result<Self, PlatformError> {
        let os = if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else {
            return Err(PlatformError::UnsupportedOs(
                std::env::consts::OS.to_string(),
            ));
        };

        let arch = if cfg!(target_arch = "x86_64") {
            Arch::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else {
            return Err(PlatformError::UnsupportedArch(
                std::env::consts::ARCH.to_string(),
            ));
        };

        Ok(Platform { os, arch })
    }

    /// Get the python-build-standalone platform string.
    ///
    /// Examples: "aarch64-apple-darwin", "x86_64-unknown-linux-gnu"
    pub fn python_platform_string(&self) -> &'static str {
        match (self.os, self.arch) {
            (Os::MacOs, Arch::Aarch64) => "aarch64-apple-darwin",
            (Os::MacOs, Arch::X86_64) => "x86_64-apple-darwin",
            (Os::Linux, Arch::X86_64) => "x86_64-unknown-linux-gnu",
            (Os::Linux, Arch::Aarch64) => "aarch64-unknown-linux-gnu",
        }
    }

    /// Get a string representation for version tracking.
    pub fn to_version_string(&self) -> String {
        format!("{}-{}", self.os.as_str(), self.arch.as_str())
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.os.as_str(), self.arch.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect().unwrap();
        // Should not panic and should return valid values
        let _ = platform.python_platform_string();
        let _ = platform.to_version_string();
    }

    #[test]
    fn test_python_platform_strings() {
        let macos_arm = Platform {
            os: Os::MacOs,
            arch: Arch::Aarch64,
        };
        assert_eq!(macos_arm.python_platform_string(), "aarch64-apple-darwin");

        let linux_x64 = Platform {
            os: Os::Linux,
            arch: Arch::X86_64,
        };
        assert_eq!(
            linux_x64.python_platform_string(),
            "x86_64-unknown-linux-gnu"
        );
    }
}
