use anyhow::Context;

use crate::domain::tools::{ToolCall, ToolResult};

use super::{input_path, input_string, tool_error, NativeToolExecutor, WorkspacePath};

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let Some(path) = input_path(&call) else {
        return Ok(tool_error(call.id, "file_edit requires input.path"));
    };
    let Some(old_text) = input_string(&call, &["old_text", "oldText", "old"]) else {
        return Ok(tool_error(call.id, "file_edit requires input.old_text"));
    };
    let Some(new_text) = input_string(&call, &["new_text", "newText", "new"]) else {
        return Ok(tool_error(call.id, "file_edit requires input.new_text"));
    };
    let replace_all = call
        .input
        .get("replace_all")
        .or_else(|| call.input.get("replaceAll"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let tool_use_id = call.id.clone();

    let target = match executor.resolve_existing_workspace_path(path, "file_edit")? {
        WorkspacePath::Inside(path) => path,
        WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
    };

    let original = tokio::fs::read_to_string(&target)
        .await
        .with_context(|| format!("failed to read file {:?}", target))?;

    if !original.contains(old_text) {
        return Ok(tool_error(call.id, "file_edit old_text not found"));
    }

    let occurrence_count = original.matches(old_text).count();
    if !replace_all && occurrence_count != 1 {
        return Ok(tool_error(
            call.id,
            format!("file_edit old_text must match exactly once; found {occurrence_count}"),
        ));
    }

    let updated = if replace_all {
        original.replace(old_text, new_text)
    } else {
        original.replacen(old_text, new_text, 1)
    };
    tokio::fs::write(&target, updated)
        .await
        .with_context(|| format!("failed to write file {:?}", target))?;

    Ok(ToolResult {
        tool_use_id,
        content: format!("replaced {occurrence_count} occurrence(s) in {path}"),
        is_error: false,
    })
}
