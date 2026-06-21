use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use async_trait::async_trait;
use futures::{stream::BoxStream, StreamExt};
use panes_agent::{
    application::ports::ModelClient,
    domain::conversation::AgentMessage,
    infrastructure::{
        anthropic::AnthropicMessagesClient, env_files, native_tools::NativeToolExecutor,
        openai_compatible::OpenAiCompatibleClient, testing::RecordingEventSink,
    },
    AgentEvent, AgentRuntime, AgentRuntimePorts, ModelRequest, ModelStreamEvent, RunTurnCommand,
    SystemContext,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;

const SENTINEL: &str = "PANES_LIVE_OK";

struct LiveToolPorts<M> {
    model: M,
    events: RecordingEventSink,
    tools: NativeToolExecutor,
}

struct CountingModelClient<M> {
    inner: M,
    stream_count: Arc<AtomicUsize>,
}

impl<M> CountingModelClient<M> {
    fn new(inner: M) -> Self {
        Self {
            inner,
            stream_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn stream_count(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.stream_count)
    }
}

#[async_trait]
impl<M> ModelClient for CountingModelClient<M>
where
    M: ModelClient,
{
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>> {
        self.stream_count.fetch_add(1, Ordering::SeqCst);
        self.inner.stream(request).await
    }
}

impl<M> AgentRuntimePorts for LiveToolPorts<M>
where
    M: ModelClient,
{
    type Model = M;
    type Events = RecordingEventSink;
    type Tools = NativeToolExecutor;

    fn model(&self) -> &Self::Model {
        &self.model
    }

    fn events(&self) -> &Self::Events {
        &self.events
    }

    fn tools(&self) -> &Self::Tools {
        &self.tools
    }
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY; run with `cargo test -p panes-agent --test live_llm live_anthropic_smoke -- --ignored --nocapture`"]
async fn live_anthropic_smoke() -> anyhow::Result<()> {
    load_local_dotenv();
    let model = std::env::var("ANTHROPIC_MODEL")
        .unwrap_or_else(|_| AnthropicMessagesClient::default_model().to_string());
    let client = AnthropicMessagesClient::from_env(model)?.with_tool_specs(Vec::new());
    let text = collect_text(client).await?;

    assert!(
        text.contains(SENTINEL),
        "expected `{SENTINEL}` in live Anthropic response, got: {text:?}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY and Python; run with `cargo test -p panes-agent --test live_llm live_anthropic_tool_workdir_bubble_sort -- --ignored --nocapture`"]
async fn live_anthropic_tool_workdir_bubble_sort() -> anyhow::Result<()> {
    load_local_dotenv();
    let python =
        python_command().context("Python was not found; tried python, python3, and py -3")?;
    let workspace = temp_workspace("panes-agent-live-anthropic-bubble-sort")?;
    fs::create_dir_all(&workspace).with_context(|| {
        format!(
            "failed to create live tool workspace {}",
            workspace.display()
        )
    })?;

    println!("live workspace: {}", workspace.display());
    run_live_anthropic_bubble_sort(&workspace, &python).await
}

async fn run_live_anthropic_bubble_sort(
    workspace: &PathBuf,
    python: &PythonCommand,
) -> anyhow::Result<()> {
    let model = std::env::var("ANTHROPIC_MODEL")
        .unwrap_or_else(|_| AnthropicMessagesClient::default_model().to_string());
    run_live_bubble_sort(
        "live-anthropic-tool-workdir-bubble-sort",
        AnthropicMessagesClient::from_env(model)?,
        workspace,
        python,
    )
    .await
}

async fn run_live_bubble_sort<M>(
    conversation_id: &str,
    model: M,
    workspace: &PathBuf,
    python: &PythonCommand,
) -> anyhow::Result<()>
where
    M: ModelClient,
{
    let model = CountingModelClient::new(model);
    let model_turn_counter = model.stream_count();
    let events = RecordingEventSink::default();
    let runtime = AgentRuntime::new(LiveToolPorts {
        model,
        events: events.clone(),
        tools: NativeToolExecutor::new(workspace.clone()),
    });
    let mut system_context = SystemContext::new(Some(workspace.to_string_lossy().into_owned()));
    system_context.append_system_prompt = Some(format!(
        "The workspace root is {}. Use native tools for all file writes and command execution.",
        workspace.display()
    ));

    let outcome = runtime
        .run_turn(RunTurnCommand {
            conversation_id: conversation_id.to_string(),
            messages: vec![AgentMessage::user(
                "In the working directory, create a file named bubble_sort.py. \
                 It must define bubble_sort(values), return a newly sorted list using bubble sort, \
                 and not mutate the input list. Use a file writing tool to create the file, then \
                 use execute_command to run Python and verify [5, 1, 4, 2, 8] becomes \
                 [1, 2, 4, 5, 8]. Keep the final answer concise."
                    .to_string(),
            )],
            system_context,
            cancellation: CancellationToken::new(),
        })
        .await?;

    println!("assistant: {}", outcome.assistant_text);

    let event_log = events.events();
    let metrics = live_run_metrics(conversation_id, workspace, &event_log, &model_turn_counter);
    let metrics_path = workspace.join("run_metrics.json");
    fs::write(&metrics_path, serde_json::to_string_pretty(&metrics)?)
        .with_context(|| format!("failed to write {}", metrics_path.display()))?;
    println!("metrics: {}", serde_json::to_string_pretty(&metrics)?);

    assert!(
        event_log.iter().any(|event| matches!(
            event,
            AgentEvent::ActionStarted { action_type, .. }
                if action_type == "file_write" || action_type == "file_edit"
        )),
        "expected a file_write or file_edit tool call, got: {event_log:#?}"
    );
    assert!(
        event_log.iter().any(|event| matches!(
            event,
            AgentEvent::ActionStarted { action_type, .. } if action_type == "execute_command"
        )),
        "expected an execute_command tool call, got: {event_log:#?}"
    );

    let module_path = workspace.join("bubble_sort.py");
    let source = fs::read_to_string(&module_path)
        .with_context(|| format!("expected model to create {}", module_path.display()))?;
    assert!(
        source.contains("def bubble_sort"),
        "bubble_sort.py should define bubble_sort(values), got:\n{source}"
    );

    verify_bubble_sort_with_python(workspace, python)?;
    Ok(())
}

fn live_run_metrics(
    conversation_id: &str,
    workspace: &PathBuf,
    events: &[AgentEvent],
    model_turn_counter: &AtomicUsize,
) -> serde_json::Value {
    let mut tool_counts = BTreeMap::<String, usize>::new();
    let mut completed_tool_call_count = 0usize;
    let mut errored_tool_call_count = 0usize;
    let mut outer_turn_count = 0usize;
    let mut completed_outer_turn_count = 0usize;
    let mut token_usage = None;

    for event in events {
        match event {
            AgentEvent::TurnStarted { .. } => outer_turn_count += 1,
            AgentEvent::TurnCompleted {
                token_usage: usage, ..
            } => {
                completed_outer_turn_count += 1;
                token_usage = usage.as_ref().map(|usage| {
                    json!({
                        "input": usage.input,
                        "output": usage.output,
                        "reasoning": usage.reasoning,
                        "cache_read": usage.cache_read,
                        "cache_write": usage.cache_write,
                        "cost_usd": usage.cost_usd,
                    })
                });
            }
            AgentEvent::ActionStarted { action_type, .. } => {
                *tool_counts.entry(action_type.clone()).or_default() += 1;
            }
            AgentEvent::ActionCompleted { is_error, .. } => {
                completed_tool_call_count += 1;
                if *is_error {
                    errored_tool_call_count += 1;
                }
            }
            _ => {}
        }
    }

    let tool_call_count: usize = tool_counts.values().sum();
    json!({
        "conversation_id": conversation_id,
        "workspace": workspace,
        "outer_turn_count": outer_turn_count,
        "completed_outer_turn_count": completed_outer_turn_count,
        "model_turn_count": model_turn_counter.load(Ordering::SeqCst),
        "tool_call_count": tool_call_count,
        "completed_tool_call_count": completed_tool_call_count,
        "errored_tool_call_count": errored_tool_call_count,
        "tool_counts": tool_counts,
        "token_usage": token_usage,
    })
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY; run with `cargo test -p panes-agent --test live_llm live_openai_compatible_smoke -- --ignored --nocapture`"]
async fn live_openai_compatible_smoke() -> anyhow::Result<()> {
    load_local_dotenv();
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let client = OpenAiCompatibleClient::from_env(
        "openai",
        model,
        None,
        Some("OPENAI_API_KEY".to_string()),
    )?
    .with_tool_specs(Vec::new());
    let text = collect_text(client).await?;

    assert!(
        text.contains(SENTINEL),
        "expected `{SENTINEL}` in live OpenAI-compatible response, got: {text:?}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY and Python; run with `cargo test -p panes-agent --test live_llm live_openai_compatible_tool_workdir_bubble_sort -- --ignored --nocapture`"]
async fn live_openai_compatible_tool_workdir_bubble_sort() -> anyhow::Result<()> {
    load_local_dotenv();
    let python =
        python_command().context("Python was not found; tried python, python3, and py -3")?;
    let workspace = temp_workspace("panes-agent-live-openai-bubble-sort")?;
    fs::create_dir_all(&workspace).with_context(|| {
        format!(
            "failed to create live tool workspace {}",
            workspace.display()
        )
    })?;

    println!("live workspace: {}", workspace.display());
    run_live_openai_compatible_bubble_sort(&workspace, &python).await
}

async fn run_live_openai_compatible_bubble_sort(
    workspace: &PathBuf,
    python: &PythonCommand,
) -> anyhow::Result<()> {
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let client = OpenAiCompatibleClient::from_env(
        "openai",
        model,
        None,
        Some("OPENAI_API_KEY".to_string()),
    )?;
    run_live_bubble_sort(
        "live-openai-compatible-tool-workdir-bubble-sort",
        client,
        workspace,
        python,
    )
    .await
}

#[tokio::test]
#[ignore = "requires OPENROUTER_API_KEY; run with `cargo test -p panes-agent --test live_llm live_openrouter_smoke -- --ignored --nocapture`"]
async fn live_openrouter_smoke() -> anyhow::Result<()> {
    load_local_dotenv();
    let model =
        std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "openai/gpt-4o-mini".to_string());
    let client = OpenAiCompatibleClient::from_env(
        "openrouter",
        model,
        None,
        Some("OPENROUTER_API_KEY".to_string()),
    )?
    .with_tool_specs(Vec::new());
    let text = collect_text(client).await?;

    assert!(
        text.contains(SENTINEL),
        "expected `{SENTINEL}` in live OpenRouter response, got: {text:?}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires local Ollama OpenAI-compatible endpoint; run with `cargo test -p panes-agent --test live_llm live_ollama_smoke -- --ignored --nocapture`"]
async fn live_ollama_smoke() -> anyhow::Result<()> {
    load_local_dotenv();
    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string());
    let client =
        OpenAiCompatibleClient::from_env("ollama", model, None, None)?.with_tool_specs(Vec::new());
    let text = collect_text(client).await?;

    assert!(
        text.contains(SENTINEL),
        "expected `{SENTINEL}` in live Ollama response, got: {text:?}"
    );
    Ok(())
}

fn load_local_dotenv() {
    if let Ok(cwd) = std::env::current_dir() {
        env_files::load_dotenv_for_dir(&cwd);
    }
}

async fn collect_text<C>(client: C) -> anyhow::Result<String>
where
    C: ModelClient,
{
    let mut stream = client
        .stream(ModelRequest {
            conversation_id: "live-llm-smoke".to_string(),
            messages: vec![AgentMessage::user(format!(
                "Reply with exactly `{SENTINEL}` and no other text."
            ))],
            system_context: SystemContext::new(None),
        })
        .await?;
    let mut text = String::new();
    while let Some(event) = stream.next().await {
        match event {
            ModelStreamEvent::TextDelta(delta) => text.push_str(&delta),
            ModelStreamEvent::Error(message) => anyhow::bail!(message),
            ModelStreamEvent::Done => break,
            _ => {}
        }
    }
    Ok(text)
}

#[derive(Debug, Clone)]
struct PythonCommand {
    program: String,
    prefix_args: Vec<String>,
}

fn python_command() -> Option<PythonCommand> {
    let candidates: Vec<(&str, Vec<&str>)> = if cfg!(windows) {
        vec![("python", vec![]), ("python3", vec![]), ("py", vec!["-3"])]
    } else {
        vec![("python3", vec![]), ("python", vec![])]
    };

    candidates.into_iter().find_map(|(program, prefix_args)| {
        let available = Command::new(program)
            .args(&prefix_args)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        available.then(|| PythonCommand {
            program: program.to_string(),
            prefix_args: prefix_args.into_iter().map(str::to_string).collect(),
        })
    })
}

fn verify_bubble_sort_with_python(
    workspace: &PathBuf,
    python: &PythonCommand,
) -> anyhow::Result<()> {
    let script = r#"
from bubble_sort import bubble_sort
data = [5, 1, 4, 2, 8]
result = bubble_sort(data)
assert result == [1, 2, 4, 5, 8], result
assert data == [5, 1, 4, 2, 8], data
assert bubble_sort([]) == []
assert bubble_sort([1]) == [1]
print("PY_BUBBLE_SORT_OK")
"#;
    let mut command = Command::new(&python.program);
    command
        .args(&python.prefix_args)
        .arg("-c")
        .arg(script)
        .current_dir(workspace);
    let output = command
        .output()
        .with_context(|| format!("failed to run {:?}", python))?;

    if !output.status.success() {
        anyhow::bail!(
            "Python verification failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("PY_BUBBLE_SORT_OK"),
        "expected Python verification sentinel, stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

fn temp_workspace(prefix: &str) -> anyhow::Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?
        .as_nanos();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(|crates_dir| crates_dir.parent())
        .context("failed to resolve workspace root from CARGO_MANIFEST_DIR")?
        .join("temp-dir");
    fs::create_dir_all(&root)
        .with_context(|| format!("failed to create temp root {}", root.display()))?;
    Ok(root.join(format!("{prefix}-{timestamp}")))
}
