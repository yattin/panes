use crate::domain::tools::{ToolCall, ToolResult};

use super::{input_string, tool_error, NativeToolExecutor};

pub(crate) async fn execute(
    executor: &NativeToolExecutor,
    call: ToolCall,
) -> anyhow::Result<ToolResult> {
    let tool_use_id = call.id.clone();
    let Some(name) = input_string(&call, &["name", "skill"]) else {
        return Ok(tool_error(tool_use_id, "skill requires input.name"));
    };
    let args = input_string(&call, &["args", "input"])
        .unwrap_or("")
        .to_string();
    let Some(skill) = executor.skills().iter().find(|skill| skill.name == name) else {
        return Ok(tool_error(tool_use_id, format!("skill not found: {name}")));
    };

    Ok(ToolResult {
        tool_use_id,
        content: format!(
            "Skill `{}` from `{}`\n\nArgs:\n{}\n\nInstructions:\n{}",
            skill.name, skill.path, args, skill.prompt
        ),
        is_error: false,
    })
}
