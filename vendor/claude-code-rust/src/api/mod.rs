//! API Module - Anthropic Messages first, OpenAI Chat Completions fallback.

use crate::config::{ApiProtocol, Settings};
use futures::{
    stream::{self, BoxStream},
    StreamExt,
};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, Response, StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::env;
use std::fmt;
use std::time::{Duration, SystemTime};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const API_5XX_MAX_ATTEMPTS: usize = 5;
const API_5XX_INITIAL_BACKOFF_MS: u64 = 250;
const API_429_MAX_ATTEMPTS: usize = 3;
const API_429_MAX_RETRY_AFTER_MS: u64 = 30_000;
const STREAM_IDLE_TIMEOUT_MS: u64 = 90_000;
const CLIENT_REQUEST_ID_HEADER: &str = "x-client-request-id";

#[derive(Clone)]
pub struct ApiClient {
    settings: Settings,
    http_client: std::sync::Arc<Client>,
}

impl ApiClient {
    pub fn new(settings: Settings) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(settings.api.timeout))
            .build()
            .unwrap_or_default();

        Self {
            settings,
            http_client: std::sync::Arc::new(http_client),
        }
    }

    pub fn get_api_key(&self) -> Option<String> {
        self.settings.api.get_api_key()
    }

    pub fn get_base_url(&self) -> String {
        self.settings.api.get_base_url()
    }

    pub fn get_model(&self) -> &str {
        &self.settings.model
    }

    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> anyhow::Result<ChatResponse> {
        match self.settings.api.protocol {
            ApiProtocol::OpenAiChatCompletions => self.chat_openai(messages, tools).await,
            ApiProtocol::AnthropicMessages => self
                .chat_anthropic(messages, tools)
                .await
                .map_err(anyhow::Error::from),
            ApiProtocol::Auto => match self.chat_anthropic(messages.clone(), tools.clone()).await {
                Ok(response) => Ok(response),
                Err(error) if error.supports_openai_fallback() => {
                    self.chat_openai(messages, tools).await
                }
                Err(error) => Err(error.into()),
            },
        }
    }

    pub async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> anyhow::Result<ModelStream> {
        match self.settings.api.protocol {
            ApiProtocol::OpenAiChatCompletions => self.chat_stream_openai(messages, tools).await,
            ApiProtocol::AnthropicMessages => self
                .chat_stream_anthropic(messages, tools)
                .await
                .map_err(anyhow::Error::from),
            ApiProtocol::Auto => match self
                .chat_stream_anthropic(messages.clone(), tools.clone())
                .await
            {
                Ok(stream) => Ok(stream),
                Err(error) if error.supports_openai_fallback() => {
                    self.chat_stream_openai(messages, tools).await
                }
                Err(error) => Err(error.into()),
            },
        }
    }

    async fn chat_anthropic(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ChatResponse, ApiCallError> {
        let api_key = self.require_api_key()?;
        let request = build_anthropic_request(
            self.settings.api.get_model_id(&self.settings.model),
            self.settings.api.max_tokens,
            false,
            messages,
            tools,
        );

        let response = self
            .send_anthropic_with_5xx_retry(&api_key, &request)
            .await?;
        let value: Value = response
            .response
            .json()
            .await
            .map_err(|error| ApiCallError::transport(error, Some(response.diagnostics)))?;
        Ok(anthropic_response_to_chat_response(value))
    }

    async fn chat_stream_anthropic(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ModelStream, ApiCallError> {
        let api_key = self.require_api_key()?;
        let request = build_anthropic_request(
            self.settings.api.get_model_id(&self.settings.model),
            self.settings.api.max_tokens,
            true,
            messages,
            tools,
        );

        let response = self
            .send_anthropic_with_5xx_retry(&api_key, &request)
            .await?;
        Ok(response_to_event_stream(
            response.response,
            StreamParser::Anthropic(AnthropicStreamParser::default()),
            response.diagnostics,
        ))
    }

    async fn chat_openai(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> anyhow::Result<ChatResponse> {
        let api_key = self
            .get_api_key()
            .ok_or_else(|| anyhow::anyhow!("API key not configured"))?;

        let request = build_openai_request(
            self.settings.api.get_model_id(&self.settings.model),
            self.settings.api.max_tokens,
            false,
            messages,
            tools,
        );

        let response = self
            .send_openai_with_5xx_retry(&api_key, &request)
            .await
            .map_err(anyhow::Error::from)?;

        let chat_response: ChatResponse = response
            .response
            .json()
            .await
            .map_err(|error| ApiCallError::transport(error, Some(response.diagnostics)))?;
        Ok(chat_response)
    }

    async fn chat_stream_openai(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> anyhow::Result<ModelStream> {
        let api_key = self
            .get_api_key()
            .ok_or_else(|| anyhow::anyhow!("API key not configured"))?;

        let request = build_openai_request(
            self.settings.api.get_model_id(&self.settings.model),
            self.settings.api.max_tokens,
            true,
            messages,
            tools,
        );

        let response = self
            .send_openai_with_5xx_retry(&api_key, &request)
            .await
            .map_err(anyhow::Error::from)?;

        Ok(response_to_event_stream(
            response.response,
            StreamParser::OpenAi,
            response.diagnostics,
        ))
    }

    fn anthropic_request(
        &self,
        url: String,
        api_key: &str,
        client_request_id: &str,
    ) -> reqwest::RequestBuilder {
        let mut request = self
            .http_client
            .post(url)
            .header("x-api-key", api_key)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header(CLIENT_REQUEST_ID_HEADER, client_request_id)
            .header("Content-Type", "application/json");

        if !self.settings.api.beta_headers.is_empty() {
            request = request.header("anthropic-beta", self.settings.api.beta_headers.join(","));
        }

        request
    }

    fn require_api_key(&self) -> Result<String, ApiCallError> {
        self.get_api_key().ok_or_else(|| ApiCallError::Other {
            error: anyhow::anyhow!("API key not configured"),
            diagnostics: None,
        })
    }

    async fn send_anthropic_with_5xx_retry(
        &self,
        api_key: &str,
        request: &Value,
    ) -> Result<ApiResponse, ApiCallError> {
        let mut attempt = 1usize;
        loop {
            let diagnostics = RequestDiagnostics::new();
            let response = self
                .anthropic_request(
                    self.anthropic_messages_url(),
                    api_key,
                    &diagnostics.client_request_id,
                )
                .json(request)
                .send()
                .await;

            match response {
                Err(error)
                    if is_retryable_transport_error(&error) && attempt < API_5XX_MAX_ATTEMPTS =>
                {
                    tokio::time::sleep(api_5xx_backoff(attempt)).await;
                    attempt += 1;
                    continue;
                }
                Err(error) => return Err(ApiCallError::transport(error, Some(diagnostics))),
                Ok(response) => match ensure_success(response, diagnostics).await {
                    Ok(response) => return Ok(response),
                    Err(error) if error.is_retryable_5xx() && attempt < API_5XX_MAX_ATTEMPTS => {
                        tokio::time::sleep(api_5xx_backoff(attempt)).await;
                        attempt += 1;
                    }
                    Err(error) if error.is_retryable_429() && attempt < API_429_MAX_ATTEMPTS => {
                        if let Some(delay) = error.retry_after_delay(attempt) {
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                        } else {
                            return Err(error);
                        }
                    }
                    Err(error) => return Err(error),
                },
            };
        }
    }

    async fn send_openai_with_5xx_retry(
        &self,
        api_key: &str,
        request: &ChatRequest,
    ) -> Result<ApiResponse, ApiCallError> {
        let mut attempt = 1usize;
        loop {
            let diagnostics = RequestDiagnostics::new();
            let response = self
                .openai_request(
                    self.openai_chat_completions_url(),
                    api_key,
                    &diagnostics.client_request_id,
                )
                .json(request)
                .send()
                .await;

            match response {
                Err(error)
                    if is_retryable_transport_error(&error) && attempt < API_5XX_MAX_ATTEMPTS =>
                {
                    tokio::time::sleep(api_5xx_backoff(attempt)).await;
                    attempt += 1;
                    continue;
                }
                Err(error) => return Err(ApiCallError::transport(error, Some(diagnostics))),
                Ok(response) => match ensure_success(response, diagnostics).await {
                    Ok(response) => return Ok(response),
                    Err(error) if error.is_retryable_5xx() && attempt < API_5XX_MAX_ATTEMPTS => {
                        tokio::time::sleep(api_5xx_backoff(attempt)).await;
                        attempt += 1;
                    }
                    Err(error) if error.is_retryable_429() && attempt < API_429_MAX_ATTEMPTS => {
                        if let Some(delay) = error.retry_after_delay(attempt) {
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                        } else {
                            return Err(error);
                        }
                    }
                    Err(error) => return Err(error),
                },
            };
        }
    }

    fn openai_request(
        &self,
        url: String,
        api_key: &str,
        client_request_id: &str,
    ) -> reqwest::RequestBuilder {
        self.http_client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header(CLIENT_REQUEST_ID_HEADER, client_request_id)
            .header("Content-Type", "application/json")
    }

    fn anthropic_messages_url(&self) -> String {
        api_url(&self.get_base_url(), "/v1/messages")
    }

    fn openai_chat_completions_url(&self) -> String {
        api_url(&self.get_base_url(), "/v1/chat/completions")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequestDiagnostics {
    client_request_id: String,
    request_id: Option<String>,
    cf_ray: Option<String>,
}

impl RequestDiagnostics {
    fn new() -> Self {
        Self {
            client_request_id: format!("req_{}", uuid::Uuid::new_v4()),
            request_id: None,
            cf_ray: None,
        }
    }

    fn with_response_headers(mut self, headers: &HeaderMap) -> Self {
        self.request_id = first_header_value(headers, &["request-id", "x-request-id"]);
        self.cf_ray = first_header_value(headers, &["cf-ray"]);
        self
    }

    fn suffix(&self) -> String {
        let mut parts = vec![format!("client_request_id={}", self.client_request_id)];
        if let Some(request_id) = &self.request_id {
            parts.push(format!("request_id={request_id}"));
        }
        if let Some(cf_ray) = &self.cf_ray {
            parts.push(format!("cf_ray={cf_ray}"));
        }
        format!(" [{}]", parts.join(", "))
    }
}

struct ApiResponse {
    response: Response,
    diagnostics: RequestDiagnostics,
}

#[derive(Debug)]
enum ApiCallError {
    Status {
        status: StatusCode,
        body: String,
        retry_after: Option<Duration>,
        diagnostics: RequestDiagnostics,
    },
    Other {
        error: anyhow::Error,
        diagnostics: Option<RequestDiagnostics>,
    },
}

impl ApiCallError {
    fn transport(error: impl Into<anyhow::Error>, diagnostics: Option<RequestDiagnostics>) -> Self {
        Self::Other {
            error: error.into(),
            diagnostics,
        }
    }

    fn supports_openai_fallback(&self) -> bool {
        match self {
            Self::Status { status, body, .. } => {
                matches!(
                    *status,
                    StatusCode::NOT_FOUND
                        | StatusCode::METHOD_NOT_ALLOWED
                        | StatusCode::NOT_IMPLEMENTED
                ) || ((*status == StatusCode::BAD_REQUEST
                    || *status == StatusCode::UNPROCESSABLE_ENTITY)
                    && body_indicates_unsupported_messages_api(body))
            }
            Self::Other { .. } => false,
        }
    }

    fn is_retryable_5xx(&self) -> bool {
        matches!(
            self,
            Self::Status { status, .. } if status.is_server_error()
        )
    }

    fn is_retryable_429(&self) -> bool {
        matches!(
            self,
            Self::Status { status, .. } if *status == StatusCode::TOO_MANY_REQUESTS
        )
    }

    fn retry_after_delay(&self, attempt: usize) -> Option<Duration> {
        match self {
            Self::Status { retry_after, .. } => match retry_after {
                Some(delay) if *delay <= Duration::from_millis(API_429_MAX_RETRY_AFTER_MS) => {
                    Some(*delay)
                }
                Some(_) => None,
                None => Some(api_429_backoff(attempt)),
            },
            Self::Other { .. } => None,
        }
    }
}

impl fmt::Display for ApiCallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status {
                status,
                body,
                diagnostics,
                ..
            } => write!(
                f,
                "API error ({}): {}{}",
                status,
                sanitize_error_body(body),
                diagnostics.suffix()
            ),
            Self::Other { error, diagnostics } => {
                if let Some(diagnostics) = diagnostics {
                    write!(f, "{error}{}", diagnostics.suffix())
                } else {
                    write!(f, "{error}")
                }
            }
        }
    }
}

impl std::error::Error for ApiCallError {}

async fn ensure_success(
    response: Response,
    diagnostics: RequestDiagnostics,
) -> Result<ApiResponse, ApiCallError> {
    let diagnostics = diagnostics.with_response_headers(response.headers());
    if response.status().is_success() {
        return Ok(ApiResponse {
            response,
            diagnostics,
        });
    }

    let status = response.status();
    let retry_after = parse_retry_after(response.headers().get("retry-after"));
    let body = response.text().await.unwrap_or_default();
    Err(ApiCallError::Status {
        status,
        body,
        retry_after,
        diagnostics,
    })
}

fn body_indicates_unsupported_messages_api(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    let mentions_messages = body.contains("messages") || body.contains("/v1/messages");
    let mentions_unsupported = [
        "unsupported",
        "not supported",
        "unknown endpoint",
        "not found",
        "invalid request",
        "unrecognized",
        "schema",
        "field",
    ]
    .iter()
    .any(|needle| body.contains(needle));

    mentions_messages && mentions_unsupported
}

fn first_header_value(headers: &HeaderMap, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        headers
            .get(*name)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_string())
    })
}

