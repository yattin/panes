use serde_json::json;

use crate::domain::tools::ToolSpec;

pub fn native_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "file_read".to_string(),
            description: "Read a UTF-8 text file from the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "list_files".to_string(),
            description: "List one level of entries in a workspace directory.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "search".to_string(),
            description:
                "Search workspace text files for a literal pattern. Compatibility alias for grep."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "pattern": { "type": "string" }
                },
                "required": ["path", "pattern"]
            }),
        },
        ToolSpec {
            name: "glob".to_string(),
            description: "Find workspace paths matching a glob pattern.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" },
                    "path": { "type": "string" },
                    "max_results": { "type": "integer" }
                },
                "required": ["pattern"]
            }),
        },
        ToolSpec {
            name: "grep".to_string(),
            description: "Search workspace text files with regex matching, glob filters, output modes, and context."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string" },
                    "path": { "type": "string" },
                    "path_glob": { "type": "string" },
                    "glob": { "type": "string" },
                    "type": { "type": "string" },
                    "output_mode": { "type": "string", "enum": ["content", "files_with_matches", "count"] },
                    "case_sensitive": { "type": "boolean" },
                    "-i": { "type": "boolean" },
                    "-n": { "type": "boolean" },
                    "context": { "type": "integer" },
                    "multiline": { "type": "boolean" },
                    "head_limit": { "type": "integer" },
                    "max_results": { "type": "integer" }
                },
                "required": ["pattern"]
            }),
        },
        ToolSpec {
            name: "file_write".to_string(),
            description: "Create or overwrite a UTF-8 text file in the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolSpec {
            name: "file_edit".to_string(),
            description: "Replace the first occurrence of old_text in a workspace file."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old_text": { "type": "string" },
                    "new_text": { "type": "string" },
                    "replace_all": { "type": "boolean" }
                },
                "required": ["path", "old_text", "new_text"]
            }),
        },
        ToolSpec {
            name: "batch_edit".to_string(),
            description: "Apply multiple exact text replacements atomically across workspace files."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "old_text": { "type": "string" },
                                "new_text": { "type": "string" },
                                "replace_all": { "type": "boolean" }
                            },
                            "required": ["path", "old_text", "new_text"]
                        }
                    }
                },
                "required": ["edits"]
            }),
        },
        ToolSpec {
            name: "apply_patch".to_string(),
            description: "Apply a unified diff patch to the workspace using git apply.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "patch": { "type": "string" } },
                "required": ["patch"]
            }),
        },
        ToolSpec {
            name: "execute_command".to_string(),
            description: "Run a shell command in the workspace after permission approval."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "description": { "type": "string" },
                    "timeout_ms": { "type": "integer" },
                    "run_in_background": { "type": "boolean" }
                },
                "required": ["command"]
            }),
        },
        ToolSpec {
            name: "monitor".to_string(),
            description: "Monitor or cancel background commands started by execute_command."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "status", "output", "cancel"] },
                    "task_id": { "type": "string" }
                }
            }),
        },
        ToolSpec {
            name: "task_management".to_string(),
            description: "Manage in-memory session tasks.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": { "type": "string" },
                    "task_id": { "type": "string" },
                    "subject": { "type": "string" },
                    "description": { "type": "string" },
                    "status": { "type": "string" }
                },
                "required": ["operation"]
            }),
        },
        ToolSpec {
            name: "skill".to_string(),
            description: "Load a named skill's SKILL.md instructions and optional arguments."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "args": { "type": "string" }
                },
                "required": ["name"]
            }),
        },
    ]
}
