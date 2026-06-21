use anyhow::Context;

use crate::domain::tools::{ToolCall, ToolResult};

use super::{input_path, tool_error, NativeToolExecutor, WorkspacePath};

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let Some(path) = input_path(&call) else {
        return Ok(tool_error(call.id, "file_read requires input.path"));
    };

    let resolved_target = match executor.resolve_existing_workspace_path(path, "file_read")? {
        WorkspacePath::Inside(path) => path,
        WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
    };

    let content = tokio::fs::read_to_string(&resolved_target)
        .await
        .with_context(|| format!("failed to read file {:?}", resolved_target))?;

    Ok(ToolResult {
        tool_use_id: call.id,
        content,
        is_error: false,
    })
}
