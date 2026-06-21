use std::sync::{Arc, Mutex};

use anyhow::Context;
use async_trait::async_trait;
use futures::{stream, stream::BoxStream};
use tokio_util::sync::CancellationToken;

use crate::{
    application::ports::{EventSink, ModelClient, ToolExecutor},
    domain::tools::{ToolCall, ToolResult},
    interfaces::{AgentEvent, ModelRequest, ModelStreamEvent},
};

pub struct StaticModelClient {
    events: Vec<ModelStreamEvent>,
}

impl StaticModelClient {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            events: vec![
                ModelStreamEvent::TextDelta(text.into()),
                ModelStreamEvent::Done,
            ],
        }
    }
}

#[async_trait]
impl ModelClient for StaticModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>> {
        Ok(Box::pin(stream::iter(self.events.clone())))
    }
}

#[derive(Clone, Default)]
pub struct ScriptedModelClient {
    responses: Arc<Mutex<Vec<Vec<ModelStreamEvent>>>>,
    requests: Arc<Mutex<Vec<ModelRequest>>>,
}

impl ScriptedModelClient {
    pub fn new(responses: Vec<Vec<ModelStreamEvent>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().rev().collect())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn requests(&self) -> Vec<ModelRequest> {
        self.requests
            .lock()
            .expect("model request log poisoned")
            .clone()
    }
}

#[async_trait]
impl ModelClient for ScriptedModelClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>> {
        self.requests
            .lock()
            .map_err(|_| anyhow::anyhow!("model request log poisoned"))?
            .push(request);
        let events = self
            .responses
            .lock()
            .map_err(|_| anyhow::anyhow!("scripted model response queue poisoned"))?
            .pop()
            .context("scripted model response queue exhausted")?;
        Ok(Box::pin(stream::iter(events)))
    }
}

#[derive(Clone)]
pub struct StaticToolExecutor {
    output: String,
}

impl StaticToolExecutor {
    pub fn text(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
        }
    }
}

#[async_trait]
impl ToolExecutor for StaticToolExecutor {
    async fn execute(
        &self,
        call: ToolCall,
        _cancellation: &CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            tool_use_id: call.id,
            content: self.output.clone(),
            is_error: false,
        })
    }
}

#[derive(Default, Clone)]
pub struct RecordingEventSink {
    events: Arc<Mutex<Vec<AgentEvent>>>,
}

impl RecordingEventSink {
    pub fn events(&self) -> Vec<AgentEvent> {
        self.events.lock().expect("event sink poisoned").clone()
    }
}

#[async_trait]
impl EventSink for RecordingEventSink {
    async fn emit(&self, event: AgentEvent) -> anyhow::Result<()> {
        self.events
            .lock()
            .map_err(|_| anyhow::anyhow!("event sink poisoned"))
            .context("failed to record event")?
            .push(event);
        Ok(())
    }
}
