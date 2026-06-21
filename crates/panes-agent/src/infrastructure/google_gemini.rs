// google_gemini.rs — Google Gemini API client for panes-agent.
//
// Implements ModelClient using the Gemini `streamGenerateContent` REST endpoint
// (SSE). Text deltas are emitted as they arrive; tool calls and usage are
// decoded from the streamed chunks.
//
// Supports:
// - Text conversations (system instruction via systemInstruction field)
// - Tool declarations (functionDeclarations) and streamed tool calls
// - Custom base URL via GOOGLE_BASE_URL env var
// - API key via GOOGLE_API_KEY env var (sent as query parameter)

use std::collections::VecDeque;

use anyhow::Context;
use futures::{stream, stream::BoxStream, StreamExt};
use serde_json::{json, Value};

use crate::{
    application::ports::ModelClient,
    domain::{
        conversation::{AgentMessage, MessageContent, Role},
        system_prompt::build_system_prompt,
        telemetry::TokenUsage,
        tools::ToolCall,
    },
    interfaces::{ModelRequest, ModelStreamEvent},
};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com";
const DEFAULT_MODEL: &str = "gemini-3.5-flash";
const DEFAULT_MAX_TOKENS: u32 = 4096;

#[derive(Debug, Clone)]
pub struct GoogleGeminiClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
    max_tokens: u32,
    tool_specs: Vec<Value>,
}

impl GoogleGeminiClient {
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
            max_tokens: DEFAULT_MAX_TOKENS,
            tool_specs: Vec::new(),
        }
    }

    pub fn with_tool_specs(mut self, tool_specs: Vec<Value>) -> Self {
        self.tool_specs = tool_specs;
        self
    }

    pub fn from_env(model: impl Into<String>) -> anyhow::Result<Self> {
        let api_key = std::env::var("GOOGLE_API_KEY")
            .context("GOOGLE_API_KEY is required for Google Gemini")?;
        let base_url = std::env::var("GOOGLE_BASE_URL")
            .or_else(|_| std::env::var("GOOGLE_API_BASE"))
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let requested_model = model.into();
        let model = if requested_model.trim().is_empty() {
            std::env::var("GOOGLE_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
        } else {
            requested_model
        };

        Ok(Self::new(api_key, base_url, model))
    }

    pub fn default_model() -> &'static str {
        DEFAULT_MODEL
    }

    pub fn has_env_credentials() -> bool {
        std::env::var("GOOGLE_API_KEY")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    }

    /// Build the Gemini request body from a ModelRequest.
    fn build_body(&self, request: &ModelRequest) -> Value {
        let contents = messages_to_gemini(&request.messages);
        let system_instruction = build_system_prompt(&request.system_context);

        let mut body = json!({
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": self.max_tokens,
            },
        });

        if !system_instruction.is_empty() {
            body["systemInstruction"] = json!({
                "parts": [{ "text": system_instruction }]
            });
        }

        if !self.tool_specs.is_empty() {
            body["tools"] = json!([{
                "functionDeclarations": self.tool_specs
            }]);
        }

        body
    }

    async fn send_request(
        &self,
        request: ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>> {
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.base_url.trim_end_matches('/'),
            self.model,
            self.api_key,
        );

        let body = self.build_body(&request);

        let response = self.http.post(&url).json(&body).send().await?;

        let status = response.status();
        if !status.is_success() {
            let err_body = response.text().await.unwrap_or_default();
            anyhow::bail!("Google Gemini request failed with {status}: {err_body}");
        }

        // SSE: each `data:` line is a JSON object with (possibly partial)
        // candidate text, tool calls, and a trailing usageMetadata.  Events can
        // span chunk boundaries, so we buffer bytes and split on newlines.
        let byte_stream = response
            .bytes_stream()
            .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
            .boxed();

        Ok(Box::pin(stream::unfold(
            GeminiStreamState {
                byte_stream,
                buffer: String::new(),
                pending: VecDeque::new(),
                finished: false,
            },
            next_gemini_event,
        )))
    }
}

