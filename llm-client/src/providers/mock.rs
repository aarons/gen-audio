//! Mock LLM provider for testing
//!
//! Provides a configurable mock provider that can simulate various behaviors
//! like failures, retries, and successful responses.

use async_trait::async_trait;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::{LlmError, Result};
use crate::provider::{LlmProvider, LlmRequest, LlmResponse};

/// A mock provider for testing retry and fallback behavior
pub struct MockProvider {
    /// Number of times to fail before succeeding (0 = always succeed)
    fail_count: AtomicUsize,
    /// Current call count
    call_count: AtomicUsize,
    /// Error to return on failure (None = always succeed)
    fail_with: Mutex<Option<LlmError>>,
    /// Response content to return on success
    success_response: String,
    /// Provider name for display
    name: &'static str,
}

impl MockProvider {
    /// Create a provider that fails `n` times with the given error, then succeeds
    pub fn fails_then_succeeds(n: usize, error: LlmError, response: &str) -> Self {
        Self {
            fail_count: AtomicUsize::new(n),
            call_count: AtomicUsize::new(0),
            fail_with: Mutex::new(Some(error)),
            success_response: response.to_string(),
            name: "mock",
        }
    }

    /// Create a provider that always fails with the given error
    pub fn always_fails(error: LlmError) -> Self {
        Self {
            fail_count: AtomicUsize::new(usize::MAX),
            call_count: AtomicUsize::new(0),
            fail_with: Mutex::new(Some(error)),
            success_response: String::new(),
            name: "mock",
        }
    }

    /// Create a provider that always succeeds
    pub fn always_succeeds(response: &str) -> Self {
        Self {
            fail_count: AtomicUsize::new(0),
            call_count: AtomicUsize::new(0),
            fail_with: Mutex::new(None),
            success_response: response.to_string(),
            name: "mock",
        }
    }

    /// Get the number of times complete() was called
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    /// Set a custom provider name (useful for testing fallback scenarios)
    pub fn with_name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn complete(&self, _request: LlmRequest) -> Result<LlmResponse> {
        let call_num = self.call_count.fetch_add(1, Ordering::SeqCst);
        let fail_count = self.fail_count.load(Ordering::SeqCst);

        if call_num < fail_count {
            // Should fail on this call
            let error = self.fail_with.lock().unwrap();
            if let Some(err) = error.as_ref() {
                // Clone the error for returning
                return Err(clone_error(err));
            }
        }

        // Success
        Ok(LlmResponse {
            content: self.success_response.clone(),
            model: "mock-model".to_string(),
            usage: None,
        })
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn is_available(&self) -> Result<()> {
        Ok(())
    }
}

/// Clone an LlmError (needed because LlmError doesn't implement Clone)
fn clone_error(err: &LlmError) -> LlmError {
    match err {
        LlmError::ServerOverloaded { message } => LlmError::ServerOverloaded {
            message: message.clone(),
        },
        LlmError::MissingApiKey { provider, env_var } => LlmError::MissingApiKey {
            provider: provider.clone(),
            env_var: env_var.clone(),
        },
        LlmError::RateLimited { retry_after } => LlmError::RateLimited {
            retry_after: *retry_after,
        },
        LlmError::ApiError {
            message,
            status_code,
        } => LlmError::ApiError {
            message: message.clone(),
            status_code: *status_code,
        },
        LlmError::ProviderUnavailable(s) => LlmError::ProviderUnavailable(s.clone()),
        LlmError::ConfigError(s) => LlmError::ConfigError(s.clone()),
        LlmError::InvalidPreset(s) => LlmError::InvalidPreset(s.clone()),
        LlmError::ClaudeCliError(s) => LlmError::ClaudeCliError(s.clone()),
        // For Io and Toml errors, we create a generic error since they can't be cloned
        LlmError::Io(_) => LlmError::ConfigError("IO error (mock)".to_string()),
        LlmError::TomlParse(_) => LlmError::ConfigError("TOML parse error (mock)".to_string()),
        LlmError::TomlSerialize(_) => {
            LlmError::ConfigError("TOML serialize error (mock)".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_always_succeeds() {
        let provider = MockProvider::always_succeeds("success");
        let request = LlmRequest {
            prompt: "test".to_string(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };

        let result = provider.complete(request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "success");
        assert_eq!(provider.call_count(), 1);
    }

    #[tokio::test]
    async fn test_always_fails() {
        let provider = MockProvider::always_fails(LlmError::ServerOverloaded {
            message: "overloaded".to_string(),
        });
        let request = LlmRequest {
            prompt: "test".to_string(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };

        for _ in 0..3 {
            let result = provider.complete(request.clone()).await;
            assert!(result.is_err());
        }
        assert_eq!(provider.call_count(), 3);
    }

    #[tokio::test]
    async fn test_fails_then_succeeds() {
        let provider = MockProvider::fails_then_succeeds(
            2,
            LlmError::ServerOverloaded {
                message: "overloaded".to_string(),
            },
            "success",
        );
        let request = LlmRequest {
            prompt: "test".to_string(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
        };

        // First two calls fail
        assert!(provider.complete(request.clone()).await.is_err());
        assert!(provider.complete(request.clone()).await.is_err());

        // Third call succeeds
        let result = provider.complete(request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "success");
        assert_eq!(provider.call_count(), 3);
    }
}
