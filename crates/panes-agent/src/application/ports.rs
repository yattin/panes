use async_trait::async_trait;
use futures::stream::BoxStream;
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{
        permission::{PermissionDecision, PermissionRequest},
        tools::{ToolCall, ToolResult},
    },
    interfaces::{AgentEvent, ModelRequest, ModelStreamEvent},
};

#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>>;
}

#[async_trait]
pub trait EventSink: Send + Sync {
    async fn emit(&self, event: AgentEvent) -> anyhow::Result<()>;
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(
        &self,
        call: ToolCall,
        cancellation: &CancellationToken,
    ) -> anyhow::Result<ToolResult>;
}

#[async_trait]
pub trait PermissionGateway: Send + Sync {
    async fn request(&self, request: PermissionRequest) -> anyhow::Result<PermissionDecision>;
}
