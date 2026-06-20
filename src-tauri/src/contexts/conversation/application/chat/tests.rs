use std::{fs, sync::Arc};

use super::*;
use crate::{
    config::app_config::AppConfig,
    db,
    engines::EngineManager,
    git::{repo::FileTreeCache, watcher::GitWatcherManager},
    models::{EngineCapabilitiesDto, ReasoningEffortOptionDto},
    power::KeepAwakeManager,
    state::{AppState, TurnManager},
    terminal::TerminalManager,
    terminal_notifications::TerminalNotificationManager,
};
use rusqlite::params;
use uuid::Uuid;

fn test_app_state() -> AppState {
    let root = std::env::temp_dir().join(format!("panes-chat-cmd-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("failed to create temp root");
    let db = crate::db::Database::open(root.join("workspaces.db"))
        .expect("failed to create test database");
    AppState {
        db,
        config: Arc::new(AppConfig::default()),
        config_write_lock: Arc::new(tokio::sync::Mutex::new(())),
        engines: Arc::new(EngineManager::new()),
        git_watchers: Arc::new(GitWatcherManager::default()),
        terminals: Arc::new(TerminalManager::default()),
        notifications: Arc::new(TerminalNotificationManager::default()),
        keep_awake: Arc::new(KeepAwakeManager::new()),
        turns: Arc::new(TurnManager::default()),
        file_tree_cache: Arc::new(FileTreeCache::new()),
    }
}

fn test_thread(state: &AppState, engine_id: &str, model_id: &str) -> ThreadDto {
    let workspace_root =
        std::env::temp_dir().join(format!("panes-chat-workspace-{}", Uuid::new_v4()));
    fs::create_dir_all(&workspace_root).expect("failed to create workspace root");
    let workspace = db::workspaces::upsert_workspace(
        &state.db,
        workspace_root.to_string_lossy().as_ref(),
        Some(1),
    )
    .expect("failed to create workspace");
    db::threads::create_thread(
        &state.db,
        &workspace.id,
        None,
        engine_id,
        model_id,
        "Thread",
    )
    .expect("failed to create thread")
}

fn attachment_validation_catalog(attachment_modalities: Vec<&str>) -> Vec<EngineInfoDto> {
    vec![EngineInfoDto {
        id: "opencode".to_string(),
        name: "OpenCode".to_string(),
        models: vec![EngineModelDto {
            id: "opencode/test".to_string(),
            display_name: "OpenCode Test".to_string(),
            description: String::new(),
            hidden: false,
            is_default: true,
            upgrade: None,
            availability_nux: None,
            upgrade_info: None,
            input_modalities: vec!["text".to_string(), "image".to_string(), "pdf".to_string()],
            attachment_modalities: attachment_modalities
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            limits: None,
            supports_personality: false,
            default_reasoning_effort: "medium".to_string(),
            supported_reasoning_efforts: Vec::new(),
        }],
        capabilities: EngineCapabilitiesDto {
            permission_modes: Vec::new(),
            sandbox_modes: Vec::new(),
            approval_decisions: Vec::new(),
        },
    }]
}

fn test_attachment(file_name: &str, mime_type: Option<&str>) -> TurnAttachment {
    TurnAttachment {
        file_name: file_name.to_string(),
        file_path: format!("/tmp/{file_name}"),
        size_bytes: 1,
        mime_type: mime_type.map(ToOwned::to_owned),
    }
}

#[test]
fn validates_attachments_against_model_attachment_modalities() {
    let catalog = attachment_validation_catalog(vec!["text", "image"]);
    let attachments = vec![
        test_attachment("notes.md", Some("text/markdown")),
        test_attachment("screenshot.png", Some("image/png")),
    ];

    assert!(validate_attachments_for_engine_model(
        &attachments,
        "opencode",
        "opencode/test",
        Some(&catalog),
    )
    .is_ok());
}

#[test]
fn rejects_attachments_when_model_disables_files() {
    let catalog = attachment_validation_catalog(Vec::new());
    let attachments = vec![test_attachment("notes.md", Some("text/markdown"))];

    let error = validate_attachments_for_engine_model(
        &attachments,
        "opencode",
        "opencode/test",
        Some(&catalog),
    )
    .expect_err("model without attachment modalities should reject files");

    assert!(error.contains("does not support file attachments"));
}

#[test]
fn rejects_attachment_modalities_not_allowed_by_model() {
    let catalog = attachment_validation_catalog(vec!["text"]);
    let attachments = vec![test_attachment("diagram.png", Some("image/png"))];

    let error = validate_attachments_for_engine_model(
        &attachments,
        "opencode",
        "opencode/test",
        Some(&catalog),
    )
    .expect_err("image should be blocked for text-only models");

    assert!(error.contains("Image attachments are not supported"));
}

fn insert_pending_approval_with_details(
    state: &AppState,
    thread: &ThreadDto,
    approval_id: &str,
    details: Value,
) -> String {
    let assistant_message = db::messages::insert_assistant_placeholder(
        &state.db,
        &thread.id,
        Some(thread.engine_id.as_str()),
        Some(thread.model_id.as_str()),
        None,
    )
    .expect("failed to create assistant message");
    db::actions::insert_approval(
        &state.db,
        approval_id,
        &thread.id,
        &assistant_message.id,
        &crate::engines::events::ActionType::Command,
        "Run command",
        &details,
    )
    .expect("failed to insert approval");
    db::threads::update_thread_status(&state.db, &thread.id, ThreadStatusDto::AwaitingApproval)
        .expect("failed to set thread status");

    let blocks = serde_json::json!([
        {
            "type": "approval",
            "approvalId": approval_id,
            "actionType": "command",
            "summary": "Run command",
            "details": details,
            "status": "pending"
        }
    ]);
    let conn = state.db.connect().expect("failed to open db connection");
    conn.execute(
        "UPDATE messages SET blocks_json = ?1 WHERE id = ?2",
        params![blocks.to_string(), assistant_message.id],
    )
    .expect("failed to persist approval block");
    assistant_message.id
}

fn insert_pending_approval(state: &AppState, thread: &ThreadDto, approval_id: &str) -> String {
    insert_pending_approval_with_details(
        state,
        thread,
        approval_id,
        serde_json::json!({ "command": "touch file.txt" }),
    )
}

#[test]
fn build_final_thread_event_uses_latest_thread_when_present() {
    let state = test_app_state();
    let fallback_thread = test_thread(&state, "codex", "gpt-5.5-codex");
    let mut latest_thread = fallback_thread.clone();
    latest_thread.title = "Renamed".to_string();

    let (event, final_thread) =
        build_final_thread_event(Some(latest_thread.clone()), &fallback_thread);

    assert_eq!(event.thread_id, latest_thread.id);
    assert_eq!(event.workspace_id, latest_thread.workspace_id);
    assert_eq!(
        event.thread.as_ref().map(|thread| thread.title.as_str()),
        Some("Renamed")
    );
    assert_eq!(
        final_thread.as_ref().map(|thread| thread.title.as_str()),
        Some("Renamed")
    );
}

#[test]
fn build_final_thread_event_emits_removal_when_thread_is_missing() {
    let state = test_app_state();
    let fallback_thread = test_thread(&state, "codex", "gpt-5.5-codex");

    let (event, final_thread) = build_final_thread_event(None, &fallback_thread);

    assert_eq!(event.thread_id, fallback_thread.id);
    assert_eq!(event.workspace_id, fallback_thread.workspace_id);
    assert!(event.thread.is_none());
    assert!(final_thread.is_none());
}

#[test]
fn external_sandbox_allows_default_workspace_write_mode() {
    assert!(!unsupported_thread_sandbox_override_for_external_sandbox(
        None, true,
    ));
}

#[test]
fn normalize_input_items_merges_adjacent_text_and_preserves_typed_items() {
    let normalized = normalize_input_items(
        "fallback",
        Some(vec![
            ChatInputItemPayload::Text {
                text: "Use ".to_string(),
            },
            ChatInputItemPayload::Text {
                text: "$lint".to_string(),
            },
            ChatInputItemPayload::Skill {
                name: "lint".to_string(),
                path: "/tmp/skills/lint".to_string(),
            },
            ChatInputItemPayload::Mention {
                name: "Docs".to_string(),
                path: "app://docs".to_string(),
            },
            ChatInputItemPayload::Text {
                text: " now".to_string(),
            },
        ]),
    )
    .expect("input items should normalize");

    assert_eq!(
        normalized,
        vec![
            TurnInputItem::Text {
                text: "Use $lint".to_string(),
            },
            TurnInputItem::Skill {
                name: "lint".to_string(),
                path: "/tmp/skills/lint".to_string(),
            },
            TurnInputItem::Mention {
                name: "Docs".to_string(),
                path: "app://docs".to_string(),
            },
            TurnInputItem::Text {
                text: " now".to_string(),
            },
        ]
    );
}

#[test]
fn chat_notification_preview_ignores_steer_blocks_and_uses_first_text_block() {
    let preview = chat_notification_preview(&[
        ContentBlock::Text {
            content: "hidden steer".to_string(),
            plan_mode: None,
            is_steer: Some(true),
        },
        ContentBlock::Text {
            content: "  First line\n\nSecond line  ".to_string(),
            plan_mode: None,
            is_steer: None,
        },
    ]);

    assert_eq!(preview.as_deref(), Some("First line Second line"));
}

#[test]
fn chat_notification_preview_falls_back_to_error_blocks() {
    let preview = chat_notification_preview(&[ContentBlock::Error {
        message: "Command failed".to_string(),
    }]);

    assert_eq!(preview.as_deref(), Some("Command failed"));
}

#[test]
fn normalize_input_items_rejects_blank_typed_entries() {
    let error = normalize_input_items(
        "fallback",
        Some(vec![ChatInputItemPayload::Skill {
            name: " ".to_string(),
            path: "/tmp/skills/lint".to_string(),
        }]),
    )
    .expect_err("blank skill names should be rejected");

    assert!(error.contains("skill input items require non-empty name and path"));
}

#[test]
fn normalize_codex_review_target_builds_commit_payload() {
    let (target, message, title) =
        normalize_codex_review_target(&CodexReviewTargetPayload::Commit {
            sha: "abcdef1234567890".to_string(),
            title: Some("Refactor auth flow".to_string()),
        })
        .expect("commit target should normalize");

    assert_eq!(
        target,
        serde_json::json!({
            "type": "commit",
            "sha": "abcdef1234567890",
            "title": "Refactor auth flow",
        })
    );
    assert_eq!(
        message,
        "Review commit `abcdef1234567890`: Refactor auth flow"
    );
    assert_eq!(title, "Review: abcdef123456");
}

#[test]
fn clone_codex_review_metadata_clears_runtime_only_fields() {
    let metadata = clone_codex_review_metadata(
        Some(&serde_json::json!({
            "manualTitle": true,
            "manualTitleUpdatedAt": "2026-03-12T00:00:00Z",
            "codexPreview": "old preview",
            "codexThreadStatus": "active",
            "codexThreadActiveFlags": ["waitingOnApproval"],
            "codexSyncRequired": true,
            "codexSyncReason": "stale",
            "serviceTier": "fast",
        })),
        "gpt-5.4",
    )
    .expect("metadata should clone");

    assert_eq!(metadata.get("manualTitle"), None);
    assert_eq!(metadata.get("codexPreview"), None);
    assert_eq!(metadata.get("codexThreadStatus"), None);
    assert_eq!(metadata.get("codexThreadActiveFlags"), None);
    assert_eq!(metadata.get("codexSyncRequired"), None);
    assert_eq!(metadata.get("codexSyncReason"), None);
    assert_eq!(
        metadata.get("serviceTier"),
        Some(&serde_json::json!("fast"))
    );
    assert_eq!(
        metadata.get("lastModelId"),
        Some(&serde_json::json!("gpt-5.4"))
    );
}

#[test]
fn normalize_input_items_rejects_non_empty_message_without_text_segments() {
    let error = normalize_input_items(
        "Use lint",
        Some(vec![ChatInputItemPayload::Skill {
            name: "lint".to_string(),
            path: "/tmp/skills/lint".to_string(),
        }]),
    )
    .expect_err("non-empty message text requires a text segment");

    assert!(error.contains("input items must include at least one text segment"));
}

#[test]
fn external_sandbox_blocks_explicit_workspace_write_override() {
    assert!(unsupported_thread_sandbox_override_for_external_sandbox(
        Some("workspace-write"),
        true,
    ));
    assert!(unsupported_thread_sandbox_override_for_external_sandbox(
        Some("read-only"),
        true,
    ));
    assert!(!unsupported_thread_sandbox_override_for_external_sandbox(
        Some("danger-full-access"),
        true,
    ));
}

#[test]
fn resolve_workspace_writable_roots_prefers_confirmed_subset() {
    let roots = resolve_workspace_writable_roots(
        ["/workspace/repo-a", "/workspace/repo-b"],
        "/workspace",
        Some(&serde_json::json!({
            "workspaceWritableRoots": ["/workspace/repo-b"]
        })),
    )
    .expect("expected confirmed roots to resolve");

    assert_eq!(roots.roots, vec![String::from("/workspace/repo-b")]);
    assert!(!roots.requires_confirmation);
}

#[test]
fn resolve_workspace_writable_roots_drops_stale_confirmed_paths() {
    let roots = resolve_workspace_writable_roots(
        ["/workspace/repo-a", "/workspace/repo-b"],
        "/workspace",
        Some(&serde_json::json!({
            "workspaceWritableRoots": ["/workspace/repo-b", "/workspace/repo-c"]
        })),
    )
    .expect("expected stale confirmed roots to be ignored");

    assert_eq!(roots.roots, vec![String::from("/workspace/repo-b")]);
    assert!(!roots.requires_confirmation);
}

#[test]
fn resolve_workspace_writable_roots_requires_reconfirmation_when_all_confirmed_roots_are_stale() {
    let roots = resolve_workspace_writable_roots(
        ["/workspace/repo-a", "/workspace/repo-b"],
        "/workspace",
        Some(&serde_json::json!({
            "workspaceWritableRoots": ["/workspace/repo-c"]
        })),
    )
    .expect("expected stale confirmed roots to resolve to current repos");

    assert_eq!(
        roots.roots,
        vec![
            String::from("/workspace/repo-a"),
            String::from("/workspace/repo-b")
        ]
    );
    assert!(roots.requires_confirmation);
}

#[test]
fn read_only_workspace_threads_ignore_stale_confirmation_requirements() {
    let resolution = WorkspaceWritableRootsResolution {
        roots: vec![
            String::from("/workspace/repo-a"),
            String::from("/workspace/repo-b"),
        ],
        requires_confirmation: true,
    };

    assert!(!workspace_write_confirmation_required(
        Some(&resolution),
        "read-only",
        true,
    ));
    assert!(workspace_write_confirmation_required(
        Some(&resolution),
        "workspace-write",
        true,
    ));
}

#[test]
fn claude_defaults_follow_trust_level_directly() {
    assert_eq!(
        approval_policy_for_engine_and_trust_level("claude", &TrustLevelDto::Trusted),
        "trusted"
    );
    assert_eq!(
        approval_policy_for_engine_and_trust_level("claude", &TrustLevelDto::Standard),
        "standard"
    );
    assert_eq!(
        approval_policy_for_engine_and_trust_level("claude", &TrustLevelDto::Restricted),
        "restricted"
    );
}

#[test]
fn opencode_defaults_use_permission_modes_not_codex_sandbox_policies() {
    assert_eq!(
        approval_policy_for_engine_and_trust_level("opencode", &TrustLevelDto::Trusted),
        "ask"
    );
    assert_eq!(
        approval_policy_for_engine_and_trust_level("opencode", &TrustLevelDto::Standard),
        "ask"
    );
    assert_eq!(
        approval_policy_for_engine_and_trust_level("opencode", &TrustLevelDto::Restricted),
        "deny"
    );
}

#[test]
fn claude_permission_mode_override_uses_claude_key() {
    let metadata = serde_json::json!({
        "claudePermissionMode": "restricted",
        "sandboxApprovalPolicy": "never",
    });

    assert_eq!(
        thread_approval_policy_override_value("claude", Some(&metadata)).unwrap(),
        Some(Value::String("restricted".to_string()))
    );
    assert_eq!(
        thread_approval_policy_override_value("codex", Some(&metadata)).unwrap(),
        Some(Value::String("never".to_string()))
    );
}

#[test]
fn opencode_permission_mode_override_uses_opencode_key() {
    let metadata = serde_json::json!({
        "opencodePermissionMode": "allow",
        "sandboxApprovalPolicy": "never",
    });

    assert_eq!(
        thread_approval_policy_override_value("opencode", Some(&metadata)).unwrap(),
        Some(Value::String("allow".to_string()))
    );
    assert_eq!(
        thread_approval_policy_override_value("codex", Some(&metadata)).unwrap(),
        Some(Value::String("never".to_string()))
    );
}

#[test]
fn invalid_structured_codex_approval_policy_is_rejected() {
    let metadata = serde_json::json!({
        "sandboxApprovalPolicy": {
            "reject": {
                "rules": true,
                "sandbox_approval": false
            }
        }
    });

    let error = thread_approval_policy_override_value("codex", Some(&metadata))
        .expect_err("expected malformed structured approval policy to fail");

    assert!(error.contains("reject.mcp_elicitations"));
}

#[test]
fn action_progress_coalescing_keeps_latest_message() {
    let merged = try_coalesce_stream_events(
        EngineEvent::ActionProgressUpdated {
            action_id: "action-1".to_string(),
            message: "Connecting".to_string(),
        },
        EngineEvent::ActionProgressUpdated {
            action_id: "action-1".to_string(),
            message: "Fetching results".to_string(),
        },
    )
    .expect("expected coalesced action progress");

    match merged {
        EngineEvent::ActionProgressUpdated { action_id, message } => {
            assert_eq!(action_id, "action-1");
            assert_eq!(message, "Fetching results");
        }
        other => panic!("expected action progress event, got {other:?}"),
    }
}

#[test]
fn debug_event_log_trims_action_output_payload() {
    let content = "x".repeat(ENGINE_EVENT_LOG_ACTION_OUTPUT_MAX_CHARS + 128);
    let event = EngineEvent::ActionOutputDelta {
        action_id: "action-1".to_string(),
        stream: OutputStream::Stdout,
        content,
    };

    let log_event = engine_event_for_debug_log(&event);

    match log_event {
        EngineEvent::ActionOutputDelta { content, .. } => {
            assert_eq!(content.len(), ENGINE_EVENT_LOG_ACTION_OUTPUT_MAX_CHARS);
        }
        other => panic!("expected action output event, got {other:?}"),
    }
}

#[test]
fn trim_action_output_chunks_keeps_tail_of_oversized_chunk() {
    let mut chunks = vec![ActionOutputChunk {
        stream: "stdout".to_string(),
        content: "0123456789".to_string(),
    }];

    assert!(trim_action_output_chunks(&mut chunks, 6));
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].content, "6789");
}

