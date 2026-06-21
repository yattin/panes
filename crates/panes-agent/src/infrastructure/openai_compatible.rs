use std::collections::{BTreeMap, VecDeque};

use anyhow::Context;
use async_trait::async_trait;
use futures::{stream, stream::BoxStream, StreamExt};
use serde_json::{json, Value};

use crate::{
    application::ports::ModelClient,
    domain::{
        conversation::{AgentMessage, MessageContent, Role},
        system_prompt::build_system_prompt,
        telemetry::TokenUsage,
        tools::{ToolCall, ToolSpec},
    },
    infrastructure::native_tools,
    interfaces::{ModelRequest, ModelStreamEvent},
};

const DEFAULT_BASE_URL: &str = "https://api.openai.com";
const DEFAULT_MODEL: &str = "gpt-4o";
const DEFAULT_MAX_TOKENS: u32 = 4096;

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleClient {
    http: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
    model: String,
    max_tokens: u32,
    tool_specs: Vec<Value>,
}

impl OpenAiCompatibleClient {
    pub fn new(
        api_key: Option<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key,
            base_url: base_url.into(),
            model: model.into(),
            max_tokens: DEFAULT_MAX_TOKENS,
            tool_specs: default_tool_specs(),
        }
    }

    pub fn from_env(
        provider: &str,
        model: impl Into<String>,
        api_base: Option<String>,
        api_key_env: Option<String>,
    ) -> anyhow::Result<Self> {
        let env_name = api_key_env.unwrap_or_else(|| match provider {
            "openrouter" => "OPENROUTER_API_KEY".to_string(),
            "ollama" => String::new(),
            _ => std::env::var("OPENAI_COMPATIBLE_API_KEY")
                .map(|_| "OPENAI_COMPATIBLE_API_KEY".to_string())
                .unwrap_or_else(|_| "OPENAI_API_KEY".to_string()),
        });
        let api_key = if env_name.is_empty() {
            None
        } else {
            Some(
                std::env::var(&env_name)
                    .with_context(|| format!("{env_name} is required for provider `{provider}`"))?,
            )
        };
        let base_url = api_base
            .or_else(|| provider_base_url_from_env(provider))
            .unwrap_or_else(|| match provider {
                "openrouter" => "https://openrouter.ai/api".to_string(),
                "ollama" => "http://localhost:11434".to_string(),
                _ => DEFAULT_BASE_URL.to_string(),
            });
        let requested_model = model.into();
        let model = if requested_model.trim().is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            requested_model
        };

        Ok(Self::new(api_key, base_url, model))
    }

    pub fn with_tool_specs(mut self, specs: Vec<ToolSpec>) -> Self {
        self.tool_specs = specs.into_iter().map(tool_spec_to_openai).collect();
        self
    }

    pub fn has_env_credentials(provider: &str) -> bool {
        match provider {
            "ollama" => true,
            "openrouter" => env_present("OPENROUTER_API_KEY"),
            _ => env_present("OPENAI_API_KEY") || env_present("OPENAI_COMPATIBLE_API_KEY"),
        }
    }

    async fn send_request(
        &self,
        request: ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>> {
        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let mut body = json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "stream": true,
            "stream_options": { "include_usage": true },
            "messages": messages_to_openai(&request),
        });
        if !self.tool_specs.is_empty() {
            body["tools"] = json!(self.tool_specs);
        }
        if request.system_context.structured_output.is_some() {
            body["response_format"] = json!({ "type": "json_object" });
        }

        let mut builder = self
            .http
            .post(url)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&body);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI-compatible request failed with {status}: {body}");
        }

        let byte_stream = response
            .bytes_stream()
            .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
            .boxed();

        Ok(Box::pin(stream::unfold(
            OpenAiStreamState {
                byte_stream,
                buffer: String::new(),
                pending: VecDeque::new(),
                tool_calls: BTreeMap::new(),
                finished: false,
            },
            next_stream_event,
        )))
    }
}

#[async_trait]
impl ModelClient for OpenAiCompatibleClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>> {
        self.send_request(request).await
    }
}

fn messages_to_openai(request: &ModelRequest) -> Vec<Value> {
    let mut messages = vec![json!({
        "role": "system",
        "content": build_system_prompt(&request.system_context),
    })];
    messages.extend(request.messages.iter().map(message_to_openai));
    messages
}

