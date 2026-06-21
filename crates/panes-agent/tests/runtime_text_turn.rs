use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use panes_agent::{
    application::ports::{PermissionGateway, ToolExecutor},
    domain::{
        budget::TokenBudget,
        conversation::{AgentMessage, MessageContent},
        permission::{PermissionDecision, PermissionRequest},
        structured_output::StructuredOutputContract,
    },
    infrastructure::{
        native_tools::NativeToolExecutor,
        testing::{RecordingEventSink, ScriptedModelClient, StaticModelClient, StaticToolExecutor},
    },
    AgentEvent, AgentOutcome, AgentRuntime, AgentRuntimePorts, ModelStreamEvent, RunTurnCommand,
    SystemContext, TokenUsage, ToolCall,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;

struct TestPorts {
    model: StaticModelClient,
    events: RecordingEventSink,
    tools: StaticToolExecutor,
}

#[tokio::test]
async fn runtime_blocks_model_call_when_input_budget_is_exceeded() {
    let events = RecordingEventSink::default();
    let model = ScriptedModelClient::new(vec![vec![ModelStreamEvent::TextDelta(
        "should not run".to_string(),
    )]]);
    let runtime = AgentRuntime::new(ToolLoopPorts {
        model: model.clone(),
        events: events.clone(),
        tools: StaticToolExecutor::text("unused"),
    });
    let mut system_context = SystemContext::new(Some("C:/codes/panes".to_string()));
    system_context.token_budget = Some(TokenBudget {
        max_input_tokens: Some(1),
        ..TokenBudget::default()
    });

    let error = runtime
        .run_turn(RunTurnCommand {
            conversation_id: "thread-budget".to_string(),
            messages: vec![AgentMessage::user("this prompt is too long for one token")],
            system_context,
            cancellation: CancellationToken::new(),
        })
        .await
        .expect_err("budget should block model call");

    assert!(error.to_string().contains("input token budget exceeded"));
    assert!(model.requests().is_empty());
    assert!(events.events().iter().any(|event| {
        matches!(
            event,
            AgentEvent::Error {
                recoverable: false,
                ..
            }
        )
    }));
}

#[tokio::test]
async fn runtime_rejects_invalid_structured_output_json() {
    let events = RecordingEventSink::default();
    let runtime = AgentRuntime::new(ToolLoopPorts {
        model: ScriptedModelClient::new(vec![vec![
            ModelStreamEvent::TextDelta("not json".to_string()),
            ModelStreamEvent::Done,
        ]]),
        events: events.clone(),
        tools: StaticToolExecutor::text("unused"),
    });
    let mut system_context = SystemContext::new(Some("C:/codes/panes".to_string()));
    system_context.structured_output = Some(StructuredOutputContract::json_schema(
        "answer",
        json!({ "type": "object" }),
    ));

    let error = runtime
        .run_turn(RunTurnCommand {
            conversation_id: "thread-structured".to_string(),
            messages: vec![AgentMessage::user("answer as json")],
            system_context,
            cancellation: CancellationToken::new(),
        })
        .await
        .expect_err("invalid JSON should fail structured output");

    assert!(error
        .to_string()
        .contains("structured output `answer` was not valid JSON"));
}

impl AgentRuntimePorts for TestPorts {
    type Model = StaticModelClient;
    type Events = RecordingEventSink;
    type Tools = StaticToolExecutor;

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
async fn runtime_streams_a_text_turn_through_the_public_facade() {
    let events = RecordingEventSink::default();
    let runtime = AgentRuntime::new(TestPorts {
        model: StaticModelClient::text("hello from claurst-native"),
        events: events.clone(),
        tools: StaticToolExecutor::text("unused"),
    });

    let outcome = runtime
        .run_turn(RunTurnCommand {
            conversation_id: "thread-1".to_string(),
            messages: vec![AgentMessage::user("hello")],
            system_context: SystemContext::new(Some("C:/codes/panes".to_string())),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("turn should complete");

    assert_eq!(
        outcome,
        AgentOutcome {
            assistant_text: "hello from claurst-native".to_string(),
        }
    );
    let event_log = events.events();
    assert!(event_log.contains(&AgentEvent::TurnStarted {
        conversation_id: "thread-1".to_string(),
    }));
    assert!(event_log.contains(&AgentEvent::TextDelta {
        content: "hello from claurst-native".to_string(),
    }));
    assert!(event_log.iter().any(|event| matches!(
        event,
        AgentEvent::TurnCompleted { token_usage: None, metrics }
            if metrics.model_turn_count == 1 && metrics.tool_call_count == 0
    )));
}

struct NativeToolPorts {
    model: ScriptedModelClient,
    events: RecordingEventSink,
    tools: NativeToolExecutor,
}

impl AgentRuntimePorts for NativeToolPorts {
    type Model = ScriptedModelClient;
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
async fn runtime_reads_a_workspace_file_with_the_native_tool_executor() {
    let workspace = temp_workspace("panes-agent-read-file");
    fs::create_dir_all(&workspace).expect("workspace should be created");
    fs::write(workspace.join("README.md"), "CueLight native runtime\n")
        .expect("fixture file should be written");

    let events = RecordingEventSink::default();
    let model = ScriptedModelClient::new(vec![
        vec![
            ModelStreamEvent::ToolUse(ToolCall {
                id: "tool-read".to_string(),
                name: "read_file".to_string(),
                input: json!({ "path": "README.md" }),
            }),
            ModelStreamEvent::Done,
        ],
        vec![
            ModelStreamEvent::TextDelta("read file complete".to_string()),
            ModelStreamEvent::Done,
        ],
    ]);
    let runtime = AgentRuntime::new(NativeToolPorts {
        model: model.clone(),
        events: events.clone(),
        tools: NativeToolExecutor::new(workspace.clone()),
    });

    let outcome = runtime
        .run_turn(RunTurnCommand {
            conversation_id: "thread-native-tools".to_string(),
            messages: vec![AgentMessage::user("read the readme")],
            system_context: SystemContext::new(Some(workspace.to_string_lossy().into_owned())),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("turn should complete after native read_file");

    assert_eq!(
        outcome,
        AgentOutcome {
            assistant_text: "read file complete".to_string(),
        }
    );
    assert!(events.events().contains(&AgentEvent::ActionCompleted {
        action_id: "tool-read".to_string(),
        output: "CueLight native runtime\n".to_string(),
        is_error: false,
    }));

    let requests = model.requests();
    assert_eq!(
        requests[1].messages.last().map(|message| &message.content),
        Some(&vec![MessageContent::ToolResult {
            tool_use_id: "tool-read".to_string(),
            content: "CueLight native runtime\n".to_string(),
            is_error: false,
        }])
    );

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn runtime_lists_workspace_files_with_the_native_tool_executor() {
    let workspace = temp_workspace("panes-agent-list-files");
    fs::create_dir_all(workspace.join("assets")).expect("workspace directory should be created");
    fs::write(workspace.join("README.md"), "readme").expect("readme should be written");
    fs::write(workspace.join("script.txt"), "script").expect("script should be written");

    let events = RecordingEventSink::default();
    let model = ScriptedModelClient::new(vec![
        vec![
            ModelStreamEvent::ToolUse(ToolCall {
                id: "tool-list".to_string(),
                name: "list_files".to_string(),
                input: json!({ "path": "." }),
            }),
            ModelStreamEvent::Done,
        ],
        vec![
            ModelStreamEvent::TextDelta("listed files".to_string()),
            ModelStreamEvent::Done,
        ],
    ]);
    let runtime = AgentRuntime::new(NativeToolPorts {
        model: model.clone(),
        events: events.clone(),
        tools: NativeToolExecutor::new(workspace.clone()),
    });

    let outcome = runtime
        .run_turn(RunTurnCommand {
            conversation_id: "thread-list-files".to_string(),
            messages: vec![AgentMessage::user("list files")],
            system_context: SystemContext::new(Some(workspace.to_string_lossy().into_owned())),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("turn should complete after native list_files");

    assert_eq!(
        outcome,
        AgentOutcome {
            assistant_text: "listed files".to_string(),
        }
    );
    assert!(events.events().contains(&AgentEvent::ActionCompleted {
        action_id: "tool-list".to_string(),
        output: "README.md\nassets/\nscript.txt\n".to_string(),
        is_error: false,
    }));

    let requests = model.requests();
    assert_eq!(
        requests[1].messages.last().map(|message| &message.content),
        Some(&vec![MessageContent::ToolResult {
            tool_use_id: "tool-list".to_string(),
            content: "README.md\nassets/\nscript.txt\n".to_string(),
            is_error: false,
        }])
    );

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn runtime_searches_workspace_text_with_the_native_tool_executor() {
    let workspace = temp_workspace("panes-agent-search");
    fs::create_dir_all(workspace.join("notes")).expect("workspace directory should be created");
    fs::write(
        workspace.join("README.md"),
        "CueLight native runtime\nplain line\n",
    )
    .expect("readme should be written");
    fs::write(
        workspace.join("notes").join("scene.txt"),
        "opening\nCueLight scene marker\n",
    )
    .expect("scene should be written");
    fs::write(workspace.join("notes").join("other.txt"), "no match\n")
        .expect("other file should be written");

    let events = RecordingEventSink::default();
    let model = ScriptedModelClient::new(vec![
        vec![
            ModelStreamEvent::ToolUse(ToolCall {
                id: "tool-search".to_string(),
                name: "search".to_string(),
                input: json!({ "path": ".", "pattern": "CueLight" }),
            }),
            ModelStreamEvent::Done,
        ],
        vec![
            ModelStreamEvent::TextDelta("search complete".to_string()),
            ModelStreamEvent::Done,
        ],
    ]);
    let runtime = AgentRuntime::new(NativeToolPorts {
        model: model.clone(),
        events: events.clone(),
        tools: NativeToolExecutor::new(workspace.clone()),
    });

    let outcome = runtime
        .run_turn(RunTurnCommand {
            conversation_id: "thread-search".to_string(),
            messages: vec![AgentMessage::user("find CueLight references")],
            system_context: SystemContext::new(Some(workspace.to_string_lossy().into_owned())),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("turn should complete after native search");

    let expected = "README.md:1:CueLight native runtime\nnotes/scene.txt:2:CueLight scene marker\n";
    assert_eq!(
        outcome,
        AgentOutcome {
            assistant_text: "search complete".to_string(),
        }
    );
    assert!(events.events().contains(&AgentEvent::ActionCompleted {
        action_id: "tool-search".to_string(),
        output: expected.to_string(),
        is_error: false,
    }));

    let requests = model.requests();
    assert_eq!(
        requests[1].messages.last().map(|message| &message.content),
        Some(&vec![MessageContent::ToolResult {
            tool_use_id: "tool-search".to_string(),
            content: expected.to_string(),
            is_error: false,
        }])
    );

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn native_glob_and_grep_find_workspace_matches() {
    let workspace = temp_workspace("panes-agent-glob-grep");
    fs::create_dir_all(workspace.join("src")).expect("workspace directory should be created");
    fs::create_dir_all(workspace.join("node_modules")).expect("ignored directory should exist");
    fs::write(workspace.join("src").join("lib.rs"), "CueLight marker\n")
        .expect("lib should be written");
    fs::write(workspace.join("src").join("main.rs"), "plain\n").expect("main should be written");
    fs::write(
        workspace.join("node_modules").join("ignored.rs"),
        "CueLight ignored\n",
    )
    .expect("ignored should be written");
    let executor = NativeToolExecutor::new(workspace.clone());

    let glob_result = execute_native_tool(
        &executor,
        "tool-glob",
        "glob",
        json!({ "pattern": "src/*.rs" }),
    )
    .await;
    assert_eq!(glob_result.content, "src/lib.rs\nsrc/main.rs\n");

    let grep_result = execute_native_tool(
        &executor,
        "tool-grep",
        "grep",
        json!({ "pattern": "cuelight", "path": ".", "case_sensitive": false, "path_glob": "src/*.rs" }),
    )
    .await;
    assert_eq!(grep_result.content, "src/lib.rs:1:CueLight marker\n");

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn native_file_write_creates_parent_directories_and_rejects_escaping_paths() {
    let workspace = temp_workspace("panes-agent-write-file");
    fs::create_dir_all(&workspace).expect("workspace should be created");
    let executor = NativeToolExecutor::new(workspace.clone());

    let write_result = execute_native_tool(
        &executor,
        "tool-write",
        "file_write",
        json!({ "path": "drafts/scene.md", "content": "CueLight scene\n" }),
    )
    .await;
    assert_eq!(write_result.tool_use_id, "tool-write");
    assert!(!write_result.is_error, "{write_result:?}");
    assert_eq!(
        fs::read_to_string(workspace.join("drafts").join("scene.md"))
            .expect("written file should exist"),
        "CueLight scene\n"
    );

    let escaping_result = execute_native_tool(
        &executor,
        "tool-write-escape",
        "file_write",
        json!({ "path": "../outside.md", "content": "outside" }),
    )
    .await;
    assert!(escaping_result.is_error);
    assert_eq!(
        escaping_result.content,
        "file_write path escapes workspace root"
    );

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn native_file_edit_replaces_once_and_reports_missing_old_text() {
    let workspace = temp_workspace("panes-agent-edit-file");
    fs::create_dir_all(&workspace).expect("workspace should be created");
    fs::write(workspace.join("outline.md"), "beat\nbeat\n")
        .expect("fixture file should be written");
    let executor = NativeToolExecutor::new(workspace.clone());

    let duplicate_result = execute_native_tool(
        &executor,
        "tool-edit",
        "file_edit",
        json!({ "path": "outline.md", "old_text": "beat", "new_text": "scene" }),
    )
    .await;
    assert!(duplicate_result.is_error);
    assert_eq!(
        duplicate_result.content,
        "file_edit old_text must match exactly once; found 2"
    );

    let edit_result = execute_native_tool(
        &executor,
        "tool-edit-all",
        "file_edit",
        json!({ "path": "outline.md", "old_text": "beat", "new_text": "scene", "replace_all": true }),
    )
    .await;
    assert_eq!(edit_result.tool_use_id, "tool-edit-all");
    assert!(!edit_result.is_error, "{edit_result:?}");
    assert_eq!(
        fs::read_to_string(workspace.join("outline.md")).expect("edited file should exist"),
        "scene\nscene\n"
    );

    let missing_result = execute_native_tool(
        &executor,
        "tool-edit-missing",
        "file_edit",
        json!({ "path": "outline.md", "old_text": "missing", "new_text": "scene" }),
    )
    .await;
    assert!(missing_result.is_error);
    assert_eq!(missing_result.content, "file_edit old_text not found");
    assert_eq!(
        fs::read_to_string(workspace.join("outline.md")).expect("edited file should exist"),
        "scene\nscene\n"
    );

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn native_task_management_runs_lifecycle_operations() {
    let workspace = temp_workspace("panes-agent-task-management");
    fs::create_dir_all(&workspace).expect("workspace should be created");
    let executor = NativeToolExecutor::new(workspace.clone());

    let created = execute_native_tool(
        &executor,
        "tool-task-create",
        "task_management",
        json!({
            "operation": "create",
            "subject": "Outline first act",
            "description": "Create CueLight story beats"
        }),
    )
    .await;
    assert!(!created.is_error, "{created:?}");
    let created_task: serde_json::Value =
        serde_json::from_str(&created.content).expect("created task should be json");
    assert_eq!(created_task["id"], "task_1");
    assert_eq!(created_task["status"], "open");

    let updated = execute_native_tool(
        &executor,
        "tool-task-update",
        "task_management",
        json!({
            "operation": "update",
            "task_id": "task_1",
            "status": "in_progress"
        }),
    )
    .await;
    assert!(!updated.is_error, "{updated:?}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&updated.content)
            .expect("updated task should be json")["status"],
        "in_progress"
    );

    let completed = execute_native_tool(
        &executor,
        "tool-task-complete",
        "task_management",
        json!({ "operation": "complete", "task_id": "task_1" }),
    )
    .await;
    assert!(!completed.is_error, "{completed:?}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&completed.content)
            .expect("completed task should be json")["status"],
        "completed"
    );

    let listed = execute_native_tool(
        &executor,
        "tool-task-list",
        "task_management",
        json!({ "operation": "list" }),
    )
    .await;
    assert!(!listed.is_error, "{listed:?}");
    let listed_tasks: Vec<serde_json::Value> =
        serde_json::from_str(&listed.content).expect("listed tasks should be json");
    assert_eq!(listed_tasks.len(), 1);
    assert_eq!(listed_tasks[0]["id"], "task_1");

    let deleted = execute_native_tool(
        &executor,
        "tool-task-delete",
        "task_management",
        json!({ "operation": "delete", "task_id": "task_1" }),
    )
    .await;
    assert!(!deleted.is_error, "{deleted:?}");
    assert_eq!(deleted.content, "deleted task_1");

    let missing = execute_native_tool(
        &executor,
        "tool-task-missing",
        "task_management",
        json!({ "operation": "get", "task_id": "task_1" }),
    )
    .await;
    assert!(missing.is_error);
    assert_eq!(missing.content, "task not found: task_1");

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn native_batch_edit_applies_multiple_replacements_atomically() {
    let workspace = temp_workspace("panes-agent-batch-edit");
    fs::create_dir_all(&workspace).expect("workspace should be created");
    fs::write(workspace.join("a.txt"), "alpha\nbeta\n").expect("fixture a");
    fs::write(workspace.join("b.txt"), "alpha\n").expect("fixture b");
    let executor = NativeToolExecutor::new(workspace.clone());

    let result = execute_native_tool(
        &executor,
        "tool-batch",
        "batch_edit",
        json!({
            "edits": [
                { "path": "a.txt", "old_text": "beta", "new_text": "gamma" },
                { "path": "b.txt", "old_text": "alpha", "new_text": "delta" }
            ]
        }),
    )
    .await;
    assert!(!result.is_error, "{result:?}");
    assert_eq!(
        fs::read_to_string(workspace.join("a.txt")).unwrap(),
        "alpha\ngamma\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("b.txt")).unwrap(),
        "delta\n"
    );

    let failed = execute_native_tool(
        &executor,
        "tool-batch-fail",
        "batch_edit",
        json!({
            "edits": [
                { "path": "a.txt", "old_text": "missing", "new_text": "nope" },
                { "path": "b.txt", "old_text": "delta", "new_text": "changed" }
            ]
        }),
    )
    .await;
    assert!(failed.is_error);
    assert_eq!(
        fs::read_to_string(workspace.join("b.txt")).unwrap(),
        "delta\n"
    );

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn native_skill_tool_loads_catalog_entry() {
    let workspace = temp_workspace("panes-agent-skill-tool");
    fs::create_dir_all(&workspace).expect("workspace should be created");
    let skill = panes_agent::domain::skills::SkillDefinition {
        name: "review".to_string(),
        path: "review/SKILL.md".to_string(),
        description: Some("Review code".to_string()),
        prompt: "Check correctness.".to_string(),
        source: panes_agent::domain::skills::SkillSource::Workspace,
    };
    let executor = NativeToolExecutor::new(workspace.clone()).with_skills(vec![skill]);

    let result = execute_native_tool(
        &executor,
        "tool-skill",
        "skill",
        json!({ "name": "review", "args": "src/lib.rs" }),
    )
    .await;
    assert!(!result.is_error, "{result:?}");
    assert!(result.content.contains("Check correctness."));
    assert!(result.content.contains("src/lib.rs"));

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn native_execute_command_background_can_be_monitored() {
    let workspace = temp_workspace("panes-agent-background-command");
    fs::create_dir_all(&workspace).expect("workspace should be created");
    let executor = NativeToolExecutor::new(workspace.clone());
    let command = if cfg!(windows) {
        "Start-Sleep -Milliseconds 50; Write-Output done"
    } else {
        "sleep 0.05; echo done"
    };

    let started = execute_native_tool(
        &executor,
        "tool-bg",
        "execute_command",
        json!({ "command": command, "run_in_background": true, "timeout_ms": 5000 }),
    )
    .await;
    assert!(!started.is_error, "{started:?}");
    let value: serde_json::Value = serde_json::from_str(&started.content).expect("json result");
    let task_id = value
        .get("task_id")
        .and_then(serde_json::Value::as_str)
        .expect("task id");

    let mut output = String::new();
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(25)).await;
        let status = execute_native_tool(
            &executor,
            "tool-monitor",
            "monitor",
            json!({ "action": "output", "task_id": task_id }),
        )
        .await;
        output = status.content;
        if output.contains("completed") {
            break;
        }
    }
    assert!(output.contains("done"), "{output}");

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn runtime_times_out_slow_execute_command_with_the_native_tool_executor() {
    let workspace = temp_workspace("panes-agent-command-timeout");
    fs::create_dir_all(&workspace).expect("workspace should be created");

    let events = RecordingEventSink::default();
    let model = ScriptedModelClient::new(vec![
        vec![
            ModelStreamEvent::ToolUse(ToolCall {
                id: "tool-command".to_string(),
                name: "execute_command".to_string(),
                input: json!({ "command": slow_command() }),
            }),
            ModelStreamEvent::Done,
        ],
        vec![
            ModelStreamEvent::TextDelta("command handled".to_string()),
            ModelStreamEvent::Done,
        ],
    ]);
    let runtime = AgentRuntime::new(NativeToolPorts {
        model,
        events: events.clone(),
        tools: NativeToolExecutor::new(workspace.clone())
            .with_command_timeout(Duration::from_millis(50)),
    });

    let started = Instant::now();
    let outcome = runtime
        .run_turn(RunTurnCommand {
            conversation_id: "thread-command-timeout".to_string(),
            messages: vec![AgentMessage::user("run a slow command")],
            system_context: SystemContext::new(Some(workspace.to_string_lossy().into_owned())),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("turn should complete after command timeout");

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "execute_command should return on the configured timeout"
    );
    assert_eq!(
        outcome,
        AgentOutcome {
            assistant_text: "command handled".to_string(),
        }
    );
    assert!(events.events().contains(&AgentEvent::ActionCompleted {
        action_id: "tool-command".to_string(),
        output: "execute_command timed out".to_string(),
        is_error: true,
    }));

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn runtime_denies_execute_command_before_spawning_the_process() {
    let workspace = temp_workspace("panes-agent-command-denied");
    fs::create_dir_all(&workspace).expect("workspace should be created");

    let events = RecordingEventSink::default();
    let model = ScriptedModelClient::new(vec![
        vec![
            ModelStreamEvent::ToolUse(ToolCall {
                id: "tool-command-denied".to_string(),
                name: "execute_command".to_string(),
                input: json!({ "command": denied_side_effect_command() }),
            }),
            ModelStreamEvent::Done,
        ],
        vec![
            ModelStreamEvent::TextDelta("denied handled".to_string()),
            ModelStreamEvent::Done,
        ],
    ]);
    let runtime = AgentRuntime::new(NativeToolPorts {
        model,
        events: events.clone(),
        tools: NativeToolExecutor::with_permissions(workspace.clone(), Arc::new(DenyPermission)),
    });

    let outcome = runtime
        .run_turn(RunTurnCommand {
            conversation_id: "thread-command-denied".to_string(),
            messages: vec![AgentMessage::user("run a denied command")],
            system_context: SystemContext::new(Some(workspace.to_string_lossy().into_owned())),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("turn should complete after command denial");

    assert_eq!(
        outcome,
        AgentOutcome {
            assistant_text: "denied handled".to_string(),
        }
    );
    assert!(events.events().contains(&AgentEvent::ActionCompleted {
        action_id: "tool-command-denied".to_string(),
        output: "execute_command denied by permission gateway".to_string(),
        is_error: true,
    }));
    assert!(
        !workspace.join("denied.txt").exists(),
        "denied command should not run side effects"
    );

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

#[tokio::test]
async fn native_execute_command_kills_running_process_when_cancelled() {
    let workspace = temp_workspace("panes-agent-command-cancelled");
    fs::create_dir_all(&workspace).expect("workspace should be created");

    let executor = NativeToolExecutor::new(workspace.clone());
    let cancellation = CancellationToken::new();
    let command_cancellation = cancellation.clone();
    let started = Instant::now();
    let task = tokio::spawn(async move {
        executor
            .execute(
                ToolCall {
                    id: "tool-command-cancelled".to_string(),
                    name: "execute_command".to_string(),
                    input: json!({ "command": slow_command() }),
                },
                &command_cancellation,
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    cancellation.cancel();

    let result = task
        .await
        .expect("command task should join")
        .expect("command should return a tool result");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "execute_command should stop promptly when cancelled"
    );
    assert_eq!(result.tool_use_id, "tool-command-cancelled");
    assert_eq!(result.content, "execute_command cancelled");
    assert!(result.is_error);

    fs::remove_dir_all(workspace).expect("workspace should be removed");
}

struct DenyPermission;

#[async_trait]
impl PermissionGateway for DenyPermission {
    async fn request(&self, request: PermissionRequest) -> anyhow::Result<PermissionDecision> {
        assert_eq!(request.action_type, "execute_command");
        Ok(PermissionDecision::Deny)
    }
}

#[cfg(windows)]
fn slow_command() -> &'static str {
    "Start-Sleep -Seconds 5; Write-Output done"
}

#[cfg(not(windows))]
fn slow_command() -> &'static str {
    "sleep 5; echo done"
}

#[cfg(windows)]
fn denied_side_effect_command() -> &'static str {
    "Set-Content -Path denied.txt -Value ran"
}

#[cfg(not(windows))]
fn denied_side_effect_command() -> &'static str {
    "printf ran > denied.txt"
}

async fn execute_native_tool(
    executor: &NativeToolExecutor,
    id: &str,
    name: &str,
    input: serde_json::Value,
) -> panes_agent::ToolResult {
    let cancellation = CancellationToken::new();
    executor
        .execute(
            ToolCall {
                id: id.to_string(),
                name: name.to_string(),
                input,
            },
            &cancellation,
        )
        .await
        .expect("native tool should return a result")
}

fn temp_workspace(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}"))
}

struct ToolLoopPorts {
    model: ScriptedModelClient,
    events: RecordingEventSink,
    tools: StaticToolExecutor,
}

impl AgentRuntimePorts for ToolLoopPorts {
    type Model = ScriptedModelClient;
    type Events = RecordingEventSink;
    type Tools = StaticToolExecutor;

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
async fn runtime_executes_a_tool_call_and_continues_the_turn() {
    let events = RecordingEventSink::default();
    let model = ScriptedModelClient::new(vec![
        vec![
            ModelStreamEvent::ToolUse(ToolCall {
                id: "tool-1".to_string(),
                name: "read_file".to_string(),
                input: json!({ "path": "README.md" }),
            }),
            ModelStreamEvent::Done,
        ],
        vec![
            ModelStreamEvent::TextDelta("README summary".to_string()),
            ModelStreamEvent::Done,
        ],
    ]);
    let runtime = AgentRuntime::new(ToolLoopPorts {
        model: model.clone(),
        events: events.clone(),
        tools: StaticToolExecutor::text("readme contents"),
    });

    let outcome = runtime
        .run_turn(RunTurnCommand {
            conversation_id: "thread-tools".to_string(),
            messages: vec![AgentMessage::user("summarize the readme")],
            system_context: SystemContext::new(Some("C:/codes/panes".to_string())),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("turn should complete after tool use");

    assert_eq!(
        outcome,
        AgentOutcome {
            assistant_text: "README summary".to_string(),
        }
    );
    let event_log = events.events();
    assert!(event_log.contains(&AgentEvent::ActionStarted {
        action_id: "tool-1".to_string(),
        action_type: "read_file".to_string(),
        input: json!({ "path": "README.md" }),
    }));
    assert!(event_log.contains(&AgentEvent::ActionCompleted {
        action_id: "tool-1".to_string(),
        output: "readme contents".to_string(),
        is_error: false,
    }));
    assert!(event_log.iter().any(|event| matches!(
        event,
        AgentEvent::TurnCompleted { metrics, .. }
            if metrics.model_turn_count == 2 && metrics.tool_call_count == 1
    )));

    let requests = model.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].messages.last().map(|message| &message.content),
        Some(&vec![MessageContent::ToolResult {
            tool_use_id: "tool-1".to_string(),
            content: "readme contents".to_string(),
            is_error: false,
        }])
    );
}

#[tokio::test]
async fn runtime_forwards_thinking_and_reports_turn_token_usage() {
    let events = RecordingEventSink::default();
    let model = ScriptedModelClient::new(vec![vec![
        ModelStreamEvent::Usage(TokenUsage {
            input: 12,
            output: 0,
            reasoning: None,
            cache_read: Some(3),
            cache_write: Some(4),
            cost_usd: Some(0.0000519),
        }),
        ModelStreamEvent::ThinkingDelta("checking outline".to_string()),
        ModelStreamEvent::TextDelta("done".to_string()),
        ModelStreamEvent::Usage(TokenUsage {
            input: 0,
            output: 7,
            reasoning: Some(2),
            cache_read: None,
            cache_write: None,
            cost_usd: Some(0.000105),
        }),
        ModelStreamEvent::Done,
    ]]);
    let runtime = AgentRuntime::new(ToolLoopPorts {
        model,
        events: events.clone(),
        tools: StaticToolExecutor::text("unused"),
    });

    runtime
        .run_turn(RunTurnCommand {
            conversation_id: "thread-usage".to_string(),
            messages: vec![AgentMessage::user("summarize")],
            system_context: SystemContext::new(Some("C:/codes/panes".to_string())),
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("turn should complete");

    let event_log = events.events();
    assert!(event_log.contains(&AgentEvent::ThinkingDelta {
        content: "checking outline".to_string(),
    }));
    assert!(event_log.contains(&AgentEvent::TextDelta {
        content: "done".to_string(),
    }));
    assert!(event_log.iter().any(|event| matches!(
        event,
        AgentEvent::TurnCompleted { token_usage: Some(usage), metrics }
            if usage.input == 12
                && usage.output == 7
                && usage.reasoning == Some(2)
                && usage.cache_read == Some(3)
                && usage.cache_write == Some(4)
                && usage.cost_usd == Some(0.0001569)
                && metrics.model_turn_count == 1
    )));
}