#[test]
fn model_reroute_notice_reindexes_action_blocks() {
    let mut blocks = Vec::new();
    let mut action_index = HashMap::new();
    let mut approval_index = HashMap::new();

    let started = apply_event_to_blocks(
        &mut blocks,
        &mut action_index,
        &mut approval_index,
        &EngineEvent::ActionStarted {
            action_id: "action-1".to_string(),
            engine_action_id: Some("item-1".to_string()),
            action_type: crate::engines::events::ActionType::Other,
            summary: "search_docs".to_string(),
            display_label: None,
            display_subtitle: None,
            details: serde_json::json!({}),
        },
        1000,
    );
    assert!(started.blocks_changed);

    let rerouted = apply_event_to_blocks(
        &mut blocks,
        &mut action_index,
        &mut approval_index,
        &EngineEvent::ModelRerouted {
            from_model: "gpt-5.1-codex-mini".to_string(),
            to_model: "gpt-5.3-codex".to_string(),
            reason: "highRiskCyberActivity".to_string(),
        },
        1000,
    );
    assert!(rerouted.blocks_changed);
    assert_eq!(rerouted.turn_model_id.as_deref(), Some("gpt-5.3-codex"));

    let progress = apply_event_to_blocks(
        &mut blocks,
        &mut action_index,
        &mut approval_index,
        &EngineEvent::ActionProgressUpdated {
            action_id: "action-1".to_string(),
            message: "Fetching results".to_string(),
        },
        1000,
    );
    assert!(progress.blocks_changed);

    assert!(matches!(
        &blocks[0],
        ContentBlock::Notice {
            kind,
            level,
            title,
            ..
        } if kind == "model_rerouted" && level == "info" && title == "Model rerouted"
    ));
    match &blocks[1] {
        ContentBlock::Action { details, .. } => {
            let details_value: Value =
                serde_json::from_str(details.get()).expect("action details should parse as JSON");
            assert_eq!(
                details_value
                    .get("progressKind")
                    .and_then(serde_json::Value::as_str),
                Some("mcp")
            );
            assert_eq!(
                details_value
                    .get("progressMessage")
                    .and_then(serde_json::Value::as_str),
                Some("Fetching results")
            );
        }
        other => panic!("expected action block, got {other:?}"),
    }
}