#[async_trait::async_trait]
impl ModelClient for GoogleGeminiClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>> {
        self.send_request(request).await
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert Panes messages to Gemini "contents" array.
fn messages_to_gemini(messages: &[AgentMessage]) -> Vec<Value> {
    messages
        .iter()
        .filter_map(|msg| {
            let role = match msg.role {
                Role::User => "user",
                Role::Assistant => "model",
            };
            let text = message_text(msg);
            if text.is_empty() {
                return None;
            }
            Some(json!({
                "role": role,
                "parts": [{ "text": text }]
            }))
        })
        .collect()
}

/// Extract plain text from an AgentMessage.
fn message_text(msg: &AgentMessage) -> String {
    msg.content
        .iter()
        .filter_map(|block| match block {
            MessageContent::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract concatenated text parts from a Gemini response chunk.
fn extract_text_from_response(response: &Value) -> Option<String> {
    let text: String = response
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(|parts| parts.as_array())?
        .iter()
        .filter_map(|part| part.get("text").and_then(|t| t.as_str()))
        .collect();
    Some(text)
}

/// Extract function calls from a Gemini response chunk.
fn extract_tool_calls(response: &Value) -> Vec<ToolCall> {
    let Some(parts) = response
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(|parts| parts.as_array())
    else {
        return Vec::new();
    };
    parts
        .iter()
        .filter_map(|part| {
            let fc = part.get("functionCall")?;
            let name = fc.get("name")?.as_str()?.to_string();
            let input = fc.get("args").cloned().unwrap_or(json!({}));
            let id = format!("gemini-{}", name);
            Some(ToolCall { id, name, input })
        })
        .collect()
}

/// Extract token usage from the response's usageMetadata.
fn extract_usage(response: &Value) -> Option<TokenUsage> {
    let meta = response.get("usageMetadata")?;
    let input = meta
        .get("promptTokenCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output = meta
        .get("candidatesTokenCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some(TokenUsage {
        input,
        output,
        reasoning: None,
        cache_read: None,
        cache_write: None,
        cost_usd: None,
    })
}

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

/// State for the SSE unfold loop. Mirrors the openai/anthropic clients:
/// raw bytes are buffered and split into `\n\n`-delimited frames.
struct GeminiStreamState {
    byte_stream: BoxStream<'static, Result<Vec<u8>, reqwest::Error>>,
    buffer: String,
    pending: VecDeque<ModelStreamEvent>,
    finished: bool,
}

async fn next_gemini_event(
    mut state: GeminiStreamState,
) -> Option<(ModelStreamEvent, GeminiStreamState)> {
    loop {
        if let Some(event) = state.pending.pop_front() {
            return Some((event, state));
        }
        if state.finished {
            return None;
        }
        let chunk = match state.byte_stream.next().await {
            Some(Ok(bytes)) => String::from_utf8_lossy(&bytes).into_owned(),
            Some(Err(error)) => {
                state.finished = true;
                return Some((ModelStreamEvent::Error(error.to_string()), state));
            }
            None => {
                // Stream ended — emit Done and terminate.
                state.pending.push_back(ModelStreamEvent::Done);
                state.finished = true;
                continue;
            }
        };
        state.buffer.push_str(&chunk);
        while let Some(frame_end) = state.buffer.find("\n\n") {
            let frame = state.buffer[..frame_end].to_string();
            state.buffer = state.buffer[frame_end + 2..].to_string();
            parse_gemini_frame(&frame, &mut state);
        }
    }
}

/// Parse one SSE frame: a `data:` line whose payload is a single JSON object.
fn parse_gemini_frame(frame: &str, state: &mut GeminiStreamState) {
    for line in frame.lines() {
        let Some(data) = line.trim().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(text) = extract_text_from_response(&value) {
            if !text.is_empty() {
                state
                    .pending
                    .push_back(ModelStreamEvent::TextDelta(text));
            }
        }
        for call in extract_tool_calls(&value) {
            state.pending.push_back(ModelStreamEvent::ToolUse(call));
        }
        if let Some(usage) = extract_usage(&value) {
            state.pending.push_back(ModelStreamEvent::Usage(usage));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_from_chunk() {
        let chunk = json!({
            "candidates": [{
                "content": { "parts": [{ "text": "hello" }] }
            }]
        });
        assert_eq!(
            extract_text_from_response(&chunk).as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn extracts_function_calls() {
        let chunk = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "run_command",
                            "args": { "cmd": "ls" }
                        }
                    }]
                }
            }]
        });
        let calls = extract_tool_calls(&chunk);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "run_command");
        assert_eq!(calls[0].input["cmd"], "ls");
    }

    #[test]
    fn extracts_usage_metadata() {
        let chunk = json!({
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 20
            }
        });
        let usage = extract_usage(&chunk).unwrap();
        assert_eq!(usage.input, 10);
        assert_eq!(usage.output, 20);
    }

    #[test]
    fn parses_sse_frame_into_events() {
        let mut state = GeminiStreamState {
            byte_stream: futures::stream::empty().boxed(),
            buffer: String::new(),
            pending: VecDeque::new(),
            finished: false,
        };
        let frame = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]}}]}";
        parse_gemini_frame(frame, &mut state);
        assert!(matches!(
            state.pending.pop_front(),
            Some(ModelStreamEvent::TextDelta(t)) if t == "hi"
        ));
    }
}
