//! Tools Module - File operations, commands, search, etc.

pub mod execute_command;
pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod git_operations;
pub mod glob;
pub mod list_files;
pub mod note_edit;
pub mod search;
pub mod task_management;

pub use execute_command::ExecuteCommandTool;
pub use file_edit::FileEditTool;
pub use file_read::FileReadTool;
pub use file_write::FileWriteTool;
pub use git_operations::GitOperationsTool;
pub use glob::GlobTool;
pub use list_files::ListFilesTool;
pub use note_edit::NoteEditTool;
pub use search::SearchTool;
pub use task_management::TaskManagementTool;

use async_trait::async_trait;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tool trait for all tools
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name
    fn name(&self) -> &str;

    /// Tool description
    fn description(&self) -> &str;

    /// Tool input schema
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool
    async fn execute(&self, input: serde_json::Value) -> Result<ToolOutput, ToolError>;

    /// Validate input before execution. Tools should override this for
    /// cross-field validation that cannot be expressed in JSON schema.
    async fn validate_input(&self, _input: &serde_json::Value) -> Result<(), ToolError> {
        Ok(())
    }

    /// Permission hook. The default allows execution; write/shell tools can
    /// override this once a project permission model is wired in.
    async fn check_permissions(&self, _input: &serde_json::Value) -> Result<(), ToolError> {
        Ok(())
    }

    /// Whether this input can safely run concurrently with other read-only
    /// tools in the same assistant turn.
    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Maximum returned content size. Oversized output is truncated by the
    /// registry so individual tools do not all need their own guard.
    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    /// Short human-readable summary for logs/UI surfaces.
    fn tool_summary(&self, _input: &serde_json::Value) -> Option<String> {
        None
    }

    /// Convert to OpenAI-compatible function definition
    fn tool_definition(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": self.description(),
                "parameters": self.input_schema()
            }
        })
    }
}

/// Tool output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// Output type
    pub output_type: String,
    /// Output content
    pub content: String,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Tool error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolError {
    /// Error message
    pub message: String,
    /// Error code
    pub code: Option<String>,
}

/// Batch tool invocation used by the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub name: String,
    pub input: serde_json::Value,
}

/// Tool registry
pub struct ToolRegistry {
    /// Registered tools
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };

        // Register built-in tools
        registry.register(Box::new(file_read::FileReadTool::new()));
        registry.register(Box::new(file_edit::FileEditTool::new()));
        registry.register(Box::new(file_write::FileWriteTool::new()));
        registry.register(Box::new(execute_command::ExecuteCommandTool::new()));
        registry.register(Box::new(search::SearchTool::new()));
        registry.register(Box::new(glob::GlobTool::new()));
        registry.register(Box::new(list_files::ListFilesTool::new()));
        registry.register(Box::new(git_operations::GitOperationsTool::new()));
        registry.register(Box::new(task_management::TaskManagementTool::new()));
        registry.register(Box::new(note_edit::NoteEditTool::new()));

        registry
    }

    /// Register a tool
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|b| b.as_ref())
    }

    /// List all tools
    pub fn list(&self) -> Vec<&dyn Tool> {
        self.tools.values().map(|b| b.as_ref()).collect()
    }

    /// Execute a tool
    pub async fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        match self.tools.get(name) {
            Some(tool) => self.execute_tool(tool.as_ref(), input).await,
            None => Err(ToolError {
                message: format!("Tool not found: {}", name),
                code: Some("tool_not_found".to_string()),
            }),
        }
    }

    async fn execute_tool(
        &self,
        tool: &dyn Tool,
        input: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        tool.validate_input(&input).await?;
        tool.check_permissions(&input).await?;
        let mut output = tool.execute(input).await?;
        let max_chars = tool.max_result_size_chars();
        if output.content.chars().count() > max_chars {
            output.content = truncate_chars(&output.content, max_chars);
            output
                .metadata
                .insert("truncated".to_string(), serde_json::json!(true));
            output.metadata.insert(
                "max_result_size_chars".to_string(),
                serde_json::json!(max_chars),
            );
        }
        Ok(output)
    }

    /// Execute a batch of tool invocations. Consecutive read-only calls are
    /// executed concurrently; mutating or shell calls act as serial barriers.
    pub async fn execute_batch(
        &self,
        invocations: Vec<ToolInvocation>,
    ) -> Vec<Result<ToolOutput, ToolError>> {
        let mut results = Vec::with_capacity(invocations.len());
        let mut read_only_batch: Vec<ToolInvocation> = Vec::new();

        for invocation in invocations {
            let safe = self
                .tools
                .get(&invocation.name)
                .map(|tool| tool.is_concurrency_safe(&invocation.input))
                .unwrap_or(false);

            if safe {
                read_only_batch.push(invocation);
                continue;
            }

            if !read_only_batch.is_empty() {
                results.extend(
                    self.execute_read_only_batch(std::mem::take(&mut read_only_batch))
                        .await,
                );
            }
            results.push(self.execute(&invocation.name, invocation.input).await);
        }

        if !read_only_batch.is_empty() {
            results.extend(self.execute_read_only_batch(read_only_batch).await);
        }

        results
    }

    async fn execute_read_only_batch(
        &self,
        invocations: Vec<ToolInvocation>,
    ) -> Vec<Result<ToolOutput, ToolError>> {
        join_all(invocations.into_iter().map(|invocation| async move {
            self.execute(&invocation.name, invocation.input).await
        }))
        .await
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let mut out: String = input.chars().take(max_chars).collect();
    out.push_str("\n\n[Tool output truncated]");
    out
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
