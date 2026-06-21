use std::{
    collections::{HashMap, VecDeque},
    mem,
};

use anyhow::Context;
use async_trait::async_trait;
use futures::{stream, stream::BoxStream, StreamExt};
use serde_json::{json, Value};

mod pricing;
mod retry;

use pricing::estimate_anthropic_usage_cost_usd;
use retry::{stream_error_message, AnthropicRequestError, RetryPolicy};

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

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_MODEL: &str = "claude-sonnet-4-6";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

#[derive(Debug, Clone)]
pub struct AnthropicMessagesClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
    max_tokens: u32,
    tool_specs: Vec<Value>,
    retry_policy: RetryPolicy,
}

impl AnthropicMessagesClient {
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
            tool_specs: default_tool_specs(),
            retry_policy: RetryPolicy::default(),
        }
    }

    pub fn with_tool_specs(mut self, tool_specs: Vec<Value>) -> Self {
        self.tool_specs = tool_specs;
        self
    }

    pub fn default_tool_specs() -> Vec<Value> {
        default_tool_specs()
    }

    pub fn from_env(model: impl Into<String>) -> anyhow::Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY is required for claurst-native")?;
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .or_else(|_| std::env::var("ANTHROPIC_API_BASE"))
            .or_else(|_| std::env::var("CLAURST_API_BASE"))
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let requested_model = model.into();
        let model = if requested_model == "anthropic-default" || requested_model.trim().is_empty() {
            std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
        } else {
            requested_model
        };

        Ok(Self::new(api_key, base_url, model))
    }

    pub fn default_model() -> &'static str {
        DEFAULT_MODEL
    }

    pub fn has_env_credentials() -> bool {
        std::env::var("ANTHROPIC_API_KEY")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    }

    async fn send_request(
        &self,
        request: ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>> {
        let mut failed_attempts = 0;
        loop {
            match self.send_request_once(request.clone()).await {
                Ok(stream) => return Ok(stream),
                Err(error) => {
                    failed_attempts += 1;
                    let Some(delay) = self.retry_policy.retry_delay(&error, failed_attempts) else {
                        return Err(error.into());
                    };
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn send_request_once(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, ModelStreamEvent>, AnthropicRequestError> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let mut body = json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "stream": true,
            "system": build_system_prompt(&request.system_context),
            "messages": request.messages.iter().map(message_to_json).collect::<Vec<_>>(),
        });
        if !self.tool_specs.is_empty() {
            body["tools"] = json!(self.tool_specs);
        }

        let response = self
            .http
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&body)
            .send()
            .await
            .map_err(AnthropicRequestError::from)?;

        let status = response.status();
        if !status.is_success() {
            let headers = response.headers().clone();
            let body = response.text().await.unwrap_or_default();
            return Err(AnthropicRequestError::http(status, &headers, body));
        }

        let byte_stream = response
            .bytes_stream()
            .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
            .boxed();

        Ok(Box::pin(stream::unfold(
            SseStreamState {
                byte_stream,
                parser: SseParser::for_model(self.model.clone()),
                pending: VecDeque::new(),
                finished: false,
            },
            next_stream_event,
        )))
    }
}

#[async_trait]
impl ModelClient for AnthropicMessagesClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>> {
        self.send_request(request).await
    }
}

fn message_to_json(message: &AgentMessage) -> Value {
    json!({
        "role": match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        },
        "content": message.content.iter().map(content_to_json).collect::<Vec<_>>(),
    })
}

fn content_to_json(content: &MessageContent) -> Value {
    match content {
        MessageContent::Text(text) => json!({
            "type": "text",
            "text": text,
        }),
        MessageContent::ToolUse { id, name, input } => json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }),
        MessageContent::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        }),
    }
}

struct SseStreamState {
    byte_stream: BoxStream<'static, Result<Vec<u8>, reqwest::Error>>,
    parser: SseParser,
    pending: VecDeque<ModelStreamEvent>,
    finished: bool,
}

async fn next_stream_event(
    mut state: SseStreamState,
) -> Option<(ModelStreamEvent, SseStreamState)> {
    loop {
        if let Some(event) = state.pending.pop_front() {
            return Some((event, state));
        }
        if state.finished {
            return None;
        }

        match state.byte_stream.next().await {
            Some(Ok(bytes)) => {
                let chunk = String::from_utf8_lossy(&bytes);
                if let Err(error) = state.parser.push_chunk(&chunk, &mut state.pending) {
                    state
                        .pending
                        .push_back(ModelStreamEvent::Error(error.to_string()));
                    state.parser.finish(&mut state.pending);
                    state.finished = true;
                }
            }
            Some(Err(error)) => {
                state.pending.push_back(ModelStreamEvent::Error(format!(
                    "failed to read Anthropic SSE stream: {error}"
                )));
                state.parser.finish(&mut state.pending);
                state.finished = true;
            }
            None => {
                state.parser.finish(&mut state.pending);
                state.finished = true;
            }
        }
    }
}

