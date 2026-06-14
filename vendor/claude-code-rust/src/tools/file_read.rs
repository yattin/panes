//! File Read Tool

use super::{Tool, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

const MAX_READ_BYTES: u64 = 5 * 1024 * 1024;
const NUL_SAMPLE_BYTES: usize = 8192;

pub struct FileReadTool;

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileReadTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "1-based line number to start reading from"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to return"
                }
            },
            "required": ["file_path"]
        })
    }

    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn tool_summary(&self, input: &serde_json::Value) -> Option<String> {
        input["file_path"]
            .as_str()
            .map(|path| format!("read {}", path))
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolOutput, ToolError> {
        let file_path = input["file_path"].as_str().ok_or_else(|| ToolError {
            message: "file_path is required".to_string(),
            code: Some("missing_parameter".to_string()),
        })?;

        let path = Path::new(file_path);

        if !path.exists() {
            return Err(ToolError {
                message: format!("File does not exist: {}", file_path),
                code: Some("file_not_found".to_string()),
            });
        }

        if !path.is_file() {
            return Err(ToolError {
                message: format!("Path is not a file: {}", file_path),
                code: Some("not_file".to_string()),
            });
        }

        let metadata = std::fs::metadata(path).map_err(|e| ToolError {
            message: format!("Failed to inspect file: {}", e),
            code: Some("metadata_error".to_string()),
        })?;

        if metadata.len() > MAX_READ_BYTES && input["limit"].is_null() {
            return Err(ToolError {
                message: format!(
                    "File is too large to read at once ({} bytes). Use offset and limit.",
                    metadata.len()
                ),
                code: Some("file_too_large".to_string()),
            });
        }

        let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = input["limit"].as_u64().map(|v| v as usize);
        let (rendered, total_lines, returned_lines) = if let Some(limit) = limit {
            read_limited_text(path, offset, limit)?
        } else {
            read_full_text(path, offset)?
        };

        let mut result_metadata = HashMap::new();
        result_metadata.insert("file_path".to_string(), serde_json::json!(file_path));
        result_metadata.insert("size_bytes".to_string(), serde_json::json!(metadata.len()));
        result_metadata.insert("total_lines".to_string(), serde_json::json!(total_lines));
        result_metadata.insert("offset".to_string(), serde_json::json!(offset));
        result_metadata.insert(
            "returned_lines".to_string(),
            serde_json::json!(returned_lines),
        );

        Ok(ToolOutput {
            output_type: "text".to_string(),
            content: rendered,
            metadata: result_metadata,
        })
    }
}

fn reject_binary_sample(path: &Path) -> Result<(), ToolError> {
    let mut file = File::open(path).map_err(|e| ToolError {
        message: format!("Failed to read file: {}", e),
        code: Some("read_error".to_string()),
    })?;
    let mut sample = vec![0_u8; NUL_SAMPLE_BYTES];
    let read = file.read(&mut sample).map_err(|e| ToolError {
        message: format!("Failed to read file: {}", e),
        code: Some("read_error".to_string()),
    })?;
    if sample[..read].iter().any(|b| *b == 0) {
        return Err(ToolError {
            message: "File appears to be binary and cannot be read as text".to_string(),
            code: Some("binary_file".to_string()),
        });
    }
    Ok(())
}

fn read_full_text(path: &Path, offset: usize) -> Result<(String, usize, usize), ToolError> {
    let bytes = std::fs::read(path).map_err(|e| ToolError {
        message: format!("Failed to read file: {}", e),
        code: Some("read_error".to_string()),
    })?;

    if bytes.iter().take(NUL_SAMPLE_BYTES).any(|b| *b == 0) {
        return Err(ToolError {
            message: "File appears to be binary and cannot be read as text".to_string(),
            code: Some("binary_file".to_string()),
        });
    }

    let content = String::from_utf8(bytes).map_err(|_| ToolError {
        message: "File is not valid UTF-8 text".to_string(),
        code: Some("invalid_utf8".to_string()),
    })?;
    let all_lines: Vec<&str> = content.lines().collect();
    let start = offset.saturating_sub(1).min(all_lines.len());
    let rendered = render_numbered_lines(all_lines[start..].iter().copied(), start + 1);
    let returned_lines = all_lines.len().saturating_sub(start);
    Ok((rendered, all_lines.len(), returned_lines))
}

fn read_limited_text(
    path: &Path,
    offset: usize,
    limit: usize,
) -> Result<(String, usize, usize), ToolError> {
    reject_binary_sample(path)?;
    let file = File::open(path).map_err(|e| ToolError {
        message: format!("Failed to read file: {}", e),
        code: Some("read_error".to_string()),
    })?;
    let reader = BufReader::new(file);
    let start_line = offset.saturating_sub(1);
    let mut selected = Vec::new();
    let mut total_lines = 0usize;

    for (idx, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| ToolError {
            message: format!("Failed to read file as UTF-8 text: {}", e),
            code: Some("invalid_utf8".to_string()),
        })?;
        total_lines = idx + 1;
        if idx >= start_line && selected.len() < limit {
            selected.push(line);
        }
    }

    let returned_lines = selected.len();
    let rendered = render_numbered_lines(selected.iter().map(String::as_str), start_line + 1);
    Ok((rendered, total_lines, returned_lines))
}

fn render_numbered_lines<'a>(
    lines: impl IntoIterator<Item = &'a str>,
    first_line_number: usize,
) -> String {
    lines
        .into_iter()
        .enumerate()
        .map(|(idx, line)| format!("{:>6}\t{}", first_line_number + idx, line))
        .collect::<Vec<_>>()
        .join("\n")
}