fn parse_retry_after(value: Option<&HeaderValue>) -> Option<Duration> {
    let value = value?.to_str().ok()?.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    let parsed = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let target = SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(parsed.timestamp().max(0) as u64))?;
    target.duration_since(SystemTime::now()).ok()
}

fn api_5xx_backoff(attempt: usize) -> Duration {
    let shift = attempt.saturating_sub(1).min(4) as u32;
    Duration::from_millis(API_5XX_INITIAL_BACKOFF_MS * 2u64.pow(shift))
}

fn api_429_backoff(attempt: usize) -> Duration {
    let shift = attempt.saturating_sub(1).min(3) as u32;
    Duration::from_millis(500 * 2u64.pow(shift))
}

fn stream_idle_timeout() -> Duration {
    env::var("CLAUDE_STREAM_IDLE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(STREAM_IDLE_TIMEOUT_MS))
}

fn is_retryable_transport_error(error: &reqwest::Error) -> bool {
    if error.is_timeout() || error.is_connect() {
        return true;
    }

    let message = error.to_string().to_ascii_lowercase();
    [
        "connection reset",
        "connection closed",
        "connection aborted",
        "broken pipe",
        "error sending request",
        "econnreset",
        "epipe",
        "early eof",
        "unexpected eof",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn sanitize_error_body(body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        return String::new();
    }

    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(message) = extract_json_error_message(&value) {
            return truncate_error_text(&message);
        }
        return truncate_error_text(&value.to_string());
    }

    if let Some(title) = extract_html_title(body) {
        return truncate_error_text(&title);
    }

    truncate_error_text(body)
}

fn extract_json_error_message(value: &Value) -> Option<String> {
    [
        &["error", "message"][..],
        &["error", "error", "message"][..],
        &["message"][..],
        &["error"][..],
    ]
    .iter()
    .find_map(|path| {
        let mut current = value;
        for key in *path {
            current = current.get(*key)?;
        }
        current.as_str().map(str::to_string)
    })
}

fn extract_html_title(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    Some(body[start..end].trim().to_string()).filter(|title| !title.is_empty())
}

fn truncate_error_text(text: &str) -> String {
    const MAX_ERROR_BODY_CHARS: usize = 1200;
    let text = text.trim();
    if text.chars().count() <= MAX_ERROR_BODY_CHARS {
        return text.to_string();
    }
    let mut truncated = text.chars().take(MAX_ERROR_BODY_CHARS).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn api_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    if let Some(rest) = path.strip_prefix("v1/") {
        if base.ends_with("/v1") {
            return format!("{}/{}", base, rest);
        }
    }
    format!("{}/{}", base, path)
}

fn build_openai_request(
    model: String,
    max_tokens: usize,
    stream: bool,
    messages: Vec<ChatMessage>,
    tools: Option<Vec<ToolDefinition>>,
) -> ChatRequest {
    let parallel_tool_calls = tools.as_ref().map(|_| false);
    ChatRequest {
        model,
        messages,
        max_tokens,
        stream,
        temperature: 0.7,
        tools,
        parallel_tool_calls,
    }
}

fn build_anthropic_request(
    model: String,
    max_tokens: usize,
    stream: bool,
    messages: Vec<ChatMessage>,
    tools: Option<Vec<ToolDefinition>>,
) -> Value {
    let mut system_parts = Vec::new();
    let mut anthropic_messages = Vec::new();

    for message in messages {
        if message.role == "system" {
            if let Some(content) = message.content {
                if !content.is_empty() {
                    system_parts.push(content);
                }
            }
            continue;
        }

        let role = match message.role.as_str() {
            "assistant" => "assistant",
            "tool" => "user",
            _ => "user",
        };

        let mut blocks = Vec::new();
        if let Some(content) = message.content {
            if !content.is_empty() {
                blocks.push(json!({
                    "type": "text",
                    "text": content,
                }));
            }
        }

        if let Some(tool_calls) = message.tool_calls {
            for tool_call in tool_calls {
                blocks.push(json!({
                    "type": "tool_use",
                    "id": tool_call.id,
                    "name": tool_call.function.name,
                    "input": parse_tool_arguments(&tool_call.function.arguments),
                }));
            }
        }

        if message.role == "tool" {
            if let Some(tool_call_id) = message.tool_call_id {
                blocks = vec![json!({
                    "type": "tool_result",
                    "tool_use_id": tool_call_id,
                    "content": blocks
                        .first()
                        .and_then(|block| block.get("text"))
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                })];
            }
        }

        if !blocks.is_empty() {
            anthropic_messages.push(json!({
                "role": role,
                "content": blocks,
            }));
        }
    }

    let mut request = json!({
        "model": model,
        "max_tokens": max_tokens,
        "stream": stream,
        "temperature": 0.7,
        "messages": anthropic_messages,
    });

    if !system_parts.is_empty() {
        request["system"] = Value::String(system_parts.join("\n\n"));
    }

    if let Some(tools) = tools {
        let anthropic_tools = tools
            .into_iter()
            .map(|tool| {
                json!({
                    "name": tool.function.name,
                    "description": tool.function.description,
                    "input_schema": tool.function.parameters,
                })
            })
            .collect::<Vec<_>>();
        request["tools"] = Value::Array(anthropic_tools);
    }

    request
}

fn parse_tool_arguments(arguments: &str) -> Value {
    match serde_json::from_str::<Value>(arguments) {
        Ok(value) if value.is_object() => value,
        Ok(value) => json!({ "arguments": value }),
        Err(_) => json!({ "arguments": arguments }),
    }
}

fn anthropic_response_to_chat_response(value: Value) -> ChatResponse {
    let id = value["id"].as_str().unwrap_or_default().to_string();
    let model = value["model"].as_str().unwrap_or_default().to_string();
    let stop_reason = value["stop_reason"].as_str().map(map_anthropic_stop_reason);

    let mut text = String::new();
    let mut tool_calls = Vec::new();
    if let Some(content) = value["content"].as_array() {
        for block in content {
            match block["type"].as_str().unwrap_or_default() {
                "text" => {
                    if let Some(block_text) = block["text"].as_str() {
                        text.push_str(block_text);
                    }
                }
                "tool_use" => {
                    tool_calls.push(ToolCall {
                        id: block["id"].as_str().unwrap_or_default().to_string(),
                        r#type: "function".to_string(),
                        function: ToolCallFunction {
                            name: block["name"].as_str().unwrap_or_default().to_string(),
                            arguments: block
                                .get("input")
                                .cloned()
                                .unwrap_or_else(|| json!({}))
                                .to_string(),
                        },
                    });
                }
                _ => {}
            }
        }
    }

    let usage = value.get("usage").and_then(|usage| {
        let input = usage["input_tokens"].as_u64().unwrap_or(0) as usize;
        let output = usage["output_tokens"].as_u64().unwrap_or(0) as usize;
        Some(Usage {
            prompt_tokens: input,
            completion_tokens: output,
            total_tokens: input + output,
        })
    });

    ChatResponse {
        id,
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model,
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: if text.is_empty() { None } else { Some(text) },
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
                tool_call_id: None,
            },
            finish_reason: stop_reason,
        }],
        usage,
    }
}

