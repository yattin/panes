//! Ignored real LLM tests for the public library crate API.
//!
//! Run manually with:
//! cargo test --test real_llm_library_test -- --ignored --nocapture

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use claude_code_rs::{
    ApiClient, ChatMessage, ChatResponse, Settings, ToolDefinition, ToolError, ToolRegistry,
};
use serde_json::json;

fn settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".panes-agent").join("settings.json"))
}

fn load_settings_or_skip() -> anyhow::Result<Option<Settings>> {
    let Some(path) = settings_path() else {
        println!("Skipping real LLM test: could not resolve the user home directory.");
        return Ok(None);
    };

    if !path.exists() {
        println!(
            "Skipping real LLM test: settings file was not found at {}.",
            path.display()
        );
        println!("Create it first, then set an API key with: claude-code config set api_key ...");
        return Ok(None);
    }

    let settings = Settings::load()?;
    if settings
        .api
        .api_key
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        println!(
            "Skipping real LLM test: settings.api.api_key is empty in {}.",
            path.display()
        );
        println!("Set an API key with: claude-code config set api_key ...");
        return Ok(None);
    }

    Ok(Some(settings))
}

fn print_response_summary(response: &ChatResponse) {
    println!("model: {}", response.model);
    if let Some(usage) = &response.usage {
        println!(
            "usage: prompt_tokens={}, completion_tokens={}, total_tokens={}",
            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
        );
    }

    let summary = response
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_deref())
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect::<String>();

    if !summary.trim().is_empty() {
        println!("assistant summary: {}", summary);
    }
}