#[test]
fn diff_update_collapses_existing_same_scope_diff_blocks() {
    let mut blocks = vec![
        ContentBlock::Diff {
            diff: "old diff 1".to_string(),
            scope: "turn".to_string(),
        },
        ContentBlock::Text {
            content: "kept".to_string(),
            plan_mode: None,
            is_steer: None,
        },
        ContentBlock::Diff {
            diff: "old diff 2".to_string(),
            scope: "turn".to_string(),
        },
    ];
    let mut action_index = HashMap::new();
    let mut approval_index = HashMap::new();

    let progress = apply_event_to_blocks(
        &mut blocks,
        &mut action_index,
        &mut approval_index,
        &EngineEvent::DiffUpdated {
            diff: "new diff".to_string(),
            scope: crate::engines::DiffScope::Turn,
        },
        1000,
    );

    assert!(progress.blocks_changed);
    assert_eq!(blocks.len(), 2);
    assert!(matches!(
        &blocks[0],
        ContentBlock::Text { content, .. } if content == "kept"
    ));
    assert!(matches!(
        &blocks[1],
        ContentBlock::Diff { diff, scope } if diff == "new diff" && scope == "turn"
    ));
}

#[test]
fn pasted_image_extension_rejects_unknown_image_mime() {
    assert_eq!(
        pasted_image_extension("pasted-image-1.png", "image/heic"),
        None
    );
}

