//! File Write Tool

use super::{Tool, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json;
use std::collections::HashMap;
use std::path::Path;

pub struct FileWriteTool;

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWriteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write content to a new file or overwrite an existing file"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                },
                "append": {
                    "type": "boolean",
                    "description": "Append instead of overwriting"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    fn tool_summary(&self, input: &serde_json::Value) -> Option<String> {
        input["file_path"]
            .as_str()
            .map(|path| format!("write {}", path))
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolOutput, ToolError> {
        let file_path = input["file_path"].as_str().ok_or_else(|| ToolError {
            message: "file_path is required".to_string(),
            code: Some("missing_parameter".to_string()),
        })?;

        let content = input["content"].as_str().ok_or_else(|| ToolError {
            message: "content is required".to_string(),
            code: Some("missing_parameter".to_string()),
        })?;

        let path = Path::new(file_path);
        let append = input["append"].as_bool().unwrap_or(false);

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError {
                message: format!("Failed to create directory: {}", e),
                code: Some("directory_error".to_string()),
            })?;
        }

        if append {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| ToolError {
                    message: format!("Failed to open file for append: {}", e),
                    code: Some("write_error".to_string()),
                })?;
            file.write_all(content.as_bytes()).map_err(|e| ToolError {
                message: format!("Failed to append file: {}", e),
                code: Some("write_error".to_string()),
            })?;
        } else {
            std::fs::write(path, content).map_err(|e| ToolError {
                message: format!("Failed to write file: {}", e),
                code: Some("write_error".to_string()),
            })?;
        }

        let mut metadata = HashMap::new();
        metadata.insert("file_path".to_string(), serde_json::json!(file_path));
        metadata.insert(
            "bytes_written".to_string(),
            serde_json::json!(content.len()),
        );
        metadata.insert("append".to_string(), serde_json::json!(append));

        Ok(ToolOutput {
            output_type: "text".to_string(),
            content: if append {
                format!(
                    "Successfully appended {} bytes to {}",
                    content.len(),
                    file_path
                )
            } else {
                format!(
                    "Successfully wrote {} bytes to {}",
                    content.len(),
                    file_path
                )
            },
            metadata,
        })
    }
}
