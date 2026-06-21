use std::{
    io::Write,
    process::{Command, Stdio},
};

use anyhow::Context;

use crate::domain::tools::{ToolCall, ToolResult};

use super::{input_string, tool_error, NativeToolExecutor};

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let Some(patch) = input_string(&call, &["patch", "diff"]) else {
        return Ok(tool_error(call.id, "apply_patch requires input.patch"));
    };
    let root = executor.workspace_root().canonicalize().with_context(|| {
        format!(
            "failed to resolve workspace root {:?}",
            executor.workspace_root()
        )
    })?;
    let patch = patch.to_string();
    let result = tokio::task::spawn_blocking(move || {
        let mut child = Command::new("git")
            .arg("apply")
            .arg("--whitespace=nowarn")
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn git apply")?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(patch.as_bytes())
                .context("failed to write patch to git apply")?;
        }
        child.wait_with_output().context("failed to run git apply")
    })
    .await
    .context("failed to join apply_patch task")??;

    let mut output = String::new();
    if !result.stdout.is_empty() {
        output.push_str(&String::from_utf8_lossy(&result.stdout));
    }
    if !result.stderr.is_empty() {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&String::from_utf8_lossy(&result.stderr));
    }
    if output.is_empty() {
        output = format!("exit status: {}", result.status);
    }

    Ok(ToolResult {
        tool_use_id: call.id,
        content: output,
        is_error: !result.status.success(),
    })
}
