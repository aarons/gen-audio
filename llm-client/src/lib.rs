//! Shared LLM client library for gen-audio workspace
//!
//! Provides a unified interface for multiple LLM providers:
//! - Claude CLI (subprocess)
//! - Anthropic API (direct)
//! - OpenRouter (multi-model access)
//! - Cerebras (fast Llama inference)

pub mod config;
pub mod error;
pub mod provider;
pub mod providers;

pub use config::{Config, ModelPreset, ProviderConfig};
pub use error::{LlmError, Result};
pub use provider::{LlmProvider, LlmRequest, LlmResponse, TokenUsage};
pub use providers::{MockProvider, ProviderKind, get_provider};