#[tokio::test]
#[ignore = "requires ~/.panes-agent/settings.json with settings.api.api_key"]
async fn real_llm_basic_chat_via_library_crate() -> anyhow::Result<()> {
    let Some(settings) = load_settings_or_skip()? else {
        return Ok(());
    };

    println!("configured model: {}", settings.model);

    let client = ApiClient::new(settings);
    let response = client
        .chat(
            vec![ChatMessage::user(
                "Reply with one short sentence confirming the library crate call works.",
            )],
            None,
        )
        .await?;

    print_response_summary(&response);

    let assistant_content = response
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_deref())
        .unwrap_or_default()
        .trim();

    assert!(
        !assistant_content.is_empty(),
        "expected at least one non-empty assistant message"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires ~/.panes-agent/settings.json with settings.api.api_key"]
async fn real_llm_tool_schema_exposure_via_library_crate() -> anyhow::Result<()> {
    let Some(settings) = load_settings_or_skip()? else {
        return Ok(());
    };

    println!("configured model: {}", settings.model);

    let tools: Vec<ToolDefinition> = ToolRegistry::new()
        .list()
        .into_iter()
        .map(|tool| ToolDefinition::new(tool.name(), tool.description(), tool.input_schema()))
        .collect();

    assert!(
        !tools.is_empty(),
        "expected built-in tools to be registered"
    );
    println!("exposed tool definitions: {}", tools.len());

    let client = ApiClient::new(settings);
    let response = client
        .chat(
            vec![ChatMessage::user(
                "Given the available tools, say which tool would list files in a directory. \
                 Keep the answer short. A tool call is optional.",
            )],
            Some(tools),
        )
        .await?;

    print_response_summary(&response);

    let first_choice = response
        .choices
        .first()
        .expect("expected the API response to include at least one choice");
    let has_text = first_choice
        .message
        .content
        .as_deref()
        .map(str::trim)
        .is_some_and(|content| !content.is_empty());
    let has_tool_call = first_choice
        .message
        .tool_calls
        .as_ref()
        .is_some_and(|tool_calls| !tool_calls.is_empty());

    assert!(
        has_text || has_tool_call,
        "expected a parsable assistant response with text or tool calls"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires ~/.panes-agent/settings.json with settings.api.api_key"]
async fn real_llm_all_tools_against_real_directory_via_library_crate() -> anyhow::Result<()> {
    let Some(settings) = load_settings_or_skip()? else {
        return Ok(());
    };

    let workspace = tempfile::tempdir()?;
    seed_real_workspace(workspace.path())?;
    println!(
        "mounted real test directory: {}",
        workspace.path().display()
    );

    let registry = ToolRegistry::new();
    let tool_definitions: Vec<ToolDefinition> = registry
        .list()
        .into_iter()
        .map(|tool| ToolDefinition::new(tool.name(), tool.description(), tool.input_schema()))
        .collect();
    let expected_tool_names: BTreeSet<String> = tool_definitions
        .iter()
        .map(|tool| tool.function.name.clone())
        .collect();

    println!(
        "exposing {} real tool schemas: {}",
        expected_tool_names.len(),
        expected_tool_names
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );

    let client = ApiClient::new(settings);
    let prompt = format!(
        "You are validating a Rust library crate integration. The mounted real test directory is: {}\n\
         Return a compact JSON object with one key, tools, containing every available tool name. \
         Do not invent names. Do not call a tool unless your API requires it.",
        workspace.path().display()
    );
    let response = client
        .chat(vec![ChatMessage::user(prompt)], Some(tool_definitions))
        .await?;

    print_response_summary(&response);
    assert_parsable_llm_response(&response, &expected_tool_names);

    let covered_tool_names = execute_all_registered_tools(&registry, workspace.path()).await?;
    assert_eq!(
        covered_tool_names, expected_tool_names,
        "local execution coverage should match the registered tool set"
    );

    Ok(())
}

fn assert_parsable_llm_response(response: &ChatResponse, expected_tool_names: &BTreeSet<String>) {
    let first_choice = response
        .choices
        .first()
        .expect("expected the API response to include at least one choice");

    let content = first_choice.message.content.as_deref().unwrap_or_default();
    let has_text = !content.trim().is_empty();
    let has_tool_call = first_choice
        .message
        .tool_calls
        .as_ref()
        .is_some_and(|tool_calls| !tool_calls.is_empty());

    if has_text {
        let missing: Vec<&String> = expected_tool_names
            .iter()
            .filter(|name| !content.contains(name.as_str()))
            .collect();
        if missing.is_empty() {
            println!("assistant mentioned every exposed tool name");
        } else {
            println!(
                "assistant response parsed, but did not mention these tool names: {}",
                missing.into_iter().cloned().collect::<Vec<_>>().join(", ")
            );
        }
    }

    if let Some(tool_calls) = &first_choice.message.tool_calls {
        for tool_call in tool_calls {
            assert!(
                expected_tool_names.contains(&tool_call.function.name),
                "model returned unknown tool call: {}",
                tool_call.function.name
            );
        }
    }

    assert!(
        has_text || has_tool_call,
        "expected a parsable assistant response with text or tool calls"
    );
}

fn seed_real_workspace(workspace: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(workspace.join("src"))?;
    std::fs::write(
        workspace.join("README.md"),
        "real llm tool fixture\nneedle-alpha\n",
    )?;
    std::fs::write(
        workspace.join("src").join("main.txt"),
        "first line\nneedle-beta\nedit me\n",
    )?;

    let output = Command::new("git").arg("init").arg(workspace).output()?;
    anyhow::ensure!(
        output.status.success(),
        "failed to initialize git fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

async fn execute_all_registered_tools(
    registry: &ToolRegistry,
    workspace: &Path,
) -> anyhow::Result<BTreeSet<String>> {
    let mut covered = BTreeSet::new();
    let readme = workspace.join("README.md");
    let editable = workspace.join("src").join("main.txt");
    let generated = workspace.join("generated").join("write.txt");

    let output = registry
        .execute(
            "file_read",
            json!({
                "file_path": readme,
                "offset": 1,
                "limit": 2
            }),
        )
        .await
        .map_err(tool_error)?;
    assert!(output.content.contains("needle-alpha"));
    covered.insert("file_read".to_string());

    let output = registry
        .execute(
            "file_write",
            json!({
                "file_path": generated,
                "content": "generated by real LLM library test\n"
            }),
        )
        .await
        .map_err(tool_error)?;
    assert!(output.content.contains("Successfully wrote"));
    covered.insert("file_write".to_string());

    let output = registry
        .execute(
            "file_edit",
            json!({
                "file_path": editable,
                "old_content": "edit me",
                "new_content": "edited safely"
            }),
        )
        .await
        .map_err(tool_error)?;
    assert!(output.content.contains("Successfully edited"));
    covered.insert("file_edit".to_string());

    let output = registry
        .execute(
            "execute_command",
            json!({
                "command": "echo tool-exec-ok",
                "cwd": workspace,
                "timeout_ms": 5000
            }),
        )
        .await
        .map_err(tool_error)?;
    assert!(output.content.contains("tool-exec-ok"));
    covered.insert("execute_command".to_string());

    let output = registry
        .execute(
            "search",
            json!({
                "path": workspace,
                "pattern": "needle-",
                "file_pattern": "*.md",
                "max_results": 5
            }),
        )
        .await
        .map_err(tool_error)?;
    assert!(output.content.contains("needle-alpha"));
    covered.insert("search".to_string());

    let output = registry
        .execute(
            "glob",
            json!({
                "pattern": glob_pattern(workspace, "**/*.txt")
            }),
        )
        .await
        .map_err(tool_error)?;
    assert!(output.content.contains("main.txt"));
    covered.insert("glob".to_string());

    let output = registry
        .execute(
            "list_files",
            json!({
                "path": workspace,
                "recursive": false,
                "max_entries": 20
            }),
        )
        .await
        .map_err(tool_error)?;
    assert!(output.content.contains("README.md"));
    covered.insert("list_files".to_string());

    let output = registry
        .execute(
            "git_operations",
            json!({
                "operation": "status",
                "path": workspace,
                "args": ["--short"]
            }),
        )
        .await
        .map_err(tool_error)?;
    assert_eq!(output.output_type, "text");
    covered.insert("git_operations".to_string());

    let output = registry
        .execute(
            "task_management",
            json!({
                "operation": "create",
                "subject": "real directory coverage",
                "description": "cover task tool from library crate test",
                "priority": "low",
                "tags": ["real-llm-test"]
            }),
        )
        .await
        .map_err(tool_error)?;
    let task_response: serde_json::Value = serde_json::from_str(&output.content)?;
    assert_eq!(task_response["success"], true);
    assert!(task_response["task_id"].as_str().is_some());
    covered.insert("task_management".to_string());

    let output = registry
        .execute(
            "note_edit",
            json!({
                "operation": "create",
                "title": "real directory coverage",
                "content": "cover note tool from library crate test",
                "format": "markdown",
                "tags": ["real-llm-test"]
            }),
        )
        .await
        .map_err(tool_error)?;
    let note_response: serde_json::Value = serde_json::from_str(&output.content)?;
    assert_eq!(note_response["success"], true);
    assert!(note_response["note_id"].as_str().is_some());
    covered.insert("note_edit".to_string());

    println!(
        "locally executed every registered tool against {}",
        workspace.display()
    );

    Ok(covered)
}

fn glob_pattern(root: &Path, pattern: &str) -> String {
    root.join(pattern).display().to_string().replace('\\', "/")
}

fn tool_error(error: ToolError) -> anyhow::Error {
    anyhow::anyhow!(
        "tool failed: {} ({})",
        error.message,
        error.code.unwrap_or_else(|| "no_code".to_string())
    )
}
