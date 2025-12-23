use thiserror::Error;

#[derive(Error, Debug)]
pub enum LlmError {
    #[error(
        "API key not found for {provider}. Set {env_var} environment variable or add to config."
    )]
    MissingApiKey { provider: String, env_var: String },

    #[error("Provider not available: {0}")]
    ProviderUnavailable(String),

    #[error("Rate limit exceeded{}", .retry_after.map(|s| format!(". Retry after {} seconds", s)).unwrap_or_default())]
    RateLimited { retry_after: Option<u64> },

    #[error("Server overloaded (HTTP 503): {message}")]
    ServerOverloaded { message: String },

    #[error("API error{}: {message}", status_code.map(|c| format!(" (HTTP {})", c)).unwrap_or_default())]
    ApiError {
        message: String,
        status_code: Option<u16>,
    },

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Invalid model preset: {0}")]
    InvalidPreset(String),

    #[error("Claude CLI error: {0}")]
    ClaudeCliError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

pub type Result<T> = std::result::Result<T, LlmError>;
