use std::collections::BTreeMap;

use anyhow::Context;
use serde_json::Value;

use crate::domain::tools::{ToolCall, ToolResult};

use super::{tool_error, NativeToolExecutor, WorkspacePath};

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let Some(edits) = call.input.get("edits").and_then(Value::as_array) else {
        return Ok(tool_error(call.id, "batch_edit requires input.edits"));
    };
    let mut pending = BTreeMap::<String, String>::new();
    let mut total_replacements = 0usize;

    for edit in edits {
        let Some(path) = edit
            .get("path")
            .or_else(|| edit.get("file_path"))
            .or_else(|| edit.get("filePath"))
            .and_then(Value::as_str)
        else {
            return Ok(tool_error(call.id, "batch_edit edit requires path"));
        };
        let Some(old_text) = edit
            .get("old_text")
            .or_else(|| edit.get("oldText"))
            .or_else(|| edit.get("old_string"))
            .or_else(|| edit.get("oldString"))
            .and_then(Value::as_str)
        else {
            return Ok(tool_error(call.id, "batch_edit edit requires old_text"));
        };
        let Some(new_text) = edit
            .get("new_text")
            .or_else(|| edit.get("newText"))
            .or_else(|| edit.get("new_string"))
            .or_else(|| edit.get("newString"))
            .and_then(Value::as_str)
        else {
            return Ok(tool_error(call.id, "batch_edit edit requires new_text"));
        };
        let replace_all = edit
            .get("replace_all")
            .or_else(|| edit.get("replaceAll"))
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let target = match executor.resolve_existing_workspace_path(path, "batch_edit")? {
            WorkspacePath::Inside(path) => path,
            WorkspacePath::Rejected(message) => return Ok(tool_error(call.id, message)),
        };
        let key = target.to_string_lossy().to_string();
        let original = match pending.get(&key) {
            Some(content) => content.clone(),
            None => tokio::fs::read_to_string(&target)
                .await
                .with_context(|| format!("failed to read file {:?}", target))?,
        };
        let occurrence_count = original.matches(old_text).count();
        if occurrence_count == 0 {
            return Ok(tool_error(
                call.id,
                format!("batch_edit old_text not found in {path}"),
            ));
        }
        if !replace_all && occurrence_count != 1 {
            return Ok(tool_error(
                call.id,
                format!("batch_edit old_text in {path} must match exactly once; found {occurrence_count}"),
            ));
        }
        let updated = if replace_all {
            original.replace(old_text, new_text)
        } else {
            original.replacen(old_text, new_text, 1)
        };
        total_replacements += occurrence_count;
        pending.insert(key, updated);
    }

    for (path, content) in pending {
        tokio::fs::write(&path, content)
            .await
            .with_context(|| format!("failed to write file {path}"))?;
    }

    Ok(ToolResult {
        tool_use_id: call.id,
        content: format!("applied {total_replacements} replacement(s)"),
        is_error: false,
    })
}
