use std::collections::BTreeMap;

use serde::Serialize;

use crate::domain::tools::{ToolCall, ToolResult};

use super::{input_string, tool_error, NativeToolExecutor};

#[derive(Debug, Default)]
pub(crate) struct TaskStore {
    next_id: u64,
    tasks: BTreeMap<String, Task>,
}

#[derive(Debug, Clone, Serialize)]
struct Task {
    id: String,
    subject: String,
    description: String,
    status: String,
}

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let Some(operation) = input_string(&call, &["operation", "action"]) else {
        return Ok(tool_error(
            call.id,
            "task_management requires input.operation",
        ));
    };

    let result = match operation {
        "create" => create_task(executor, &call),
        "list" => list_tasks(executor),
        "get" => get_task(executor, &call),
        "update" => update_task(executor, &call),
        "complete" => complete_task(executor, &call),
        "delete" => delete_task(executor, &call),
        _ => Err(format!("unknown task operation: {operation}")),
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

fn create_task(executor: &NativeToolExecutor, call: &ToolCall) -> Result<String, String> {
    let subject = input_string(call, &["subject", "title"]).unwrap_or("Untitled task");
    let description = input_string(call, &["description"]).unwrap_or("");
    let mut store = executor
        .task_store()
        .lock()
        .map_err(|_| "task store poisoned".to_string())?;

    store.next_id += 1;
    let id = format!("task_{}", store.next_id);
    let task = Task {
        id: id.clone(),
        subject: subject.to_string(),
        description: description.to_string(),
        status: "open".to_string(),
    };
    store.tasks.insert(id.clone(), task.clone());
    task_json(&task)
}

fn list_tasks(executor: &NativeToolExecutor) -> Result<String, String> {
    let store = executor
        .task_store()
        .lock()
        .map_err(|_| "task store poisoned".to_string())?;
    serde_json::to_string_pretty(&store.tasks.values().collect::<Vec<_>>())
        .map_err(|error| error.to_string())
}

fn get_task(executor: &NativeToolExecutor, call: &ToolCall) -> Result<String, String> {
    let id = required_task_id(call)?;
    let store = executor
        .task_store()
        .lock()
        .map_err(|_| "task store poisoned".to_string())?;
    let task = store
        .tasks
        .get(id)
        .ok_or_else(|| format!("task not found: {id}"))?;
    task_json(task)
}

fn update_task(executor: &NativeToolExecutor, call: &ToolCall) -> Result<String, String> {
    let id = required_task_id(call)?;
    let mut store = executor
        .task_store()
        .lock()
        .map_err(|_| "task store poisoned".to_string())?;
    let task = store
        .tasks
        .get_mut(id)
        .ok_or_else(|| format!("task not found: {id}"))?;

    if let Some(subject) = input_string(call, &["subject", "title"]) {
        task.subject = subject.to_string();
    }
    if let Some(description) = input_string(call, &["description"]) {
        task.description = description.to_string();
    }
    if let Some(status) = input_string(call, &["status"]) {
        task.status = status.to_string();
    }
    task_json(task)
}

fn complete_task(executor: &NativeToolExecutor, call: &ToolCall) -> Result<String, String> {
    let id = required_task_id(call)?;
    let mut store = executor
        .task_store()
        .lock()
        .map_err(|_| "task store poisoned".to_string())?;
    let task = store
        .tasks
        .get_mut(id)
        .ok_or_else(|| format!("task not found: {id}"))?;
    task.status = "completed".to_string();
    task_json(task)
}

fn delete_task(executor: &NativeToolExecutor, call: &ToolCall) -> Result<String, String> {
    let id = required_task_id(call)?;
    let mut store = executor
        .task_store()
        .lock()
        .map_err(|_| "task store poisoned".to_string())?;
    if store.tasks.remove(id).is_some() {
        Ok(format!("deleted {id}"))
    } else {
        Err(format!("task not found: {id}"))
    }
}

fn required_task_id(call: &ToolCall) -> Result<&str, String> {
    input_string(call, &["task_id", "taskId", "id"])
        .ok_or_else(|| "task_id is required".to_string())
}

fn task_json(task: &Task) -> Result<String, String> {
    serde_json::to_string_pretty(task).map_err(|error| error.to_string())
}
