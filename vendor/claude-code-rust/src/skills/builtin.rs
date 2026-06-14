//! Built-in Skills

use super::{Skill, SkillCategory, SkillContext, SkillError, SkillParams, SkillResult};
use async_trait::async_trait;
use serde_json;
use std::sync::Arc;

pub struct CommitSkill;

#[async_trait]
impl Skill for CommitSkill {
    fn name(&self) -> &str {
        "commit"
    }

    fn description(&self) -> &str {
        "Inspect git changes and optionally create a commit with --message"
    }

    fn examples(&self) -> Vec<String> {
        vec![
            "/commit".to_string(),
            "/commit --message=\"Fixed bug\"".to_string(),
            "/commit --all --message=\"Update implementation\"".to_string(),
        ]
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string"},
                "amend": {"type": "boolean"},
                "all": {"type": "boolean"}
            }
        })
    }

    async fn execute(
        &self,
        params: SkillParams,
        context: SkillContext,
    ) -> Result<SkillResult, SkillError> {
        let registry = tool_registry(&context);
        let amend = params.flags.contains_key("amend");
        let all = params.flags.contains_key("all") || params.flags.contains_key("a");

        if all {
            run_tool(
                &registry,
                "git_operations",
                serde_json::json!({
                    "operation": "add",
                    "path": context.cwd,
                    "files": ["."]
                }),
            )
            .await?;
        }

        if let Some(message) = params.named_params.get("message") {
            let mut args = Vec::new();
            if amend {
                args.push("--amend".to_string());
            }
            let result = run_tool(
                &registry,
                "git_operations",
                serde_json::json!({
                    "operation": "commit",
                    "path": context.cwd,
                    "message": message,
                    "args": args
                }),
            )
            .await?;

            return Ok(skill_ok(
                "Git commit executed",
                serde_json::json!({
                    "skill": "commit",
                    "committed": true,
                    "tool_output": result.content,
                    "metadata": result.metadata
                }),
            ));
        }

        let status = run_tool(
            &registry,
            "git_operations",
            serde_json::json!({
                "operation": "status",
                "path": context.cwd,
                "args": ["--short"]
            }),
        )
        .await?;

        Ok(skill_ok(
            "Git status collected. Provide --message to commit.",
            serde_json::json!({
                "skill": "commit",
                "committed": false,
                "amend": amend,
                "all": all,
                "status": status.content
            }),
        ))
    }
}

pub struct ReviewSkill;

#[async_trait]
impl Skill for ReviewSkill {
    fn name(&self) -> &str {
        "review"
    }

    fn description(&self) -> &str {
        "Collect code review context from git diff or selected files"
    }

    fn examples(&self) -> Vec<String> {
        vec![
            "/review".to_string(),
            "/review src/main.rs".to_string(),
            "/review --diff --strict".to_string(),
        ]
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "files": {"type": "array", "items": {"type": "string"}},
                "diff": {"type": "boolean"},
                "strict": {"type": "boolean"}
            }
        })
    }

    async fn execute(
        &self,
        params: SkillParams,
        context: SkillContext,
    ) -> Result<SkillResult, SkillError> {
        let registry = tool_registry(&context);
        let files = params.args;
        let strict = params.flags.contains_key("strict");

        let review_context = if files.is_empty() || params.flags.contains_key("diff") {
            let diff = run_tool(
                &registry,
                "git_operations",
                serde_json::json!({
                    "operation": "diff",
                    "path": context.cwd,
                    "args": ["--stat"]
                }),
            )
            .await?;
            serde_json::json!({"mode": "diff", "summary": diff.content})
        } else {
            let mut previews = Vec::new();
            for file in &files {
                let preview = run_tool(
                    &registry,
                    "file_read",
                    serde_json::json!({
                        "file_path": file,
                        "limit": 80
                    }),
                )
                .await?;
                previews.push(serde_json::json!({"file": file, "preview": preview.content}));
            }
            serde_json::json!({"mode": "files", "files": previews})
        };

        Ok(skill_ok(
            "Review context collected",
            serde_json::json!({
                "skill": "review",
                "strict": strict,
                "context": review_context
            }),
        ))
    }
}

pub struct TestSkill;

#[async_trait]
impl Skill for TestSkill {
    fn name(&self) -> &str {
        "test"
    }

    fn description(&self) -> &str {
        "Prepare or execute the project test command"
    }

    fn examples(&self) -> Vec<String> {
        vec![
            "/test".to_string(),
            "/test --execute".to_string(),
            "/test --command=\"cargo test --lib\" --execute".to_string(),
        ]
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {"type": "string"},
                "execute": {"type": "boolean"},
                "unit": {"type": "boolean"},
                "integration": {"type": "boolean"},
                "coverage": {"type": "boolean"}
            }
        })
    }

    async fn execute(
        &self,
        params: SkillParams,
        context: SkillContext,
    ) -> Result<SkillResult, SkillError> {
        let command = params
            .named_params
            .get("command")
            .cloned()
            .unwrap_or_else(|| "cargo test".to_string());

        if !params.flags.contains_key("execute") {
            return Ok(skill_ok(
                "Test command prepared",
                serde_json::json!({
                    "skill": "test",
                    "command": command,
                    "executed": false,
                    "unit": params.flags.contains_key("unit"),
                    "integration": params.flags.contains_key("integration"),
                    "coverage": params.flags.contains_key("coverage")
                }),
            ));
        }

        let registry = tool_registry(&context);
        let result = run_tool(
            &registry,
            "execute_command",
            serde_json::json!({
                "command": command,
                "cwd": context.cwd,
                "timeout_ms": 120000
            }),
        )
        .await?;
        Ok(skill_ok(
            "Test command executed",
            serde_json::json!({
                "skill": "test",
                "executed": true,
                "tool_output": result.content
            }),
        ))
    }
}

