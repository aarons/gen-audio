use async_trait::async_trait;

use crate::error::Result;

/// Request to send to an LLM provider
#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// Response from an LLM provider
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<TokenUsage>,
}

/// Token usage information
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Trait for LLM providers
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Execute a completion request
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse>;

    /// Get the provider name for display
    fn name(&self) -> &'static str;

    /// Check if the provider is available (API key set, CLI installed, etc.)
    fn is_available(&self) -> Result<()>;
}
