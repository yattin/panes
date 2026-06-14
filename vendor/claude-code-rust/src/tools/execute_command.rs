//! Execute Command Tool

use super::{Tool, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json;
use std::collections::HashMap;

pub struct ExecuteCommandTool;

impl Default for ExecuteCommandTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecuteCommandTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ExecuteCommandTool {
    fn name(&self) -> &str {
        "execute_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (optional)"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (optional)"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory for the command"
                }
            },
            "required": ["command"]
        })
    }

    fn tool_summary(&self, input: &serde_json::Value) -> Option<String> {
        input["command"]
            .as_str()
            .map(|command| format!("run {}", command))
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolOutput, ToolError> {
        let command = input["command"].as_str().ok_or_else(|| ToolError {
            message: "command is required".to_string(),
            code: Some("missing_parameter".to_string()),
        })?;

        let timeout_ms = input["timeout_ms"]
            .as_u64()
            .or_else(|| {
                input["timeout"]
                    .as_u64()
                    .map(|seconds| seconds.saturating_mul(1000))
            })
            .unwrap_or(60_000);
        let cwd = input["cwd"].as_str();

        let mut process = if cfg!(target_os = "windows") {
            let mut cmd = tokio::process::Command::new("cmd");
            cmd.arg("/C").arg(command);
            cmd
        } else {
            let mut cmd = tokio::process::Command::new("sh");
            cmd.arg("-c").arg(command);
            cmd
        };

        if let Some(cwd) = cwd {
            process.current_dir(cwd);
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            process.output(),
        )
        .await;

        match output {
            Ok(Ok(result)) => {
                let stdout = String::from_utf8_lossy(&result.stdout).to_string();
                let stderr = String::from_utf8_lossy(&result.stderr).to_string();

                let mut metadata = HashMap::new();
                metadata.insert(
                    "success".to_string(),
                    serde_json::json!(result.status.success()),
                );
                metadata.insert(
                    "exit_code".to_string(),
                    serde_json::json!(result.status.code()),
                );
                metadata.insert(
                    "stdout_bytes".to_string(),
                    serde_json::json!(result.stdout.len()),
                );
                metadata.insert(
                    "stderr_bytes".to_string(),
                    serde_json::json!(result.stderr.len()),
                );
                metadata.insert("timeout_ms".to_string(), serde_json::json!(timeout_ms));
                if let Some(cwd) = cwd {
                    metadata.insert("cwd".to_string(), serde_json::json!(cwd));
                }

                let content = serde_json::json!({
                    "success": result.status.success(),
                    "exit_code": result.status.code(),
                    "stdout": stdout,
                    "stderr": stderr
                })
                .to_string();

                Ok(ToolOutput {
                    output_type: "json".to_string(),
                    content,
                    metadata,
                })
            }
            Ok(Err(e)) => Err(ToolError {
                message: format!("Failed to execute command: {}", e),
                code: Some("execution_error".to_string()),
            }),
            Err(_) => Err(ToolError {
                message: format!("Command timed out after {} ms", timeout_ms),
                code: Some("timeout".to_string()),
            }),
        }
    }
}
