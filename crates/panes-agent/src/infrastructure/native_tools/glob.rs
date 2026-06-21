use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;

use crate::domain::tools::{ToolCall, ToolResult};

use super::{input_string, tool_error, NativeToolExecutor, WorkspacePath};

const DEFAULT_MAX_RESULTS: usize = 200;

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let Some(pattern) = input_string(&call, &["pattern", "glob"]) else {
        return Ok(tool_error(call.id, "glob requires input.pattern"));
    };
    let base_path = input_string(&call, &["path"]).unwrap_or(".");
    let max_results = call
        .input
        .get("max_results")
        .or_else(|| call.input.get("maxResults"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(DEFAULT_MAX_RESULTS);

    let root = match executor.resolve_existing_workspace_path(".", "glob")? {
        WorkspacePath::Inside(path) => path,
        WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
    };
    let start = match executor.resolve_existing_workspace_path(base_path, "glob")? {
        WorkspacePath::Inside(path) => path,
        WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
    };

    let mut builder = GlobSetBuilder::new();
    let glob = match Glob::new(pattern) {
        Ok(glob) => glob,
        Err(error) => {
            return Ok(tool_error(
                call.id,
                format!("invalid glob pattern: {error}"),
            ))
        }
    };
    builder.add(glob);
    let set = match builder.build() {
        Ok(set) => set,
        Err(error) => return Ok(tool_error(call.id, format!("invalid glob set: {error}"))),
    };

    let mut matches = WalkBuilder::new(start)
        .hidden(false)
        .parents(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| !ignored_path(entry.path()))
        .build()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_type()
                .map(|file_type| file_type.is_file())
                .unwrap_or(false)
        })
        .filter_map(|entry| {
            let relative = entry
                .path()
                .strip_prefix(&root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");
            set.is_match(&relative).then_some(relative)
        })
        .take(max_results)
        .collect::<Vec<_>>();

    matches.sort();
    Ok(ToolResult {
        tool_use_id: call.id,
        content: if matches.is_empty() {
            String::new()
        } else {
            matches.join("\n") + "\n"
        },
        is_error: false,
    })
}

pub(crate) fn ignored_path(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|name| matches!(name, ".git" | "node_modules" | "target" | "dist" | "build"))
        .unwrap_or(false)
}
