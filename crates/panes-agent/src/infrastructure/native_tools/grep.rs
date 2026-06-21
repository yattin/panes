use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use regex::RegexBuilder;

use crate::domain::tools::{ToolCall, ToolResult};

use super::{
    glob::ignored_path, input_path, input_string, tool_error, NativeToolExecutor, WorkspacePath,
};

const DEFAULT_MAX_RESULTS: usize = 200;

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let Some(pattern) = input_string(&call, &["pattern", "query"]) else {
        return Ok(tool_error(call.id, "grep requires input.pattern"));
    };
    if pattern.is_empty() {
        return Ok(tool_error(call.id, "grep input.pattern must not be empty"));
    }
    let path = input_path(&call).unwrap_or(".");
    let path_glob = input_string(&call, &["path_glob", "pathGlob", "glob"]);
    let output_mode = input_string(&call, &["output_mode", "outputMode"]).unwrap_or("content");
    let type_filter = input_string(&call, &["type"]);
    let context = call
        .input
        .get("context")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);
    let line_numbers = call
        .input
        .get("-n")
        .or_else(|| call.input.get("line_numbers"))
        .or_else(|| call.input.get("lineNumbers"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let explicit_case_sensitive = call
        .input
        .get("case_sensitive")
        .or_else(|| call.input.get("caseSensitive"))
        .and_then(serde_json::Value::as_bool);
    let ignore_case = call
        .input
        .get("-i")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let case_sensitive = explicit_case_sensitive.unwrap_or(!ignore_case);
    let multiline = call
        .input
        .get("multiline")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let max_results = call
        .input
        .get("max_results")
        .or_else(|| call.input.get("maxResults"))
        .or_else(|| call.input.get("head_limit"))
        .or_else(|| call.input.get("headLimit"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(DEFAULT_MAX_RESULTS);

    let root = match executor.resolve_existing_workspace_path(".", "grep")? {
        WorkspacePath::Inside(path) => path,
        WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
    };
    let resolved_target = match executor.resolve_existing_workspace_path(path, "grep")? {
        WorkspacePath::Inside(path) => path,
        WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
    };

    let regex = match RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .multi_line(multiline)
        .dot_matches_new_line(multiline)
        .build()
    {
        Ok(regex) => regex,
        Err(error) => return Ok(tool_error(call.id, format!("invalid grep regex: {error}"))),
    };
    let glob_filter = match build_glob_filter(path_glob) {
        Ok(filter) => filter,
        Err(message) => return Ok(tool_error(call.id, message)),
    };
    let mut matches = Vec::new();
    let mut count_by_file = Vec::<(String, usize)>::new();
    let mut files_with_matches = Vec::<String>::new();
    for entry in WalkBuilder::new(resolved_target)
        .hidden(false)
        .parents(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| !ignored_path(entry.path()))
        .build()
        .filter_map(Result::ok)
    {
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let relative_path = entry
            .path()
            .strip_prefix(&root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        if let Some(glob_filter) = &glob_filter {
            if !glob_filter.is_match(&relative_path) {
                continue;
            }
        }
        if !type_matches(type_filter, &relative_path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let mut file_match_count = 0usize;
        for (index, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                file_match_count += 1;
                if output_mode == "content" {
                    let lines = content.lines().collect::<Vec<_>>();
                    let start = index.saturating_sub(context);
                    let end = (index + context + 1).min(lines.len());
                    for (line_index, matched_line) in lines[start..end].iter().enumerate() {
                        let actual = start + line_index + 1;
                        if line_numbers {
                            matches.push(format!("{relative_path}:{actual}:{matched_line}"));
                        } else {
                            matches.push(format!("{relative_path}:{matched_line}"));
                        }
                        if matches.len() >= max_results {
                            break;
                        }
                    }
                }
            }
            if matches.len() >= max_results {
                break;
            }
        }
        if file_match_count > 0 {
            files_with_matches.push(relative_path.clone());
            count_by_file.push((relative_path, file_match_count));
        }
        if matches.len() >= max_results {
            break;
        }
    }

    let content = match output_mode {
        "files_with_matches" => {
            files_with_matches.sort();
            files_with_matches
                .into_iter()
                .take(max_results)
                .collect::<Vec<_>>()
        }
        "count" => {
            count_by_file.sort_by(|left, right| left.0.cmp(&right.0));
            count_by_file
                .into_iter()
                .take(max_results)
                .map(|(path, count)| format!("{path}:{count}"))
                .collect()
        }
        _ => {
            matches.sort();
            matches
        }
    };
    Ok(ToolResult {
        tool_use_id: call.id,
        content: if content.is_empty() {
            String::new()
        } else {
            content.join("\n") + "\n"
        },
        is_error: false,
    })
}

fn build_glob_filter(pattern: Option<&str>) -> Result<Option<globset::GlobSet>, String> {
    let Some(pattern) = pattern else {
        return Ok(None);
    };
    let mut builder = GlobSetBuilder::new();
    let glob = Glob::new(pattern).map_err(|error| format!("invalid grep glob: {error}"))?;
    builder.add(glob);
    builder
        .build()
        .map(Some)
        .map_err(|error| format!("invalid grep glob set: {error}"))
}

fn type_matches(type_filter: Option<&str>, path: &str) -> bool {
    let Some(type_filter) = type_filter else {
        return true;
    };
    let extension = path.rsplit('.').next().unwrap_or("");
    match type_filter {
        "rust" | "rs" => extension == "rs",
        "python" | "py" => extension == "py",
        "javascript" | "js" => extension == "js" || extension == "jsx",
        "typescript" | "ts" => extension == "ts" || extension == "tsx",
        "json" => extension == "json",
        "markdown" | "md" => extension == "md" || extension == "markdown",
        other => extension == other.trim_start_matches('.'),
    }
}
