use serde_json::json;

use crate::domain::tools::{ToolCall, ToolResult};

use super::{input_string, tool_error, NativeToolExecutor};

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let action = input_string(&call, &["action", "operation"]).unwrap_or("list");
    let result = match action {
        "list" => list(executor),
        "status" | "output" => get(executor, &call),
        "cancel" => cancel(executor, &call),
        other => Err(format!("unknown monitor action: {other}")),
    };

    match result {
        Ok(content) => Ok(ToolResult {
            tool_use_id: call.id,
            content,
            is_error: false,
        }),
        Err(message) => Ok(tool_error(call.id, message)),
    }
}

fn list(executor: &NativeToolExecutor) -> Result<String, String> {
    let store = executor
        .background_commands()
        .lock()
        .map_err(|_| "background command store poisoned".to_string())?;
    let items = store
        .list()
        .into_iter()
        .map(|command| {
            json!({
                "task_id": command.id,
                "command": command.command,
                "status": command.status,
                "is_error": command.is_error,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&items).map_err(|error| error.to_string())
}

fn get(executor: &NativeToolExecutor, call: &ToolCall) -> Result<String, String> {
    let id = input_string(call, &["task_id", "taskId", "id"])
        .ok_or_else(|| "monitor task_id is required".to_string())?;
    let store = executor
        .background_commands()
        .lock()
        .map_err(|_| "background command store poisoned".to_string())?;
    let command = store
        .get(id)
        .ok_or_else(|| format!("background command not found: {id}"))?;
    serde_json::to_string_pretty(&json!({
        "task_id": command.id,
        "command": command.command,
        "status": command.status,
        "output": command.output,
        "is_error": command.is_error,
    }))
    .map_err(|error| error.to_string())
}

fn cancel(executor: &NativeToolExecutor, call: &ToolCall) -> Result<String, String> {
    let id = input_string(call, &["task_id", "taskId", "id"])
        .ok_or_else(|| "monitor task_id is required".to_string())?;
    let mut store = executor
        .background_commands()
        .lock()
        .map_err(|_| "background command store poisoned".to_string())?;
    if store.cancel(id) {
        Ok(format!("cancelled {id}"))
    } else {
        Err(format!("background command not found: {id}"))
    }
}
