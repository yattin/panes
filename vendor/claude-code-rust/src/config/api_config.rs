//! API Configuration

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiProtocol {
    Auto,
    AnthropicMessages,
    OpenAiChatCompletions,
}

fn default_protocol() -> ApiProtocol {
    ApiProtocol::Auto
}

fn default_base_url() -> String {
    "https://api.anthropic.com".to_string()
}

fn default_max_tokens() -> usize {
    4096
}

fn default_timeout() -> u64 {
    120
}

fn default_streaming() -> bool {
    true
}

/// Anthropic API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API key (can be set via environment variable)
    #[serde(default)]
    pub api_key: Option<String>,
    /// Base URL for API requests
    #[serde(default = "default_base_url")]
    pub base_url: String,
    /// Preferred upstream protocol.
    #[serde(default = "default_protocol")]
    pub protocol: ApiProtocol,
    /// Maximum tokens per request
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// Enable streaming responses
    #[serde(default = "default_streaming")]
    pub streaming: bool,
    /// Beta headers to include
    #[serde(default)]
    pub beta_headers: Vec<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: default_base_url(),
            protocol: default_protocol(),
            max_tokens: 4096,
            timeout: 120,
            streaming: true,
            beta_headers: vec![],
        }
    }
}

impl ApiConfig {
    /// Get the API key from config file only
    pub fn get_api_key(&self) -> Option<String> {
        self.api_key.clone()
    }

    /// Get the base URL from config file only
    pub fn get_base_url(&self) -> String {
        self.base_url.clone()
    }

    /// Get the model ID for the given model name
    pub fn get_model_id(&self, model: &str) -> String {
        match model {
            "opus" => "claude-3-opus-20240229".to_string(),
            "sonnet" => "claude-3-5-sonnet-20241022".to_string(),
            "haiku" => "claude-3-5-haiku-20241022".to_string(),
            _ => model.to_string(),
        }
    }
}
