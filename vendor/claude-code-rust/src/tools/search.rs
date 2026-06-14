//! Search Tool

use super::{Tool, ToolError, ToolOutput};
use async_trait::async_trait;
use serde_json;
use std::collections::HashMap;
use std::path::Path;

pub struct SearchTool;

impl Default for SearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &str {
        "search"
    }

    fn description(&self) -> &str {
        "Search for patterns in files using regex"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to search in"
                },
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "file_pattern": {
                    "type": "string",
                    "description": "File pattern to match (optional)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum matching lines to return"
                }
            },
            "required": ["path", "pattern"]
        })
    }

    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn tool_summary(&self, input: &serde_json::Value) -> Option<String> {
        input["pattern"]
            .as_str()
            .map(|pattern| format!("search {}", pattern))
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolOutput, ToolError> {
        let path = input["path"].as_str().ok_or_else(|| ToolError {
            message: "path is required".to_string(),
            code: Some("missing_parameter".to_string()),
        })?;

        let pattern = input["pattern"].as_str().ok_or_else(|| ToolError {
            message: "pattern is required".to_string(),
            code: Some("missing_parameter".to_string()),
        })?;

        let search_path = Path::new(path);
        let file_pattern = input["file_pattern"].as_str();
        let max_results = input["max_results"].as_u64().unwrap_or(500) as usize;

        if !search_path.exists() {
            return Err(ToolError {
                message: format!("Path does not exist: {}", path),
                code: Some("path_not_found".to_string()),
            });
        }

        if let Some(output) = run_ripgrep(path, pattern, file_pattern, max_results).await {
            return output;
        }

        let regex = regex::Regex::new(pattern).map_err(|e| ToolError {
            message: format!("Invalid regex pattern: {}", e),
            code: Some("invalid_pattern".to_string()),
        })?;
        let glob_pattern = match file_pattern {
            Some(pattern) => Some(glob::Pattern::new(pattern).map_err(|e| ToolError {
                message: format!("Invalid file pattern: {}", e),
                code: Some("invalid_file_pattern".to_string()),
            })?),
            None => None,
        };

        let mut results = Vec::new();

        // Walk the directory
        for entry in walkdir::WalkDir::new(search_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();

            if entry_path.is_file() {
                if let Some(glob_pattern) = &glob_pattern {
                    if !glob_pattern.matches_path(entry_path) {
                        continue;
                    }
                }
                // Try to read and search
                if let Ok(content) = std::fs::read_to_string(entry_path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if regex.is_match(line) {
                            results.push(format!(
                                "{}:{}: {}",
                                entry_path.display(),
                                line_num + 1,
                                line
                            ));
                            if results.len() >= max_results {
                                break;
                            }
                        }
                    }
                }
            }
            if results.len() >= max_results {
                break;
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("count".to_string(), serde_json::json!(results.len()));
        metadata.insert("max_results".to_string(), serde_json::json!(max_results));
        metadata.insert("engine".to_string(), serde_json::json!("walkdir_regex"));

        Ok(ToolOutput {
            output_type: "text".to_string(),
            content: results.join("\n"),
            metadata,
        })
    }
}

async fn run_ripgrep(
    path: &str,
    pattern: &str,
    file_pattern: Option<&str>,
    max_results: usize,
) -> Option<Result<ToolOutput, ToolError>> {
    let rg = which::which("rg").ok()?;
    let mut command = tokio::process::Command::new(rg);
    command.arg("--line-number").arg("--color").arg("never");
    command.arg("--max-count").arg(max_results.to_string());
    if let Some(file_pattern) = file_pattern {
        command.arg("--glob").arg(file_pattern);
    }
    command.arg(pattern).arg(path);

    let output = match command.output().await {
        Ok(output) => output,
        Err(_) => return None,
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && output.status.code() != Some(1) {
        return Some(Err(ToolError {
            message: format!("rg failed: {}", stderr),
            code: Some("search_error".to_string()),
        }));
    }

    let lines: Vec<&str> = stdout.lines().take(max_results).collect();
    let mut metadata = HashMap::new();
    metadata.insert("count".to_string(), serde_json::json!(lines.len()));
    metadata.insert("max_results".to_string(), serde_json::json!(max_results));
    metadata.insert("engine".to_string(), serde_json::json!("rg"));

    Some(Ok(ToolOutput {
        output_type: "text".to_string(),
        content: lines.join("\n"),
        metadata,
    }))
}
