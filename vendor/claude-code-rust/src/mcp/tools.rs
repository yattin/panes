//! MCP Tools - Tool registration and execution

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub server_name: Option<String>,
}

impl McpTool {
    pub fn new(name: &str, description: &str, input_schema: serde_json::Value) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
            server_name: None,
        }
    }

    pub fn with_server(mut self, server_name: &str) -> Self {
        self.server_name = Some(server_name.to_string());
        self
    }
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, params: serde_json::Value) -> anyhow::Result<serde_json::Value>;
}

pub type ToolExecutorFn = Box<
    dyn Fn(
            serde_json::Value,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = anyhow::Result<serde_json::Value>> + Send>,
        > + Send
        + Sync,
>;

pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, McpTool>>>,
    executors: Arc<RwLock<HashMap<String, Arc<dyn ToolExecutor>>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            executors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, tool: McpTool, executor: Arc<dyn ToolExecutor>) {
        let name = tool.name.clone();
        let mut tools = self.tools.write().await;
        let mut executors = self.executors.write().await;
        tools.insert(name.clone(), tool);
        executors.insert(name, executor);
    }

    pub async fn unregister(&self, name: &str) {
        let mut tools = self.tools.write().await;
        let mut executors = self.executors.write().await;
        tools.remove(name);
        executors.remove(name);
    }

    pub async fn get(&self, name: &str) -> Option<McpTool> {
        let tools = self.tools.read().await;
        tools.get(name).cloned()
    }

    pub async fn list(&self) -> Vec<McpTool> {
        let tools = self.tools.read().await;
        tools.values().cloned().collect()
    }

    pub async fn execute(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let executors = self.executors.read().await;
        let executor = executors
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", name))?;
        executor.execute(params).await
    }

    pub async fn register_builtin_tools(&self) {
        let core_registry = crate::tools::ToolRegistry::new();
        for tool in core_registry.list() {
            self.register(
                McpTool::new(tool.name(), tool.description(), tool.input_schema()),
                Arc::new(CoreToolExecutor {
                    tool_name: tool.name().to_string(),
                }),
            )
            .await;
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

struct CoreToolExecutor {
    tool_name: String,
}

#[async_trait]
impl ToolExecutor for CoreToolExecutor {
    async fn execute(&self, params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let registry = crate::tools::ToolRegistry::new();
        let output = registry
            .execute(
                &self.tool_name,
                normalize_core_tool_params(&self.tool_name, params),
            )
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "{}: {}",
                    e.code.unwrap_or_else(|| "tool_error".to_string()),
                    e.message
                )
            })?;
        Ok(serde_json::json!({
            "type": output.output_type,
            "content": output.content,
            "metadata": output.metadata
        }))
    }
}

fn normalize_core_tool_params(tool_name: &str, mut params: serde_json::Value) -> serde_json::Value {
    if matches!(tool_name, "file_read" | "file_write" | "file_edit") {
        if let Some(path) = params.get("path").cloned() {
            if params.get("file_path").is_none() {
                params["file_path"] = path;
            }
        }
    }
    params
}
