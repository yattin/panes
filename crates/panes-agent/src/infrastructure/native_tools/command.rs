use std::{
    collections::BTreeMap,
    process::{Output, Stdio},
    time::Duration,
};

use anyhow::Context;
use serde_json::json;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::domain::{
    permission::PermissionRequest,
    tools::{ToolCall, ToolResult},
};

use super::{input_string, tool_error, NativeToolExecutor};

const COMMAND_KILL_GRACE: Duration = Duration::from_secs(2);

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
    cancellation: &CancellationToken,
) -> anyhow::Result<ToolResult> {
    let Some(command) = input_string(&call, &["command", "cmd"]) else {
        return Ok(tool_error(
            call.id,
            "execute_command requires input.command",
        ));
    };
    let tool_use_id = call.id.clone();
    let run_in_background = call
        .input
        .get("run_in_background")
        .or_else(|| call.input.get("runInBackground"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let timeout = call
        .input
        .get("timeout_ms")
        .or_else(|| call.input.get("timeoutMs"))
        .or_else(|| call.input.get("timeout"))
        .and_then(serde_json::Value::as_u64)
        .map(Duration::from_millis)
        .unwrap_or_else(|| executor.command_timeout());

    let decision = executor
        .permissions()
        .request(PermissionRequest {
            action_id: tool_use_id.clone(),
            action_type: "execute_command".to_string(),
            summary: command.to_string(),
            details: json!({
                "command": command,
                "workingDirectory": executor.workspace_root().to_string_lossy(),
            }),
        })
        .await?;
    if !decision.allows() {
        return Ok(tool_error(
            tool_use_id,
            "execute_command denied by permission gateway",
        ));
    }
    if cancellation.is_cancelled() {
        return Ok(tool_error(tool_use_id, "execute_command cancelled"));
    }

    let root = executor.workspace_root().canonicalize().with_context(|| {
        format!(
            "failed to resolve workspace root {:?}",
            executor.workspace_root()
        )
    })?;

    if run_in_background {
        let task_id = register_background_command(executor, command.to_string(), root, timeout)?;
        return Ok(ToolResult {
            tool_use_id,
            content: serde_json::json!({
                "task_id": task_id,
                "status": "running",
            })
            .to_string(),
            is_error: false,
        });
    }

    let output = match run_command(command, root, timeout, cancellation).await? {
        CommandRunOutcome::Completed(output) => output,
        CommandRunOutcome::TimedOut => {
            return Ok(tool_error(tool_use_id, "execute_command timed out"));
        }
        CommandRunOutcome::Cancelled => {
            return Ok(tool_error(tool_use_id, "execute_command cancelled"));
        }
    };
    let content = command_output_content(&output);

    Ok(ToolResult {
        tool_use_id,
        content,
        is_error: !output.status.success(),
    })
}

#[derive(Debug, Clone)]
pub(crate) struct BackgroundCommand {
    pub id: String,
    pub command: String,
    pub status: String,
    pub output: String,
    pub is_error: bool,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Default)]
pub(crate) struct BackgroundCommandStore {
    next_id: u64,
    commands: BTreeMap<String, BackgroundCommand>,
}

impl BackgroundCommandStore {
    pub(crate) fn list(&self) -> Vec<BackgroundCommand> {
        self.commands.values().cloned().collect()
    }

    pub(crate) fn get(&self, id: &str) -> Option<BackgroundCommand> {
        self.commands.get(id).cloned()
    }

    pub(crate) fn cancel(&mut self, id: &str) -> bool {
        let Some(command) = self.commands.get_mut(id) else {
            return false;
        };
        command.cancellation.cancel();
        if command.status == "running" {
            command.status = "cancelled".to_string();
        }
        true
    }
}

fn register_background_command(
    executor: &NativeToolExecutor,
    command: String,
    root: std::path::PathBuf,
    timeout: Duration,
) -> anyhow::Result<String> {
    let cancellation = CancellationToken::new();
    let (task_id, task_cancellation) = {
        let mut store = executor
            .background_commands()
            .lock()
            .map_err(|_| anyhow::anyhow!("background command store poisoned"))?;
        store.next_id = store.next_id.saturating_add(1);
        let task_id = format!("cmd_{}", store.next_id);
        let task_cancellation = cancellation.clone();
        store.commands.insert(
            task_id.clone(),
            BackgroundCommand {
                id: task_id.clone(),
                command: command.clone(),
                status: "running".to_string(),
                output: String::new(),
                is_error: false,
                cancellation,
            },
        );
        (task_id, task_cancellation)
    };

    let store = executor.background_commands().clone();
    let task_id_for_task = task_id.clone();
    tokio::spawn(async move {
        let result = run_command(&command, root, timeout, &task_cancellation).await;
        let (status, output, is_error) = match result {
            Ok(CommandRunOutcome::Completed(output)) => {
                let content = command_output_content(&output);
                let is_error = !output.status.success();
                (
                    if is_error { "failed" } else { "completed" }.to_string(),
                    content,
                    is_error,
                )
            }
            Ok(CommandRunOutcome::TimedOut) => (
                "timed_out".to_string(),
                "execute_command timed out".to_string(),
                true,
            ),
            Ok(CommandRunOutcome::Cancelled) => (
                "cancelled".to_string(),
                "execute_command cancelled".to_string(),
                true,
            ),
            Err(error) => ("failed".to_string(), error.to_string(), true),
        };
        if let Ok(mut store) = store.lock() {
            if let Some(command) = store.commands.get_mut(&task_id_for_task) {
                command.status = status;
                command.output = output;
                command.is_error = is_error;
            }
        }
    });

    Ok(task_id)
}

async fn run_command(
    command: &str,
    root: std::path::PathBuf,
    timeout: Duration,
    cancellation: &CancellationToken,
) -> anyhow::Result<CommandRunOutcome> {
    let mut process = shell_command(command);
    process.current_dir(root);
    process.kill_on_drop(true);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());
    run_with_timeout(process, timeout, command, cancellation).await
}