#[derive(Default)]
struct SseParser {
    buffer: String,
    tool_uses: HashMap<u64, ToolUseBuilder>,
    emitted_done: bool,
    model: String,
}

impl SseParser {
    fn for_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Self::default()
        }
    }

    fn push_chunk(
        &mut self,
        chunk: &str,
        events: &mut VecDeque<ModelStreamEvent>,
    ) -> anyhow::Result<()> {
        self.buffer.push_str(chunk);
        while let Some(frame_end) = self.buffer.find("\n\n") {
            let frame = self.buffer[..frame_end].to_string();
            self.buffer.drain(..frame_end + 2);
            self.parse_frame(&frame, events)?;
        }
        Ok(())
    }

    fn finish(&mut self, events: &mut VecDeque<ModelStreamEvent>) {
        if !self.buffer.trim().is_empty() {
            let frame = mem::take(&mut self.buffer);
            if let Err(error) = self.parse_frame(&frame, events) {
                events.push_back(ModelStreamEvent::Error(error.to_string()));
            }
        }
        self.emit_done(events);
    }

    fn parse_frame(
        &mut self,
        frame: &str,
        events: &mut VecDeque<ModelStreamEvent>,
    ) -> anyhow::Result<()> {
        let mut data_lines = Vec::new();
        for line in frame.lines() {
            if let Some(data) = line.strip_prefix("data:") {
                data_lines.push(data.trim_start());
            }
        }
        if data_lines.is_empty() {
            return Ok(());
        }

        let data = data_lines.join("\n");
        if data == "[DONE]" {
            self.emit_done(events);
            return Ok(());
        }

        let value: Value = serde_json::from_str(&data)
            .with_context(|| format!("failed to parse Anthropic SSE event: {data}"))?;
        match value.get("type").and_then(Value::as_str) {
            Some("message_start") => {
                if let Some(usage) = usage_from_message_start(&value, &self.model) {
                    events.push_back(ModelStreamEvent::Usage(usage));
                }
            }
            Some("content_block_start") => {
                if let Some(builder) = tool_use_from_start_event(&value) {
                    self.tool_uses.insert(builder.index, builder);
                }
            }
            Some("content_block_delta") => self.parse_delta(&value, events),
            Some("message_delta") => {
                if let Some(usage) = usage_from_message_delta(&value, &self.model) {
                    events.push_back(ModelStreamEvent::Usage(usage));
                }
            }
            Some("content_block_stop") => {
                if let Some(index) = value.get("index").and_then(Value::as_u64) {
                    if let Some(builder) = self.tool_uses.remove(&index) {
                        events.push_back(ModelStreamEvent::ToolUse(builder.finish()?));
                    }
                }
            }
            Some("message_stop") => self.emit_done(events),
            Some("error") => anyhow::bail!(stream_error_message(&value)),
            _ => {}
        }
        Ok(())
    }

    fn parse_delta(&mut self, value: &Value, events: &mut VecDeque<ModelStreamEvent>) {
        let Some(delta_type) = value
            .get("delta")
            .and_then(|delta| delta.get("type"))
            .and_then(Value::as_str)
        else {
            return;
        };

        match delta_type {
            "text_delta" => {
                if let Some(text) = value
                    .get("delta")
                    .and_then(|delta| delta.get("text"))
                    .and_then(Value::as_str)
                {
                    events.push_back(ModelStreamEvent::TextDelta(text.to_string()));
                }
            }
            "thinking_delta" => {
                if let Some(thinking) = value
                    .get("delta")
                    .and_then(|delta| delta.get("thinking"))
                    .and_then(Value::as_str)
                {
                    events.push_back(ModelStreamEvent::ThinkingDelta(thinking.to_string()));
                }
            }
            "input_json_delta" => {
                if let Some(index) = value.get("index").and_then(Value::as_u64) {
                    if let Some(builder) = self.tool_uses.get_mut(&index) {
                        if let Some(partial) = value
                            .get("delta")
                            .and_then(|delta| delta.get("partial_json"))
                            .and_then(Value::as_str)
                        {
                            builder.partial_json.push_str(partial);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn emit_done(&mut self, events: &mut VecDeque<ModelStreamEvent>) {
        if !self.emitted_done {
            events.push_back(ModelStreamEvent::Done);
            self.emitted_done = true;
        }
    }
}

fn usage_from_message_start(value: &Value, model: &str) -> Option<TokenUsage> {
    value
        .get("message")
        .and_then(|message| message.get("usage"))
        .and_then(|usage| usage_from_value(usage, model))
}

fn usage_from_message_delta(value: &Value, model: &str) -> Option<TokenUsage> {
    value
        .get("usage")
        .and_then(|usage| usage_from_value(usage, model))
}

fn usage_from_value(value: &Value, model: &str) -> Option<TokenUsage> {
    let mut usage = TokenUsage {
        input: value
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output: value
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        reasoning: value.get("thinking_tokens").and_then(Value::as_u64),
        cache_read: value.get("cache_read_input_tokens").and_then(Value::as_u64),
        cache_write: value
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64),
        cost_usd: None,
    };
    usage.cost_usd = estimate_anthropic_usage_cost_usd(model, &usage);
    (!usage.is_empty()).then_some(usage)
}

#[derive(Debug)]
struct ToolUseBuilder {
    index: u64,
    id: String,
    name: String,
    partial_json: String,
}

impl ToolUseBuilder {
    fn finish(self) -> anyhow::Result<ToolCall> {
        let input = if self.partial_json.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&self.partial_json).with_context(|| {
                format!(
                    "failed to parse tool input JSON for Anthropic tool_use {}",
                    self.id
                )
            })?
        };

        Ok(ToolCall {
            id: self.id,
            name: self.name,
            input,
        })
    }
}

fn tool_use_from_start_event(value: &Value) -> Option<ToolUseBuilder> {
    let index = value.get("index")?.as_u64()?;
    let block = value.get("content_block")?;
    if block.get("type")?.as_str()? != "tool_use" {
        return None;
    }

    let partial_json = block
        .get("input")
        .filter(|input| !input.is_null())
        .filter(|input| {
            input
                .as_object()
                .map(|object| !object.is_empty())
                .unwrap_or(true)
        })
        .map(Value::to_string)
        .unwrap_or_default();

    Some(ToolUseBuilder {
        index,
        id: block.get("id")?.as_str()?.to_string(),
        name: block.get("name")?.as_str()?.to_string(),
        partial_json,
    })
}

fn default_tool_specs() -> Vec<Value> {
    native_tools::tool_specs()
        .into_iter()
        .map(tool_spec_to_anthropic)
        .collect()
}

fn tool_spec_to_anthropic(spec: ToolSpec) -> Value {
    json!({
        "name": spec.name,
        "description": spec.description,
        "input_schema": spec.input_schema,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };

    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    #[test]
    fn sse_parser_emits_usage_and_thinking_delta() {
        let mut parser = SseParser::for_model(DEFAULT_MODEL);
        let mut events = VecDeque::new();

        parser
            .push_chunk(
                concat!(
                    "event: message_start\n",
                    "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":12,\"cache_read_input_tokens\":3,\"cache_creation_input_tokens\":4}}}\n\n",
                    "event: content_block_delta\n",
                    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"checking\"}}\n\n",
                    "event: message_delta\n",
                    "data: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":7,\"thinking_tokens\":2}}\n\n",
                    "event: message_stop\n",
                    "data: {\"type\":\"message_stop\"}\n\n",
                ),
                &mut events,
            )
            .expect("SSE frames should parse");

        assert_eq!(
            events.into_iter().collect::<Vec<_>>(),
            vec![
                ModelStreamEvent::Usage(TokenUsage {
                    input: 12,
                    output: 0,
                    reasoning: None,
                    cache_read: Some(3),
                    cache_write: Some(4),
                    cost_usd: Some(0.0000519),
                }),
                ModelStreamEvent::ThinkingDelta("checking".to_string()),
                ModelStreamEvent::Usage(TokenUsage {
                    input: 0,
                    output: 7,
                    reasoning: Some(2),
                    cache_read: None,
                    cache_write: None,
                    cost_usd: Some(0.000105),
                }),
                ModelStreamEvent::Done,
            ]
        );
    }

    #[test]
    fn sse_parser_reassembles_split_tool_input_json() {
        let mut parser = SseParser::for_model(DEFAULT_MODEL);
        let mut events = VecDeque::new();

        parser
            .push_chunk(
                concat!(
                    "event: content_block_start\n",
                    "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tool-1\",\"name\":\"file_read\",\"input\":{}}}\n\n",
                    "event: content_block_delta\n",
                    "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\n\n",
                ),
                &mut events,
            )
            .expect("initial tool frames should parse");
        parser
            .push_chunk(
                concat!(
                    "event: content_block_delta\n",
                    "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"README.md\\\"}\"}}\n\n",
                    "event: content_block_stop\n",
                    "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
                ),
                &mut events,
            )
            .expect("final tool frames should parse");

        assert_eq!(
            events.into_iter().collect::<Vec<_>>(),
            vec![ModelStreamEvent::ToolUse(ToolCall {
                id: "tool-1".to_string(),
                name: "file_read".to_string(),
                input: json!({ "path": "README.md" }),
            })]
        );
    }

    #[test]
    fn sse_parser_reports_stream_error_and_finishes_once() {
        let mut parser = SseParser::for_model(DEFAULT_MODEL);
        let mut events = VecDeque::new();

        let error = parser
            .push_chunk(
                concat!(
                    "event: error\n",
                    "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"try again\"}}\n\n",
                ),
                &mut events,
            )
            .expect_err("Anthropic error event should surface");
        assert!(error.to_string().contains("overloaded_error"));

        parser.finish(&mut events);
        parser.finish(&mut events);

        assert_eq!(
            events.into_iter().collect::<Vec<_>>(),
            vec![ModelStreamEvent::Done]
        );
    }

    #[test]
    fn sse_parser_replays_recorded_tool_use_fixture() {
        let mut parser = SseParser::for_model(DEFAULT_MODEL);
        let mut events = VecDeque::new();

        parser
            .push_chunk(
                include_str!("anthropic/fixtures/messages_tool_use_recording.sse"),
                &mut events,
            )
            .expect("recorded SSE fixture should parse");
        parser.finish(&mut events);

        assert_eq!(
            events.into_iter().collect::<Vec<_>>(),
            vec![
                ModelStreamEvent::Usage(TokenUsage {
                    input: 21,
                    output: 0,
                    reasoning: None,
                    cache_read: Some(4),
                    cache_write: None,
                    cost_usd: Some(0.0000642),
                }),
                ModelStreamEvent::TextDelta("Here is ".to_string()),
                ModelStreamEvent::ThinkingDelta("planning".to_string()),
                ModelStreamEvent::ToolUse(ToolCall {
                    id: "toolu_recording_01".to_string(),
                    name: "search".to_string(),
                    input: json!({
                        "query": "scene",
                        "path": "scripts",
                    }),
                }),
                ModelStreamEvent::Usage(TokenUsage {
                    input: 0,
                    output: 9,
                    reasoning: Some(3),
                    cache_read: None,
                    cache_write: None,
                    cost_usd: Some(0.000135),
                }),
                ModelStreamEvent::Done,
            ]
        );
    }

    #[tokio::test]
    async fn client_retries_retryable_http_response_and_streams_success() {
        let (base_url, attempts) = spawn_retrying_anthropic_server().await;
        let mut client = AnthropicMessagesClient::new("test-key", base_url, DEFAULT_MODEL);
        client.retry_policy = RetryPolicy::new(2, Duration::ZERO, Duration::ZERO);
        client.tool_specs = Vec::new();

        let mut stream = client
            .stream(ModelRequest {
                conversation_id: "conversation-1".to_string(),
                messages: vec![AgentMessage {
                    role: Role::User,
                    content: vec![MessageContent::Text("hello".to_string())],
                }],
                system_context: crate::interfaces::SystemContext::new(None),
            })
            .await
            .expect("retry should eventually open a stream");

        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        assert_eq!(
            attempts.load(Ordering::SeqCst),
            2,
            "client should send one retry after the 429"
        );
        assert_eq!(
            events,
            vec![
                ModelStreamEvent::TextDelta("ok".to_string()),
                ModelStreamEvent::Done,
            ]
        );
    }

    #[tokio::test]
    async fn client_sends_assembled_system_prompt() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let addr = listener.local_addr().expect("test server should have addr");
        let (body_tx, body_rx) = oneshot::channel();

        tokio::spawn(async move {
            let (mut socket, _) = listener
                .accept()
                .await
                .expect("test server should accept request");
            let request = read_http_request(&mut socket).await;
            let body = request_body_json(&request);
            let _ = body_tx.send(body);
            write_http_response(
                &mut socket,
                "200 OK",
                "text/event-stream",
                None,
                concat!(
                    "event: message_stop\n",
                    "data: {\"type\":\"message_stop\"}\n\n",
                ),
            )
            .await;
        });

        let mut client =
            AnthropicMessagesClient::new("test-key", format!("http://{addr}"), DEFAULT_MODEL);
        client.tool_specs = Vec::new();

        let mut stream = client
            .stream(ModelRequest {
                conversation_id: "conversation-system".to_string(),
                messages: vec![AgentMessage::user("hello")],
                system_context: {
                    let mut context =
                        crate::interfaces::SystemContext::new(Some("C:/codes/panes".to_string()));
                    context.append_system_prompt = Some("CueLight business appendix.".to_string());
                    context
                },
            })
            .await
            .expect("request should open a stream");

        while stream.next().await.is_some() {}

        let body = body_rx.await.expect("server should send request body");
        let system = body["system"]
            .as_str()
            .expect("request body should include string system prompt");
        assert!(system.contains("运行在 Panes 内的 native agent"));
        assert!(system.contains("通用软件/项目执行 agent"));
        assert!(system.contains("Working directory: C:/codes/panes"));
        assert!(system.contains("Business appendix:"));
        assert!(system.contains("CueLight business appendix."));
    }

    #[tokio::test]
    #[ignore = "requires ANTHROPIC_API_KEY and ANTHROPIC_RECORDING_OUT; records a live Anthropic SSE fixture"]
    async fn record_anthropic_messages_sse_fixture_from_live_api() -> anyhow::Result<()> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY is required to record a live fixture")?;
        let output_path =
            std::path::PathBuf::from(std::env::var("ANTHROPIC_RECORDING_OUT").context(
                "ANTHROPIC_RECORDING_OUT is required and should point at the fixture to write",
            )?);
        let base_url =
            std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let model = std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create fixture directory {parent:?}"))?;
        }

        let response = reqwest::Client::new()
            .post(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&json!({
                "model": model,
                "max_tokens": 128,
                "stream": true,
                "messages": [{
                    "role": "user",
                    "content": "Say `fixture ok`, then call the search tool for query scene in path scripts."
                }],
                "tools": [{
                    "name": "search",
                    "description": "Search text in a workspace path.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string" },
                            "path": { "type": "string" }
                        },
                        "required": ["query"]
                    }
                }],
                "tool_choice": {
                    "type": "tool",
                    "name": "search"
                }
            }))
            .send()
            .await
            .context("failed to send live Anthropic recording request")?;

        let status = response.status();
        if !status.is_success() {
            anyhow::bail!(
                "live Anthropic recording request failed with {status}: {}",
                response.text().await.unwrap_or_default()
            );
        }

        let mut output = tokio::fs::File::create(&output_path)
            .await
            .with_context(|| format!("failed to create fixture file {output_path:?}"))?;
        let mut chunks = response.bytes_stream();
        while let Some(chunk) = chunks.next().await {
            output
                .write_all(&chunk.context("failed to read live Anthropic SSE chunk")?)
                .await
                .with_context(|| format!("failed to write fixture file {output_path:?}"))?;
        }
        output.flush().await?;

        Ok(())
    }

    async fn spawn_retrying_anthropic_server() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let addr = listener.local_addr().expect("test server should have addr");
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_task = attempts.clone();

        tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener
                    .accept()
                    .await
                    .expect("test server should accept request");
                read_http_request(&mut socket).await;
                let attempt = attempts_for_task.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt == 1 {
                    write_http_response(
                        &mut socket,
                        "429 Too Many Requests",
                        "application/json",
                        Some("Retry-After: 0\r\n"),
                        r#"{"error":{"type":"rate_limit_error","message":"slow down"}}"#,
                    )
                    .await;
                } else {
                    write_http_response(
                        &mut socket,
                        "200 OK",
                        "text/event-stream",
                        None,
                        concat!(
                            "event: content_block_delta\n",
                            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n",
                            "event: message_stop\n",
                            "data: {\"type\":\"message_stop\"}\n\n",
                        ),
                    )
                    .await;
                }
            }
        });

        (format!("http://{addr}"), attempts)
    }

    async fn read_http_request(socket: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            let bytes_read = socket
                .read(&mut buffer)
                .await
                .expect("test server should read request");
            if bytes_read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..bytes_read]);

            if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let content_length = content_length_from_headers(&request[..header_end]);
                let required_len = header_end + 4 + content_length;
                if request.len() >= required_len {
                    break;
                }
            }
        }
        request
    }

    fn request_body_json(request: &[u8]) -> Value {
        let header_end = request
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("request should contain header boundary");
        serde_json::from_slice(&request[header_end + 4..]).expect("request body should be JSON")
    }

    fn content_length_from_headers(headers: &[u8]) -> usize {
        String::from_utf8_lossy(headers)
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0)
    }

    async fn write_http_response(
        socket: &mut tokio::net::TcpStream,
        status: &str,
        content_type: &str,
        extra_headers: Option<&str>,
        body: &str,
    ) {
        let headers = extra_headers.unwrap_or_default();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("test server should write response");
    }
}
