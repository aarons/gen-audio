//! Anthropic API provider
//!
//! Direct HTTP implementation for the Anthropic Messages API.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{LlmError, Result};
use crate::provider::{LlmProvider, LlmRequest, LlmResponse, TokenUsage};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Provider for direct Anthropic API calls
pub struct AnthropicProvider {
    model: String,
    api_key: String,
    client: Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(model: &str, api_key: String) -> Result<Self> {
        let client = Client::new();

        Ok(Self {
            model: model.to_string(),
            api_key,
            client,
        })
    }
}

// Anthropic API request/response types

#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    usage: ResponseUsage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    text: String,
}

#[derive(Debug, Deserialize)]
struct ResponseUsage {
    input_tokens: u32,
    output_tokens: u32,
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
impl LlmProvider for AnthropicProvider {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let messages = vec![Message {
            role: "user".to_string(),
            content: request.prompt.clone(),
        }];

        let api_request = MessagesRequest {
            model: self.model.clone(),
            max_tokens: request.max_tokens.unwrap_or(4096),
            system: request.system_prompt.clone(),
            messages,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&api_request)
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

        let api_response: MessagesResponse =
            response.json().await.map_err(|e| LlmError::ApiError {
                message: format!("Failed to parse response: {}", e),
                status_code: None,
            })?;

        let content = api_response
            .content
            .first()
            .map(|c| c.text.clone())
            .unwrap_or_default();

        let usage = Some(TokenUsage {
            input_tokens: api_response.usage.input_tokens,
            output_tokens: api_response.usage.output_tokens,
        });

        Ok(LlmResponse {
            content,
            model: self.model.clone(),
            usage,
        })
    }

    fn name(&self) -> &'static str {
        "Anthropic API"
    }

    fn is_available(&self) -> Result<()> {
        // API key was provided in constructor
        Ok(())
    }
}