#[test]
fn generic_notice_blocks_are_upserted_by_kind() {
    let mut blocks = Vec::new();
    let mut action_index = HashMap::new();
    let mut approval_index = HashMap::new();

    let first = apply_event_to_blocks(
        &mut blocks,
        &mut action_index,
        &mut approval_index,
        &EngineEvent::Notice {
            kind: "deprecation_notice".to_string(),
            level: "warning".to_string(),
            title: "Deprecation notice".to_string(),
            message: "Use the newer API.".to_string(),
        },
        1000,
    );
    assert!(first.blocks_changed);

    let second = apply_event_to_blocks(
        &mut blocks,
        &mut action_index,
        &mut approval_index,
        &EngineEvent::Notice {
            kind: "deprecation_notice".to_string(),
            level: "warning".to_string(),
            title: "Deprecation notice".to_string(),
            message: "Use the newer permissions API.".to_string(),
        },
        1000,
    );
    assert!(second.blocks_changed);
    assert_eq!(blocks.len(), 1);
    assert!(matches!(
        &blocks[0],
        ContentBlock::Notice { message, .. } if message == "Use the newer permissions API."
    ));
}

#[test]
fn approval_response_persistence_tracks_permissions_session_scope() {
    let response = serde_json::json!({
        "permissions": {
            "network": {
                "enabled": true
            }
        },
        "scope": "session"
    });

    assert_eq!(
        approval_response_decision_for_persistence(&response),
        "accept_for_session"
    );
}