pub struct DocumentSkill;

#[async_trait]
impl Skill for DocumentSkill {
    fn name(&self) -> &str {
        "document"
    }

    fn description(&self) -> &str {
        "Collect documentation context for README/API updates"
    }

    fn examples(&self) -> Vec<String> {
        vec!["/document".to_string(), "/document --readme".to_string()]
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "api": {"type": "boolean"},
                "readme": {"type": "boolean"},
                "force": {"type": "boolean"}
            }
        })
    }

    async fn execute(
        &self,
        params: SkillParams,
        context: SkillContext,
    ) -> Result<SkillResult, SkillError> {
        let registry = tool_registry(&context);
        let readme_preview = run_tool(
            &registry,
            "file_read",
            serde_json::json!({
                "file_path": format!("{}/README.md", context.cwd),
                "limit": 80
            }),
        )
        .await
        .ok()
        .map(|output| output.content);

        Ok(skill_ok(
            "Documentation context collected",
            serde_json::json!({
                "skill": "document",
                "readme_preview": readme_preview,
                "api": params.flags.contains_key("api"),
                "readme": params.flags.contains_key("readme"),
                "force": params.flags.contains_key("force")
            }),
        ))
    }
}

pub struct BuildSkill;

#[async_trait]
impl Skill for BuildSkill {
    fn name(&self) -> &str {
        "build"
    }

    fn description(&self) -> &str {
        "Prepare or execute the project build command"
    }

    fn examples(&self) -> Vec<String> {
        vec![
            "/build".to_string(),
            "/build --release --execute".to_string(),
            "/build --clean --verbose --execute".to_string(),
        ]
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "release": {"type": "boolean"},
                "clean": {"type": "boolean"},
                "verbose": {"type": "boolean"},
                "execute": {"type": "boolean"}
            }
        })
    }

    async fn execute(
        &self,
        params: SkillParams,
        context: SkillContext,
    ) -> Result<SkillResult, SkillError> {
        let mut command = String::new();
        if params.flags.contains_key("clean") {
            command.push_str("cargo clean && ");
        }
        command.push_str(if params.flags.contains_key("release") {
            "cargo build --release"
        } else {
            "cargo build"
        });
        if params.flags.contains_key("verbose") {
            command.push_str(" --verbose");
        }

        if !params.flags.contains_key("execute") {
            return Ok(skill_ok(
                "Build command prepared",
                serde_json::json!({
                    "skill": "build",
                    "command": command,
                    "executed": false
                }),
            ));
        }

        let registry = tool_registry(&context);
        let result = run_tool(
            &registry,
            "execute_command",
            serde_json::json!({
                "command": command,
                "cwd": context.cwd,
                "timeout_ms": 120000
            }),
        )
        .await?;
        Ok(skill_ok(
            "Build command executed",
            serde_json::json!({
                "skill": "build",
                "executed": true,
                "tool_output": result.content
            }),
        ))
    }
}

pub struct BuiltinSkills;

impl BuiltinSkills {
    pub fn all() -> Vec<(Box<dyn Skill>, Vec<SkillCategory>)> {
        vec![
            (
                Box::new(CommitSkill) as Box<dyn Skill>,
                vec![SkillCategory::Git, SkillCategory::Utility],
            ),
            (
                Box::new(ReviewSkill),
                vec![SkillCategory::CodeReview, SkillCategory::Utility],
            ),
            (
                Box::new(TestSkill),
                vec![SkillCategory::Testing, SkillCategory::Utility],
            ),
            (
                Box::new(DocumentSkill),
                vec![SkillCategory::Documentation, SkillCategory::Utility],
            ),
            (
                Box::new(BuildSkill),
                vec![SkillCategory::ProjectSetup, SkillCategory::Utility],
            ),
        ]
    }
}

fn skill_ok(message: &str, output: serde_json::Value) -> SkillResult {
    SkillResult {
        success: true,
        message: message.to_string(),
        output: Some(output),
        metadata: std::collections::HashMap::new(),
    }
}

fn tool_registry(context: &SkillContext) -> Arc<crate::tools::ToolRegistry> {
    context
        .tool_registry
        .clone()
        .unwrap_or_else(|| Arc::new(crate::tools::ToolRegistry::new()))
}

async fn run_tool(
    registry: &Arc<crate::tools::ToolRegistry>,
    name: &str,
    input: serde_json::Value,
) -> Result<crate::tools::ToolOutput, SkillError> {
    registry.execute(name, input).await.map_err(|e| SkillError {
        message: e.message,
        code: e.code.unwrap_or_else(|| "tool_error".to_string()),
        details: None,
    })
}
