//! OpenAI-compatible API provider
//!
//! Used for providers that implement the OpenAI chat completions API:
//! - OpenRouter
//! - Cerebras
//! - And others

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{LlmError, Result};
use crate::provider::{LlmProvider, LlmRequest, LlmResponse, TokenUsage};

/// Provider for OpenAI-compatible APIs
pub struct OpenAICompatibleProvider {
    model: String,
    base_url: String,
    api_key: String,
    name: &'static str,
    client: Client,
}

impl OpenAICompatibleProvider {
    /// Create a new OpenAI-compatible provider
    pub fn new(model: &str, base_url: &str, api_key: String, name: &'static str) -> Result<Self> {
        let client = Client::new();

        Ok(Self {
            model: model.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            name,
            client,
        })
    }

    /// Create an OpenRouter provider
    pub fn openrouter(model: &str, api_key: String) -> Result<Self> {
        Self::new(model, "https://openrouter.ai/api/v1", api_key, "OpenRouter")
    }

    /// Create a Cerebras provider
    pub fn cerebras(model: &str, api_key: String) -> Result<Self> {
        Self::new(model, "https://api.cerebras.ai/v1", api_key, "Cerebras")
    }
}

// OpenAI API request/response types

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

#[async_trait]
impl LlmProvider for OpenAICompatibleProvider {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let mut messages = Vec::new();

        if let Some(system) = &request.system_prompt {
            messages.push(Message {
                role: "system".to_string(),
                content: system.clone(),
            });
        }

        messages.push(Message {
            role: "user".to_string(),
            content: request.prompt.clone(),
        });

        let chat_request = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
        };

        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&chat_request)
            .send()
            .await
            .map_err(|e| LlmError::ApiError {
                message: format!("Request failed: {}", e),
                status_code: None,
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            let message =
                if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&error_text) {
                    error_response.error.message
                } else {
                    error_text
                };

            // Handle 503 (server overloaded) separately for retry logic
            if status.as_u16() == 503 {
                return Err(LlmError::ServerOverloaded { message });
            }

            return Err(LlmError::ApiError {
                message,
                status_code: Some(status.as_u16()),
            });
        }

        let chat_response: ChatCompletionResponse =
            response.json().await.map_err(|e| LlmError::ApiError {
                message: format!("Failed to parse response: {}", e),
                status_code: None,
            })?;

        let content = chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let usage = chat_response.usage.map(|u| TokenUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
        });

        Ok(LlmResponse {
            content,
            model: self.model.clone(),
            usage,
        })
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn is_available(&self) -> Result<()> {
        // API key was provided in constructor
        Ok(())
    }
}