#[test]
fn approval_response_persistence_tracks_mcp_elicitation_actions() {
    let response = serde_json::json!({
        "action": "decline"
    });

    assert_eq!(
        approval_response_decision_for_persistence(&response),
        "decline"
    );
}

#[test]
fn approval_response_persistence_treats_empty_permissions_as_decline() {
    let response = serde_json::json!({
        "permissions": {},
        "scope": "turn"
    });

    assert_eq!(
        approval_response_decision_for_persistence(&response),
        "decline"
    );
}

#[tokio::test]
async fn invalid_claude_approval_response_keeps_approval_pending() {
    let state = test_app_state();
    let thread = test_thread(&state, "claude", "claude-sonnet-4-6");
    let approval_id = "approval-invalid";
    let message_id = insert_pending_approval(&state, &thread, approval_id);

    let error = respond_to_approval_inner(
        &state,
        thread.id.clone(),
        approval_id.to_string(),
        serde_json::json!({}),
    )
    .await
    .expect_err("expected invalid approval payload to fail");

    assert!(error.contains("explicit `decision`"));

    let conn = state.db.connect().expect("failed to open db connection");
    let approval_row = conn
        .query_row(
            "SELECT status, decision FROM approvals WHERE id = ?1",
            params![approval_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .expect("failed to load approval row");
    assert_eq!(approval_row.0, "pending");
    assert_eq!(approval_row.1, None);

    let thread_status = conn
        .query_row(
            "SELECT status FROM threads WHERE id = ?1",
            params![thread.id],
            |row| row.get::<_, String>(0),
        )
        .expect("failed to load thread status");
    assert_eq!(thread_status, "awaiting_approval");

    let raw_blocks = conn
        .query_row(
            "SELECT blocks_json FROM messages WHERE id = ?1",
            params![message_id],
            |row| row.get::<_, String>(0),
        )
        .expect("failed to load message blocks");
    let blocks: Value =
        serde_json::from_str(&raw_blocks).expect("message blocks should deserialize");
    assert_eq!(
        blocks
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item.get("status"))
            .and_then(Value::as_str),
        Some("pending")
    );
    assert!(blocks
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("decision"))
        .is_none());
}

