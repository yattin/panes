use anyhow::Context;

use crate::domain::tools::{ToolCall, ToolResult};

use super::{input_path, input_string, tool_error, NativeToolExecutor, WorkspacePath};

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let Some(path) = input_path(&call) else {
        return Ok(tool_error(call.id, "file_write requires input.path"));
    };
    let Some(content) = input_string(&call, &["content", "text"]) else {
        return Ok(tool_error(call.id, "file_write requires input.content"));
    };
    let tool_use_id = call.id.clone();

    let target = match executor.resolve_new_workspace_file_path(path, "file_write")? {
        WorkspacePath::Inside(path) => path,
        WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
    };

    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create parent directory {:?}", parent))?;
    }
    tokio::fs::write(&target, content)
        .await
        .with_context(|| format!("failed to write file {:?}", target))?;

    Ok(ToolResult {
        tool_use_id,
        content: format!("wrote {} bytes to {}", content.len(), path),
        is_error: false,
    })
}