fn map_anthropic_stop_reason(reason: &str) -> String {
    match reason {
        "tool_use" => "tool_calls",
        "end_turn" => "stop",
        "max_tokens" => "length",
        other => other,
    }
    .to_string()
}

pub type ModelStream = BoxStream<'static, anyhow::Result<ModelStreamEvent>>;

#[derive(Debug, Clone, PartialEq)]
pub enum ModelStreamEvent {
    TextDelta(String),
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: String,
    },
    Finish {
        reason: Option<String>,
    },
    Usage(Usage),
}

enum StreamParser {
    OpenAi,
    Anthropic(AnthropicStreamParser),
}

struct SseState {
    bytes: BoxStream<'static, Result<Vec<u8>, reqwest::Error>>,
    buffer: String,
    queue: VecDeque<anyhow::Result<ModelStreamEvent>>,
    parser: StreamParser,
    diagnostics: RequestDiagnostics,
    idle_timeout: Duration,
    done: bool,
}

fn response_to_event_stream(
    response: Response,
    parser: StreamParser,
    diagnostics: RequestDiagnostics,
) -> ModelStream {
    response_to_event_stream_with_idle_timeout(response, parser, diagnostics, stream_idle_timeout())
}

fn response_to_event_stream_with_idle_timeout(
    response: Response,
    parser: StreamParser,
    diagnostics: RequestDiagnostics,
    idle_timeout: Duration,
) -> ModelStream {
    let bytes = response
        .bytes_stream()
        .map(|item| item.map(|b| b.to_vec()))
        .boxed();
    let state = SseState {
        bytes,
        buffer: String::new(),
        queue: VecDeque::new(),
        parser,
        diagnostics,
        idle_timeout,
        done: false,
    };

    stream::unfold(state, |mut state| async move {
        loop {
            if let Some(event) = state.queue.pop_front() {
                return Some((event, state));
            }
            if state.done {
                return None;
            }

            match tokio::time::timeout(state.idle_timeout, state.bytes.next()).await {
                Err(_) => {
                    state.done = true;
                    return Some((
                        Err(anyhow::anyhow!(
                            "Stream idle timeout after {}ms{}",
                            state.idle_timeout.as_millis(),
                            state.diagnostics.suffix()
                        )),
                        state,
                    ));
                }
                Ok(result) => match result {
                    Some(Ok(bytes)) => {
                        state.buffer.push_str(&String::from_utf8_lossy(&bytes));
                        drain_sse_lines(&mut state);
                    }
                    Some(Err(error)) => {
                        state.done = true;
                        return Some((
                            Err(anyhow::anyhow!(
                                "Stream error: {}{}",
                                error,
                                state.diagnostics.suffix()
                            )),
                            state,
                        ));
                    }
                    None => {
                        if !state.buffer.trim().is_empty() {
                            drain_sse_lines(&mut state);
                        }
                        state.done = true;
                    }
                },
            }
        }
    })
    .boxed()
}