#[tokio::test]
async fn missing_live_codex_approval_request_keeps_approval_pending() {
    let state = test_app_state();
    let thread = test_thread(&state, "codex", "gpt-5.5-codex");
    let approval_id = "approval-reset";
    let message_id = insert_pending_approval_with_details(
        &state,
        &thread,
        approval_id,
        serde_json::json!({
            "command": "touch file.txt",
            "_serverMethod": "item/fileChange/requestApproval",
            "_rawRequestId": 42
        }),
    );

    let error = respond_to_approval_inner(
        &state,
        thread.id.clone(),
        approval_id.to_string(),
        serde_json::json!({ "decision": "accept" }),
    )
    .await
    .expect_err("expected codex approval without live request to fail");

    assert!(error.contains("runtime connection was reset"));

    let conn = state.db.connect().expect("failed to open db connection");
    let approval_row = conn
        .query_row(
            "SELECT status, decision FROM approvals WHERE id = ?1",
            params![approval_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .expect("failed to load approval row");
    assert_eq!(approval_row.0, "pending");
    assert_eq!(approval_row.1, None);

    let thread_status = conn
        .query_row(
            "SELECT status FROM threads WHERE id = ?1",
            params![thread.id],
            |row| row.get::<_, String>(0),
        )
        .expect("failed to load thread status");
    assert_eq!(thread_status, "awaiting_approval");

    let raw_blocks = conn
        .query_row(
            "SELECT blocks_json FROM messages WHERE id = ?1",
            params![message_id],
            |row| row.get::<_, String>(0),
        )
        .expect("failed to load message blocks");
    let blocks: Value =
        serde_json::from_str(&raw_blocks).expect("message blocks should deserialize");
    assert_eq!(
        blocks
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item.get("status"))
            .and_then(Value::as_str),
        Some("pending")
    );
    assert!(blocks
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("decision"))
        .is_none());
}