fn message_to_openai(message: &AgentMessage) -> Value {
    match message.role {
        Role::User => {
            let mut content = Vec::new();
            let mut tool_results = Vec::new();
            for item in &message.content {
                match item {
                    MessageContent::Text(text) => content.push(text.clone()),
                    MessageContent::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => tool_results.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id,
                        "content": if *is_error {
                            format!("ERROR: {content}")
                        } else {
                            content.clone()
                        },
                    })),
                    MessageContent::ToolUse { .. } => {}
                }
            }
            if let Some(result) = tool_results.into_iter().next() {
                result
            } else {
                json!({ "role": "user", "content": content.join("\n") })
            }
        }
        Role::Assistant => {
            let text = message
                .content
                .iter()
                .filter_map(|item| match item {
                    MessageContent::Text(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            let tool_calls = message
                .content
                .iter()
                .filter_map(|item| match item {
                    MessageContent::ToolUse { id, name, input } => Some(json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": input.to_string(),
                        }
                    })),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if tool_calls.is_empty() {
                json!({ "role": "assistant", "content": text })
            } else {
                json!({ "role": "assistant", "content": text, "tool_calls": tool_calls })
            }
        }
    }
}

struct OpenAiStreamState {
    byte_stream: BoxStream<'static, Result<Vec<u8>, reqwest::Error>>,
    buffer: String,
    pending: VecDeque<ModelStreamEvent>,
    tool_calls: BTreeMap<usize, PartialToolCall>,
    finished: bool,
}

#[derive(Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

async fn next_stream_event(
    mut state: OpenAiStreamState,
) -> Option<(ModelStreamEvent, OpenAiStreamState)> {
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
                finish_tool_calls(&mut state);
                state.pending.push_back(ModelStreamEvent::Done);
                state.finished = true;
                continue;
            }
        };
        state.buffer.push_str(&chunk);
        while let Some(frame_end) = state.buffer.find("\n\n") {
            let frame = state.buffer[..frame_end].to_string();
            state.buffer = state.buffer[frame_end + 2..].to_string();
            parse_frame(&frame, &mut state);
        }
    }
}

fn parse_frame(frame: &str, state: &mut OpenAiStreamState) {
    for line in frame.lines() {
        let Some(data) = line.trim().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            finish_tool_calls(state);
            state.pending.push_back(ModelStreamEvent::Done);
            state.finished = true;
            return;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(usage) = value.get("usage") {
            state.pending.push_back(ModelStreamEvent::Usage(TokenUsage {
                input: usage
                    .get("prompt_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                output: usage
                    .get("completion_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                reasoning: None,
                cache_read: None,
                cache_write: None,
                cost_usd: None,
            }));
        }
        let Some(delta) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"))
        else {
            continue;
        };
        if let Some(content) = delta.get("content").and_then(Value::as_str) {
            state
                .pending
                .push_back(ModelStreamEvent::TextDelta(content.to_string()));
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tool in tool_calls {
                let index = tool.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let partial = state.tool_calls.entry(index).or_default();
                if let Some(id) = tool.get("id").and_then(Value::as_str) {
                    partial.id.push_str(id);
                }
                if let Some(function) = tool.get("function") {
                    if let Some(name) = function.get("name").and_then(Value::as_str) {
                        partial.name.push_str(name);
                    }
                    if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                        partial.arguments.push_str(arguments);
                    }
                }
            }
        }
    }
}

fn finish_tool_calls(state: &mut OpenAiStreamState) {
    let calls = std::mem::take(&mut state.tool_calls);
    for (_, call) in calls {
        if call.id.is_empty() || call.name.is_empty() {
            continue;
        }
        let input = serde_json::from_str(&call.arguments).unwrap_or_else(|_| {
            json!({
                "raw": call.arguments,
            })
        });
        state.pending.push_back(ModelStreamEvent::ToolUse(ToolCall {
            id: call.id,
            name: call.name,
            input,
        }));
    }
}

fn default_tool_specs() -> Vec<Value> {
    native_tools::tool_specs()
        .into_iter()
        .map(tool_spec_to_openai)
        .collect()
}

fn tool_spec_to_openai(spec: ToolSpec) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": spec.name,
            "description": spec.description,
            "parameters": spec.input_schema,
        }
    })
}

fn env_present(name: &str) -> bool {
    std::env::var(name)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn provider_base_url_from_env(provider: &str) -> Option<String> {
    let upper = provider.replace('-', "_").to_ascii_uppercase();
    [
        format!("{upper}_BASE_URL"),
        format!("{upper}_API_BASE"),
        "OPENAI_BASE_URL".to_string(),
        "OPENAI_API_BASE".to_string(),
        "OPENAI_COMPATIBLE_BASE_URL".to_string(),
        "OPENAI_COMPATIBLE_API_BASE".to_string(),
        "CLAURST_API_BASE".to_string(),
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}