fn command_output_content(output: &Output) -> String {
    let mut content = String::new();
    if !output.stdout.is_empty() {
        content.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    if content.is_empty() {
        content = format!("exit status: {}", output.status);
    }
    content
}

enum CommandRunOutcome {
    Completed(Output),
    TimedOut,
    Cancelled,
}

async fn run_with_timeout(
    mut process: Command,
    timeout: Duration,
    command: &str,
    cancellation: &CancellationToken,
) -> anyhow::Result<CommandRunOutcome> {
    let mut child = process
        .spawn()
        .with_context(|| format!("failed to run command {command:?}"))?;
    let mut stdout = child.stdout.take().context("failed to capture stdout")?;
    let mut stderr = child.stderr.take().context("failed to capture stderr")?;

    let stdout_task = tokio::spawn(async move {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output).await.map(|_| output)
    });
    let stderr_task = tokio::spawn(async move {
        let mut output = Vec::new();
        stderr.read_to_end(&mut output).await.map(|_| output)
    });

    let status = tokio::select! {
        status = child.wait() => {
            status.with_context(|| format!("failed to run command {command:?}"))?
        }
        _ = tokio::time::sleep(timeout) => {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(COMMAND_KILL_GRACE, child.wait()).await;
            stdout_task.abort();
            stderr_task.abort();
            return Ok(CommandRunOutcome::TimedOut);
        }
        _ = cancellation.cancelled() => {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(COMMAND_KILL_GRACE, child.wait()).await;
            stdout_task.abort();
            stderr_task.abort();
            return Ok(CommandRunOutcome::Cancelled);
        }
    };

    let stdout = stdout_task
        .await
        .context("failed to join stdout reader")?
        .context("failed to read stdout")?;
    let stderr = stderr_task
        .await
        .context("failed to join stderr reader")?
        .context("failed to read stderr")?;

    Ok(CommandRunOutcome::Completed(Output {
        status,
        stdout,
        stderr,
    }))
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut process = Command::new("powershell");
    process.args(["-NoProfile", "-Command", command]);
    process
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> Command {
    let mut process = Command::new("sh");
    process.args(["-c", command]);
    process
}