#[tokio::test]
async fn load_codex_approval_response_route_reads_persisted_transport_metadata() {
    let state = test_app_state();
    let thread = test_thread(&state, "codex", "gpt-5.5-codex");
    insert_pending_approval_with_details(
        &state,
        &thread,
        "approval-route",
        serde_json::json!({
            "command": "touch file.txt",
            "_serverMethod": "item/fileChange/requestApproval",
            "_rawRequestId": 42
        }),
    );

    let route = load_approval_response_route(state.db.clone(), "codex", "approval-route")
        .await
        .unwrap();

    assert_eq!(
        route,
        Some(ApprovalRequestRoute {
            server_method: "item/fileChange/requestApproval".to_string(),
            raw_request_id: serde_json::json!(42),
        })
    );
}

#[test]
fn resolve_reasoning_effort_from_catalog_falls_back_to_model_default() {
    let engines = vec![EngineInfoDto {
        id: "codex".to_string(),
        name: "Codex".to_string(),
        models: vec![EngineModelDto {
            id: "gpt-5.1-codex-mini".to_string(),
            display_name: "GPT-5.1 Codex Mini".to_string(),
            description: String::new(),
            hidden: false,
            is_default: false,
            upgrade: None,
            availability_nux: None,
            upgrade_info: None,
            input_modalities: vec!["text".to_string()],
            attachment_modalities: vec!["text".to_string()],
            limits: None,
            supports_personality: false,
            default_reasoning_effort: "medium".to_string(),
            supported_reasoning_efforts: vec![
                ReasoningEffortOptionDto {
                    reasoning_effort: "medium".to_string(),
                    description: String::new(),
                },
                ReasoningEffortOptionDto {
                    reasoning_effort: "high".to_string(),
                    description: String::new(),
                },
            ],
        }],
        capabilities: EngineCapabilitiesDto {
            permission_modes: Vec::new(),
            sandbox_modes: Vec::new(),
            approval_decisions: Vec::new(),
        },
    }];

    assert_eq!(
        resolve_reasoning_effort_from_catalog(
            &engines,
            "codex",
            "gpt-5.1-codex-mini",
            Some("xhigh"),
        ),
        Some("medium".to_string())
    );
}