fn drain_sse_lines(state: &mut SseState) {
    while let Some(pos) = state.buffer.find('\n') {
        let line = state.buffer[..pos].trim().to_string();
        state.buffer = state.buffer[pos + 1..].to_string();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with("data: ") {
            continue;
        }

        let data = &line[6..];
        if data == "[DONE]" {
            state.done = true;
            continue;
        }

        match &mut state.parser {
            StreamParser::OpenAi => parse_openai_stream_data(data, &mut state.queue),
            StreamParser::Anthropic(parser) => parser.parse_data(data, &mut state.queue),
        }
    }
}

fn parse_openai_stream_data(data: &str, queue: &mut VecDeque<anyhow::Result<ModelStreamEvent>>) {
    let Ok(value) = serde_json::from_str::<Value>(data) else {
        return;
    };

    if let Some(usage) = value.get("usage").filter(|u| u.is_object()) {
        queue.push_back(Ok(ModelStreamEvent::Usage(Usage {
            prompt_tokens: usage["prompt_tokens"].as_u64().unwrap_or(0) as usize,
            completion_tokens: usage["completion_tokens"].as_u64().unwrap_or(0) as usize,
            total_tokens: usage["total_tokens"].as_u64().unwrap_or(0) as usize,
        })));
    }

    let Some(choice) = value["choices"]
        .as_array()
        .and_then(|choices| choices.first())
    else {
        return;
    };

    if let Some(content) = choice["delta"]["content"].as_str() {
        if !content.is_empty() {
            queue.push_back(Ok(ModelStreamEvent::TextDelta(content.to_string())));
        }
    }

    if let Some(calls) = choice["delta"]["tool_calls"].as_array() {
        for call in calls {
            let index = call["index"].as_u64().unwrap_or(0) as usize;
            queue.push_back(Ok(ModelStreamEvent::ToolCallDelta {
                index,
                id: call["id"].as_str().map(str::to_string),
                name: call["function"]["name"].as_str().map(str::to_string),
                arguments_delta: call["function"]["arguments"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            }));
        }
    }

    if choice.get("finish_reason").is_some() && !choice["finish_reason"].is_null() {
        queue.push_back(Ok(ModelStreamEvent::Finish {
            reason: choice["finish_reason"].as_str().map(str::to_string),
        }));
    }
}

#[derive(Default)]
struct AnthropicStreamParser {
    input_tokens: usize,
    output_tokens: usize,
}

impl AnthropicStreamParser {
    fn parse_data(&mut self, data: &str, queue: &mut VecDeque<anyhow::Result<ModelStreamEvent>>) {
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            return;
        };

        match value["type"].as_str().unwrap_or_default() {
            "message_start" => {
                if let Some(usage) = value["message"].get("usage") {
                    self.input_tokens = usage["input_tokens"].as_u64().unwrap_or(0) as usize;
                    self.output_tokens = usage["output_tokens"].as_u64().unwrap_or(0) as usize;
                    self.push_usage(queue);
                }
            }
            "content_block_start" => {
                let block = &value["content_block"];
                if block["type"].as_str() == Some("tool_use") {
                    queue.push_back(Ok(ModelStreamEvent::ToolCallDelta {
                        index: value["index"].as_u64().unwrap_or(0) as usize,
                        id: block["id"].as_str().map(str::to_string),
                        name: block["name"].as_str().map(str::to_string),
                        arguments_delta: block
                            .get("input")
                            .filter(|input| {
                                !input.is_null() && !input.as_object().is_some_and(|o| o.is_empty())
                            })
                            .map(Value::to_string)
                            .unwrap_or_default(),
                    }));
                }
            }
            "content_block_delta" => {
                let delta = &value["delta"];
                match delta["type"].as_str().unwrap_or_default() {
                    "text_delta" => {
                        if let Some(text) = delta["text"].as_str() {
                            if !text.is_empty() {
                                queue.push_back(Ok(ModelStreamEvent::TextDelta(text.to_string())));
                            }
                        }
                    }
                    "input_json_delta" => {
                        queue.push_back(Ok(ModelStreamEvent::ToolCallDelta {
                            index: value["index"].as_u64().unwrap_or(0) as usize,
                            id: None,
                            name: None,
                            arguments_delta: delta["partial_json"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string(),
                        }));
                    }
                    _ => {}
                }
            }
            "message_delta" => {
                if let Some(usage) = value.get("usage") {
                    self.output_tokens = usage["output_tokens"].as_u64().unwrap_or(0) as usize;
                    self.push_usage(queue);
                }
                if let Some(reason) = value["delta"]["stop_reason"].as_str() {
                    queue.push_back(Ok(ModelStreamEvent::Finish {
                        reason: Some(map_anthropic_stop_reason(reason)),
                    }));
                }
            }
            _ => {}
        }
    }

    fn push_usage(&self, queue: &mut VecDeque<anyhow::Result<ModelStreamEvent>>) {
        queue.push_back(Ok(ModelStreamEvent::Usage(Usage {
            prompt_tokens: self.input_tokens,
            completion_tokens: self.output_tokens,
            total_tokens: self.input_tokens + self.output_tokens,
        })));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: ToolFunction,
}

impl ToolDefinition {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            r#type: "function".to_string(),
            function: ToolFunction {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_with_tools(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: usize,
    stream: bool,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: i32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Usage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamChoice {
    pub index: i32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Delta {
    pub role: Option<String>,
    pub content: Option<String>,
}

pub type AnthropicClient = ApiClient;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ApiProtocol, Settings};
    use futures::StreamExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use wiremock::matchers::{header_exists, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn endpoint_helper_does_not_duplicate_v1() {
        assert_eq!(
            api_url("https://api.example.com", "/v1/messages"),
            "https://api.example.com/v1/messages"
        );
        assert_eq!(
            api_url("https://api.example.com/v1", "/v1/messages"),
            "https://api.example.com/v1/messages"
        );
        assert_eq!(
            api_url("https://api.example.com/v1/", "v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn builds_anthropic_request_with_system_tools_and_tool_results() {
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "search".to_string(),
                arguments: r#"{"query":"alpha"}"#.to_string(),
            },
        };
        let request = build_anthropic_request(
            "claude-test".to_string(),
            123,
            false,
            vec![
                ChatMessage::system("system prompt"),
                ChatMessage::user("hello"),
                ChatMessage::assistant_with_tools(vec![tool_call]),
                ChatMessage::tool("call_1", "result text"),
            ],
            Some(vec![ToolDefinition::new(
                "search",
                "Search files",
                json!({"type":"object","properties":{"query":{"type":"string"}}}),
            )]),
        );

        assert_eq!(request["system"], "system prompt");
        assert_eq!(request["messages"][0]["role"], "user");
        assert_eq!(request["messages"][1]["content"][0]["type"], "tool_use");
        assert_eq!(
            request["messages"][1]["content"][0]["input"]["query"],
            "alpha"
        );
        assert_eq!(request["messages"][2]["content"][0]["type"], "tool_result");
        assert_eq!(request["tools"][0]["input_schema"]["type"], "object");
    }

    #[test]
    fn unsupported_messages_body_is_fallback_eligible() {
        assert!(body_indicates_unsupported_messages_api(
            "invalid request: /v1/messages is not supported"
        ));
        assert!(!body_indicates_unsupported_messages_api(
            "authentication failed for this token"
        ));
    }

    #[test]
    fn parses_anthropic_stream_events() {
        let mut parser = AnthropicStreamParser::default();
        let mut queue = VecDeque::new();
        parser.parse_data(
            r#"{"type":"message_start","message":{"usage":{"input_tokens":5,"output_tokens":0}}}"#,
            &mut queue,
        );
        parser.parse_data(
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}"#,
            &mut queue,
        );
        parser.parse_data(
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"read","input":{}}}"#,
            &mut queue,
        );
        parser.parse_data(
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"a\"}"}}"#,
            &mut queue,
        );
        parser.parse_data(
            r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":7}}"#,
            &mut queue,
        );

        let events = queue
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()
            .unwrap();
        assert!(events.contains(&ModelStreamEvent::TextDelta("hi".to_string())));
        assert!(events.iter().any(|event| matches!(
            event,
            ModelStreamEvent::ToolCallDelta {
                id: Some(id),
                name: Some(name),
                ..
            } if id == "toolu_1" && name == "read"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ModelStreamEvent::Finish {
                reason: Some(reason)
            } if reason == "tool_calls"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ModelStreamEvent::Usage(usage)
                if usage.prompt_tokens == 5 && usage.completion_tokens == 7
        )));
    }

    #[test]
    fn parses_openai_stream_events() {
        let mut queue = VecDeque::new();
        parse_openai_stream_data(
            r#"{"choices":[{"delta":{"content":"hello","tool_calls":[{"index":0,"id":"call_1","function":{"name":"read","arguments":"{\"path\""}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}"#,
            &mut queue,
        );

        let events = queue
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()
            .unwrap();
        assert!(events.contains(&ModelStreamEvent::TextDelta("hello".to_string())));
        assert!(events.iter().any(|event| matches!(
            event,
            ModelStreamEvent::ToolCallDelta {
                id: Some(id),
                name: Some(name),
                ..
            } if id == "call_1" && name == "read"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ModelStreamEvent::Finish {
                reason: Some(reason)
            } if reason == "tool_calls"
        )));
    }

    #[test]
    fn old_api_config_defaults_to_auto_protocol() {
        let config: crate::config::ApiConfig = serde_json::from_value(json!({
            "api_key": "key",
            "base_url": "https://api.example.com",
            "max_tokens": 12,
            "timeout": 30,
            "streaming": true,
            "beta_headers": []
        }))
        .unwrap();

        assert_eq!(config.protocol, ApiProtocol::Auto);
    }

    #[test]
    fn sanitizes_nested_json_and_html_error_bodies() {
        assert_eq!(
            sanitize_error_body(r#"{"error":{"message":"proxy failed"}}"#),
            "proxy failed"
        );
        assert_eq!(
            sanitize_error_body(r#"{"error":{"error":{"message":"anthropic failed"}}}"#),
            "anthropic failed"
        );
        assert_eq!(
            sanitize_error_body("<html><head><title>Cloudflare 502</title></head></html>"),
            "Cloudflare 502"
        );
    }

    #[test]
    fn parses_retry_after_seconds_and_http_date() {
        let seconds = HeaderValue::from_static("2");
        assert_eq!(
            parse_retry_after(Some(&seconds)),
            Some(Duration::from_secs(2))
        );

        let future = (chrono::Utc::now() + chrono::Duration::seconds(2)).to_rfc2822();
        let date = HeaderValue::from_str(&future).unwrap();
        let parsed = parse_retry_after(Some(&date)).unwrap();
        assert!(parsed <= Duration::from_secs(3));
    }

    #[tokio::test]
    async fn auto_chat_falls_back_when_messages_endpoint_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl_1",
                "object": "chat.completion",
                "created": 1,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "fallback ok" },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3 }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings(&server));
        let response = client
            .chat(vec![ChatMessage::user("hi")], None)
            .await
            .unwrap();

        assert_eq!(
            response.choices[0].message.content.as_deref(),
            Some("fallback ok")
        );
    }

    #[tokio::test]
    async fn request_includes_unique_client_request_ids() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header_exists(CLIENT_REQUEST_ID_HEADER))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl_1",
                "object": "chat.completion",
                "created": 1,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "ok" },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3 }
            })))
            .expect(2)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings_with_protocol(
            &server,
            ApiProtocol::OpenAiChatCompletions,
        ));
        client
            .chat(vec![ChatMessage::user("one")], None)
            .await
            .unwrap();
        client
            .chat(vec![ChatMessage::user("two")], None)
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        let ids = requests
            .iter()
            .map(|request| {
                request
                    .headers
                    .get(CLIENT_REQUEST_ID_HEADER)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
    }

    #[tokio::test]
    async fn http_error_includes_clean_body_and_diagnostics() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(401)
                    .append_header("request-id", "req_upstream")
                    .append_header("cf-ray", "cf123")
                    .set_body_json(json!({"error":{"message":"bad key"}})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings(&server));
        let error = client
            .chat(vec![ChatMessage::user("hi")], None)
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("bad key"));
        assert!(error.contains("client_request_id=req_"));
        assert!(error.contains("request_id=req_upstream"));
        assert!(error.contains("cf_ray=cf123"));
    }

    #[tokio::test]
    async fn auto_chat_does_not_fallback_on_auth_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings(&server));
        let error = client
            .chat(vec![ChatMessage::user("hi")], None)
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("401"));
        assert!(error.contains("bad key"));
    }

    #[tokio::test]
    async fn auto_chat_retries_5xx_then_succeeds_without_fallback() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(502).set_body_string("temporary bad gateway"))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "model": "test-model",
                "content": [{ "type": "text", "text": "retry ok" }],
                "stop_reason": "end_turn",
                "usage": { "input_tokens": 1, "output_tokens": 2 }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings(&server));
        let response = client
            .chat(vec![ChatMessage::user("hi")], None)
            .await
            .unwrap();

        assert_eq!(
            response.choices[0].message.content.as_deref(),
            Some("retry ok")
        );
    }

    #[tokio::test]
    async fn auto_chat_retries_5xx_five_times_and_does_not_fallback() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(500).set_body_string("temporary outage"))
            .expect(API_5XX_MAX_ATTEMPTS as u64)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings(&server));
        let error = client
            .chat(vec![ChatMessage::user("hi")], None)
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("500"));
        assert!(error.contains("temporary outage"));
    }

    #[tokio::test]
    async fn openai_chat_retries_5xx_then_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl_1",
                "object": "chat.completion",
                "created": 1,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "openai retry ok" },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3 }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings_with_protocol(
            &server,
            ApiProtocol::OpenAiChatCompletions,
        ));
        let response = client
            .chat(vec![ChatMessage::user("hi")], None)
            .await
            .unwrap();

        assert_eq!(
            response.choices[0].message.content.as_deref(),
            Some("openai retry ok")
        );
    }

    #[tokio::test]
    async fn openai_stream_retries_5xx_five_times_before_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(502).set_body_string("bad gateway"))
            .expect(API_5XX_MAX_ATTEMPTS as u64)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings_with_protocol(
            &server,
            ApiProtocol::OpenAiChatCompletions,
        ));
        let error = match client
            .chat_stream(vec![ChatMessage::user("hi")], None)
            .await
        {
            Ok(_) => panic!("expected stream request to fail after 5xx retries"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains("502"));
        assert!(error.contains("bad gateway"));
    }

    #[tokio::test]
    async fn openai_chat_retries_429_retry_after_seconds() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .append_header("retry-after", "0")
                    .set_body_string("rate limited"),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl_1",
                "object": "chat.completion",
                "created": 1,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "429 retry ok" },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3 }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings_with_protocol(
            &server,
            ApiProtocol::OpenAiChatCompletions,
        ));
        let response = client
            .chat(vec![ChatMessage::user("hi")], None)
            .await
            .unwrap();

        assert_eq!(
            response.choices[0].message.content.as_deref(),
            Some("429 retry ok")
        );
    }

    #[tokio::test]
    async fn openai_chat_does_not_retry_overlong_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .append_header("retry-after", "31")
                    .set_body_string("rate limited"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings_with_protocol(
            &server,
            ApiProtocol::OpenAiChatCompletions,
        ));
        let error = client
            .chat(vec![ChatMessage::user("hi")], None)
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("429"));
        assert!(error.contains("rate limited"));
    }

    #[tokio::test]
    async fn openai_chat_retries_connection_closed_before_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for _ in 0..2 {
                let (socket, _) = listener.accept().await.unwrap();
                drop(socket);
            }

            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0u8; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            let body = json!({
                "id": "chatcmpl_1",
                "object": "chat.completion",
                "created": 1,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "transport retry ok" },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3 }
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let mut settings = Settings::default();
        settings.api.api_key = Some("test-key".to_string());
        settings.api.base_url = format!("http://{addr}");
        settings.api.protocol = ApiProtocol::OpenAiChatCompletions;
        settings.model = "test-model".to_string();

        let client = ApiClient::new(settings);
        let response = client
            .chat(vec![ChatMessage::user("hi")], None)
            .await
            .unwrap();

        assert_eq!(
            response.choices[0].message.content.as_deref(),
            Some("transport retry ok")
        );
    }

    #[tokio::test]
    async fn stream_idle_watchdog_errors_after_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "content-type: text/event-stream\r\n",
                "request-id: stream_req\r\n",
                "\r\n"
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            tokio::time::sleep(Duration::from_millis(250)).await;
        });

        let response = reqwest::Client::new()
            .get(format!("http://{addr}"))
            .send()
            .await
            .unwrap();
        let diagnostics = RequestDiagnostics::new().with_response_headers(response.headers());
        let mut stream = response_to_event_stream_with_idle_timeout(
            response,
            StreamParser::OpenAi,
            diagnostics,
            Duration::from_millis(40),
        );
        let error = stream.next().await.unwrap().unwrap_err().to_string();

        assert!(error.contains("Stream idle timeout after 40ms"));
        assert!(error.contains("request_id=stream_req"));
    }

    #[tokio::test]
    async fn auto_stream_falls_back_before_streaming_starts() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(405).set_body_string("method not allowed"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("content-type", "text/event-stream")
                    .set_body_string(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"stream ok\"},\"finish_reason\":null}]}\n\n\
                         data: [DONE]\n\n",
                    ),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = ApiClient::new(test_settings(&server));
        let mut stream = client
            .chat_stream(vec![ChatMessage::user("hi")], None)
            .await
            .unwrap();

        let mut text = String::new();
        while let Some(event) = stream.next().await {
            if let ModelStreamEvent::TextDelta(delta) = event.unwrap() {
                text.push_str(&delta);
            }
        }

        assert_eq!(text, "stream ok");
    }

    fn test_settings(server: &MockServer) -> Settings {
        test_settings_with_protocol(server, ApiProtocol::Auto)
    }

    fn test_settings_with_protocol(server: &MockServer, protocol: ApiProtocol) -> Settings {
        let mut settings = Settings::default();
        settings.api.api_key = Some("test-key".to_string());
        settings.api.base_url = server.uri();
        settings.api.protocol = protocol;
        settings.model = "test-model".to_string();
        settings
    }
}
