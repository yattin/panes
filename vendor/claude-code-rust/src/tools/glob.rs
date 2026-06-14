//! Glob Tool

use super::{Tool, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json;
use std::collections::HashMap;

pub struct GlobTool;

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files by glob pattern"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern, for example src/**/*.rs"
                }
            },
            "required": ["pattern"]
        })
    }

    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn tool_summary(&self, input: &serde_json::Value) -> Option<String> {
        input["pattern"]
            .as_str()
            .map(|pattern| format!("glob {}", pattern))
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolOutput, ToolError> {
        let pattern = input["pattern"].as_str().ok_or_else(|| ToolError {
            message: "pattern is required".to_string(),
            code: Some("missing_parameter".to_string()),
        })?;

        let mut matches = Vec::new();
        for entry in glob::glob(pattern).map_err(|e| ToolError {
            message: format!("Invalid glob pattern: {}", e),
            code: Some("invalid_pattern".to_string()),
        })? {
            match entry {
                Ok(path) => matches.push(path.display().to_string()),
                Err(e) => {
                    return Err(ToolError {
                        message: format!("Failed to read glob entry: {}", e),
                        code: Some("glob_error".to_string()),
                    });
                }
            }
        }
        matches.sort();

        let mut metadata = HashMap::new();
        metadata.insert("count".to_string(), serde_json::json!(matches.len()));

        Ok(ToolOutput {
            output_type: "text".to_string(),
            content: matches.join("\n"),
            metadata,
        })
    }
}