#[test]
fn resolve_reasoning_effort_from_catalog_keeps_supported_effort() {
    let engines = vec![EngineInfoDto {
        id: "codex".to_string(),
        name: "Codex".to_string(),
        models: vec![EngineModelDto {
            id: "gpt-5.1-codex-mini".to_string(),
            display_name: "GPT-5.1 Codex Mini".to_string(),
            description: String::new(),
            hidden: false,
            is_default: false,
            upgrade: None,
            availability_nux: None,
            upgrade_info: None,
            input_modalities: vec!["text".to_string()],
            attachment_modalities: vec!["text".to_string()],
            limits: None,
            supports_personality: false,
            default_reasoning_effort: "medium".to_string(),
            supported_reasoning_efforts: vec![
                ReasoningEffortOptionDto {
                    reasoning_effort: "medium".to_string(),
                    description: String::new(),
                },
                ReasoningEffortOptionDto {
                    reasoning_effort: "high".to_string(),
                    description: String::new(),
                },
            ],
        }],
        capabilities: EngineCapabilitiesDto {
            permission_modes: Vec::new(),
            sandbox_modes: Vec::new(),
            approval_decisions: Vec::new(),
        },
    }];

    assert_eq!(
        resolve_reasoning_effort_from_catalog(
            &engines,
            "codex",
            "gpt-5.1-codex-mini",
            Some("high"),
        ),
        Some("high".to_string())
    );
}

#[test]
fn resolve_turn_model_id_accepts_thread_last_model_without_catalog() {
    let state = test_app_state();
    let mut thread = test_thread(&state, "codex", "gpt-5.5-codex");
    thread.engine_metadata = Some(serde_json::json!({
        "lastModelId": "gpt-5.1-codex-mini"
    }));

    assert_eq!(
        resolve_turn_model_id(&thread, Some("gpt-5.1-codex-mini"), None)
            .expect("last model should resolve without a catalog"),
        "gpt-5.1-codex-mini"
    );
}
