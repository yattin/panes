use crate::domain::tools::{ToolCall, ToolResult};

use super::{input_path, input_string, tool_error, NativeToolExecutor, WorkspacePath};

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let Some(path) = input_path(&call) else {
        return Ok(tool_error(call.id, "search requires input.path"));
    };
    let Some(pattern) = input_string(&call, &["pattern", "query"]) else {
        return Ok(tool_error(call.id, "search requires input.pattern"));
    };
    if pattern.is_empty() {
        return Ok(tool_error(
            call.id,
            "search input.pattern must not be empty",
        ));
    }

    let root = match executor.resolve_existing_workspace_path(".", "search")? {
        WorkspacePath::Inside(path) => path,
        WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
    };
    let resolved_target = match executor.resolve_existing_workspace_path(path, "search")? {
        WorkspacePath::Inside(path) => path,
        WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
    };

    let mut matches = Vec::new();
    let mut pending = vec![resolved_target];
    while let Some(path) = pending.pop() {
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        if metadata.is_dir() {
            let Ok(entries) = std::fs::read_dir(&path) else {
                continue;
            };
            for entry in entries.flatten() {
                pending.push(entry.path());
            }
            continue;
        }

        if !metadata.is_file() {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let relative_path = path
            .strip_prefix(&root)
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");

        for (index, line) in content.lines().enumerate() {
            if line.contains(pattern) {
                matches.push(format!("{}:{}:{}", relative_path, index + 1, line));
            }
        }
    }

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
