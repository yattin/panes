use anyhow::Context;

use crate::domain::tools::{ToolCall, ToolResult};

use super::{input_path, tool_error, NativeToolExecutor, WorkspacePath};

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let Some(path) = input_path(&call) else {
        return Ok(tool_error(call.id, "list_files requires input.path"));
    };

    let resolved_target = match executor.resolve_existing_workspace_path(path, "list_files")? {
        WorkspacePath::Inside(path) => path,
        WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
    };

    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&resolved_target)
        .await
        .with_context(|| format!("failed to list directory {:?}", resolved_target))?;

    while let Some(entry) = dir
        .next_entry()
        .await
        .with_context(|| format!("failed to read directory entry {:?}", resolved_target))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry
            .file_type()
            .await
            .with_context(|| format!("failed to read file type for {:?}", entry.path()))?;
        if file_type.is_dir() {
            entries.push(format!("{name}/"));
        } else {
            entries.push(name);
        }
    }

    entries.sort();
    Ok(ToolResult {
        tool_use_id: call.id,
        content: entries.join("\n") + "\n",
        is_error: false,
    })
}
