use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{value::RawValue, Value};
use tauri::{Emitter, State};
use tokio::fs as tokio_fs;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    db,
    engines::{
        approval_response_route_for_engine, normalize_approval_response_for_engine,
        trim_action_output_delta_content, validate_engine_sandbox_mode, ApprovalRequestRoute,
        EngineEvent, OutputStream, SandboxPolicy, ThreadScope, TurnAttachment,
        TurnCompletionStatus, TurnInput, TurnInputItem, STREAMED_DIFF_MAX_CHARS,
    },
    models::{
        ActionOutputDto, EngineInfoDto, EngineModelDto, MessageDto, MessageStatusDto,
        MessageWindowCursorDto, MessageWindowDto, RepoDto, SearchResultDto, ThreadDto,
        ThreadStatusDto, TrustLevelDto,
    },
    runtime_env,
    state::AppState,
};

const MAX_THREAD_TITLE_CHARS: usize = 72;
const STREAM_EVENT_COALESCE_MAX_CHARS: usize = 8_192;
const STREAM_EVENT_COALESCE_IDLE_FLUSH_INTERVAL: Duration = Duration::from_millis(24);
const STREAM_DB_FLUSH_INTERVAL: Duration = Duration::from_millis(250);
const STREAM_DB_BLOCKS_FLUSH_INTERVAL: Duration = Duration::from_millis(900);
const ENGINE_EVENT_QUEUE_CAPACITY: usize = 128;
const ACTION_OUTPUT_MAX_CHUNKS: usize = 240;
const ENGINE_EVENT_LOG_ACTION_OUTPUT_MAX_CHARS: usize = 4_096;
const TRUNCATED_SUFFIX: &str = "\n... [truncated]";
const MAX_ATTACHMENTS_PER_TURN: usize = 10;
const MAX_PASTED_IMAGE_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;
const TEXT_ATTACHMENT_EXTENSIONS: &[&str] = &[
    "txt", "md", "json", "js", "ts", "tsx", "jsx", "py", "rs", "go", "css", "html", "yaml", "yml",
    "toml", "xml", "sql", "sh", "csv",
];
const IMAGE_ATTACHMENT_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "svg",
];
const MESSAGE_WINDOW_DEFAULT_LIMIT: usize = 120;
const MESSAGE_WINDOW_MAX_LIMIT: usize = 400;
const MAX_CHAT_NOTIFICATION_PREVIEW_CHARS: usize = 240;

#[path = "chat/approvals.rs"]
mod approvals;
#[path = "chat/attachments.rs"]
mod attachments;
#[path = "chat/input.rs"]
mod input;
#[path = "chat/legacy_native.rs"]
mod legacy_native;
#[path = "chat/message_query.rs"]
mod message_query;
#[path = "chat/policy.rs"]
mod policy;
#[path = "chat/review.rs"]
mod review;
#[path = "chat/turn_blocks.rs"]
mod turn_blocks;
#[path = "chat/turn_notifications.rs"]
mod turn_notifications;
#[path = "chat/turn_runner.rs"]
mod turn_runner;
#[path = "chat/turn_stream.rs"]
mod turn_stream;
#[path = "chat/types.rs"]
mod types;

#[cfg(test)]
use approvals::{
    approval_response_decision_for_persistence, load_approval_response_route,
    respond_to_approval_inner,
};
#[cfg(test)]
use attachments::pasted_image_extension;
use input::*;
use legacy_native::*;
use policy::*;
use review::*;
use turn_blocks::*;
use turn_notifications::*;
use turn_runner::*;
use turn_stream::*;
use types::{
    ActionBlockResult, ActionOutputChunk, ChatTurnFinishedEvent, ContentBlock, EventProgress,
    ThreadUpdatedEvent,
};
pub use types::{
    AttachmentPreviewPayload, ChatAttachmentPayload, ChatInputItemPayload,
    CodexReviewDeliveryPayload, CodexReviewTargetPayload,
};

fn value_to_raw(value: &Value) -> Box<RawValue> {
    serde_json::value::to_raw_value(value).unwrap_or_else(|_| empty_raw_value())
}

fn empty_raw_value() -> Box<RawValue> {
    RawValue::from_string("null".to_string()).expect("\"null\" is a valid JSON literal")
}

#[tauri::command]
pub async fn save_pasted_image_attachment(
    file_name: String,
    mime_type: String,
    data_base64: String,
) -> Result<ChatAttachmentPayload, String> {
    attachments::save_pasted_image_attachment(file_name, mime_type, data_base64).await
}

#[tauri::command]
pub async fn read_attachment_preview(
    file_path: String,
    mime_type: Option<String>,
) -> Result<Option<AttachmentPreviewPayload>, String> {
    attachments::read_attachment_preview(file_path, mime_type).await
}

async fn run_db<T, F>(db: crate::db::Database, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&crate::db::Database) -> anyhow::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || operation(&db))
        .await
        .map_err(|error| error.to_string())?
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    thread_id: String,
    message: String,
    display_message: Option<String>,
    model_id: Option<String>,
    reasoning_effort: Option<String>,
    attachments: Option<Vec<ChatAttachmentPayload>>,
    input_items: Option<Vec<ChatInputItemPayload>>,
    plan_mode: Option<bool>,
    client_turn_id: Option<String>,
) -> Result<String, String> {
    let already_running = state.turns.get(&thread_id).await.is_some();
    if already_running {
        return Err(
            "A turn is already running for this thread. Cancel it before sending another message."
                .to_string(),
        );
    }

    let db = state.db.clone();
    let mut thread = run_db(db.clone(), {
        let thread_id = thread_id.clone();
        move |db| db::threads::get_thread(db, &thread_id)
    })
    .await?
    .ok_or_else(|| format!("thread not found: {thread_id}"))?;
    let requested_model_id = model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let attachments = normalize_attachments(attachments)?;
    let input_items = normalize_input_items(message.as_str(), input_items)?;
    let display_message = display_message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let plan_mode = plan_mode.unwrap_or(false);
    let turn_input = TurnInput {
        message: message.clone(),
        attachments: attachments.clone(),
        plan_mode,
        input_items: input_items.clone(),
    };
    let current_turn_model_id = thread_last_model_id(thread.engine_metadata.as_ref())
        .unwrap_or_else(|| thread.model_id.clone());
    let model_switch_requested = requested_model_id
        .map(|value| value != current_turn_model_id.as_str())
        .unwrap_or(false);
    let validation_catalog = if model_switch_requested {
        state.engines.list_engines().await.ok()
    } else {
        None
    };
    let effective_model_id =
        resolve_turn_model_id(&thread, requested_model_id, validation_catalog.as_deref())?;
    let attachment_catalog = if attachments.is_empty() {
        None
    } else if let Some(catalog) = validation_catalog.as_ref() {
        Some(catalog.clone())
    } else {
        Some(state.engines.list_engines().await.map_err(err_to_string)?)
    };
    validate_attachments_for_engine_model(
        &attachments,
        &thread.engine_id,
        &effective_model_id,
        attachment_catalog.as_deref(),
    )?;

    let legacy_native_migration = migrate_legacy_native_thread_metadata(&mut thread);
    if legacy_native_migration
        .as_ref()
        .map(|migration| migration.metadata_changed)
        .unwrap_or(false)
    {
        let metadata = thread
            .engine_metadata
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        run_db(db.clone(), {
            let thread_id = thread.id.clone();
            move |db| db::threads::update_engine_metadata(db, &thread_id, &metadata)
        })
        .await?;
    }

    let (workspace, repos, selected_repo) = run_db(db.clone(), {
        let workspace_id = thread.workspace_id.clone();
        let thread_id = thread.id.clone();
        let repo_id = thread.repo_id.clone();
        move |db| {
            let workspace = db::workspaces::list_workspaces(db)?
                .into_iter()
                .find(|item| item.id == workspace_id)
                .ok_or_else(|| anyhow::anyhow!("workspace not found for thread {thread_id}"))?;
            let repos = db::repos::get_repos(db, &workspace_id)?;
            let selected_repo = if let Some(repo_id) = repo_id.as_deref() {
                db::repos::find_repo_by_id(db, repo_id)?
            } else {
                None
            };
            Ok((workspace, repos, selected_repo))
        }
    })
    .await?;

    let workspace_root = workspace.root_path.clone();
    let requested_reasoning_effort = normalize_reasoning_effort_value(reasoning_effort.as_deref());
    let stored_reasoning_effort = thread_reasoning_effort(thread.engine_metadata.as_ref());
    let configured_reasoning_effort = requested_reasoning_effort
        .clone()
        .or_else(|| stored_reasoning_effort.clone());
    let reasoning_effort = if requested_reasoning_effort.is_some() {
        requested_reasoning_effort
    } else if model_switch_requested {
        validation_catalog
            .as_deref()
            .map(|engines| {
                resolve_reasoning_effort_from_catalog(
                    engines,
                    thread.engine_id.as_str(),
                    effective_model_id.as_str(),
                    configured_reasoning_effort.as_deref(),
                )
            })
            .unwrap_or_else(|| {
                normalize_reasoning_effort_value(configured_reasoning_effort.as_deref())
            })
    } else {
        configured_reasoning_effort.clone()
    };
    let sandbox_mode_override = thread_sandbox_mode(thread.engine_metadata.as_ref())?;
    let supports_panes_sandbox = thread.engine_id != "opencode";
    let sandbox_mode = if supports_panes_sandbox {
        Some(
            sandbox_mode_override
                .clone()
                .unwrap_or_else(|| "workspace-write".to_string()),
        )
    } else {
        if sandbox_mode_override.is_some() {
            log::warn!(
                "ignoring sandbox mode override on OpenCode thread {}",
                thread.id
            );
        }
        None
    };
    let workspace_writable_roots = if selected_repo.is_some() {
        None
    } else {
        Some(resolve_workspace_writable_roots(
            repos.iter().map(|repo| repo.path.as_str()),
            workspace_root.as_str(),
            thread.engine_metadata.as_ref(),
        )?)
    };
    let scope = if let Some(repo) = selected_repo.as_ref() {
        ThreadScope::Repo {
            repo_path: repo.path.clone(),
        }
    } else {
        ThreadScope::Workspace {
            root_path: workspace_root,
            writable_roots: workspace_writable_roots
                .as_ref()
                .map(|resolution| resolution.roots.clone())
                .unwrap_or_default(),
        }
    };

    let trust_level = selected_repo
        .as_ref()
        .map(|repo| repo.trust_level.clone())
        .unwrap_or_else(|| aggregate_workspace_trust_level(&repos));
    let codex_external_sandbox_active = if thread.engine_id == "codex" {
        state.engines.codex_uses_external_sandbox().await
    } else {
        false
    };
    let permission_profile = if thread.engine_id == "codex" {
        thread_permission_profile(thread.engine_metadata.as_ref())
    } else {
        None
    };

    if permission_profile.is_none() {
        if let Some(sandbox_mode) = sandbox_mode.as_deref() {
            if unsupported_thread_sandbox_override_for_external_sandbox(
                sandbox_mode_override.as_deref(),
                codex_external_sandbox_active,
            ) {
                return Err(
                "Codex read-only and workspace-write sandbox overrides are unavailable while Panes is using external sandbox mode. Clear the override or restore local Codex sandboxing first.".to_string(),
            );
            }

            validate_engine_sandbox_mode(thread.engine_id.as_str(), Some(sandbox_mode))?;

            if workspace_write_confirmation_required(
                workspace_writable_roots.as_ref(),
                sandbox_mode,
                workspace_write_opt_in_enabled(thread.engine_metadata.as_ref()),
            ) {
                return Err(
                "Workspace thread with multiple writable repositories requires explicit confirmation before execution.".to_string(),
            );
            }
        }
    }

    if requested_model_id.is_some() || reasoning_effort != stored_reasoning_effort {
        let mut metadata = thread
            .engine_metadata
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        if !metadata.is_object() {
            metadata = serde_json::json!({});
        }
        if let Some(object) = metadata.as_object_mut() {
            if requested_model_id.is_some() {
                object.insert(
                    "lastModelId".to_string(),
                    Value::String(effective_model_id.clone()),
                );
            }
            match reasoning_effort.as_ref() {
                Some(value) => {
                    object.insert("reasoningEffort".to_string(), Value::String(value.clone()));
                }
                None => {
                    object.remove("reasoningEffort");
                }
            }
        }
        run_db(db.clone(), {
            let thread_id = thread.id.clone();
            let metadata = metadata.clone();
            move |db| db::threads::update_engine_metadata(db, &thread_id, &metadata)
        })
        .await?;
        thread.engine_metadata = Some(metadata);
    }

    let writable_roots = match &scope {
        ThreadScope::Repo { repo_path } => vec![repo_path.clone()],
        ThreadScope::Workspace {
            writable_roots,
            root_path,
        } => {
            if writable_roots.is_empty() {
                vec![root_path.clone()]
            } else {
                writable_roots.clone()
            }
        }
    };

    let allow_network =
        if thread.engine_id == "codex" && sandbox_mode.as_deref() == Some("danger-full-access") {
            true
        } else {
            thread_allow_network_override(thread.engine_metadata.as_ref())
                .unwrap_or_else(|| allow_network_for_trust_level(&trust_level))
        };
    let personality = if thread.engine_id == "codex"
        && model_supports_personality(state.inner(), &thread.engine_id, &effective_model_id).await
    {
        thread_personality(thread.engine_metadata.as_ref())
    } else {
        None
    };

    let approval_policy_override = thread_approval_policy_override_value(
        thread.engine_id.as_str(),
        thread.engine_metadata.as_ref(),
    )?;

    let sandbox = SandboxPolicy {
        writable_roots,
        allow_network,
        approval_policy: Some(approval_policy_override.unwrap_or_else(|| {
            Value::String(
                approval_policy_for_engine_and_trust_level(thread.engine_id.as_str(), &trust_level)
                    .to_string(),
            )
        })),
        permission_profile,
        approvals_reviewer: if thread.engine_id == "codex" {
            thread_approvals_reviewer(thread.engine_metadata.as_ref())
        } else {
            None
        },
        reasoning_effort: reasoning_effort.clone(),
        sandbox_mode,
        service_tier: thread_service_tier(thread.engine_metadata.as_ref()),
        personality,
        output_schema: thread_output_schema(thread.engine_metadata.as_ref()),
        opencode_agent: thread_opencode_agent(thread.engine_metadata.as_ref()),
    };

    let engine_thread_id = state
        .engines
        .ensure_engine_thread(&thread, Some(effective_model_id.as_str()), scope, sandbox)
        .await
        .map_err(err_to_string)?;

    if thread.engine_thread_id.as_deref() != Some(&engine_thread_id) {
        run_db(db.clone(), {
            let thread_id = thread.id.clone();
            let engine_thread_id = engine_thread_id.clone();
            move |db| db::threads::set_engine_thread_id(db, &thread_id, &engine_thread_id)
        })
        .await?;
        thread.engine_thread_id = Some(engine_thread_id.clone());
    }

    let cancellation = CancellationToken::new();
    if !state
        .turns
        .try_register(&thread.id, cancellation.clone())
        .await
    {
        return Err(
            "A turn is already running for this thread. Cancel it before sending another message."
                .to_string(),
        );
    }

    let assistant_message = match run_db(db.clone(), {
        let thread_id = thread.id.clone();
        let message = message.clone();
        let persisted_message = display_message.clone().unwrap_or_else(|| message.clone());
        let persisted_input_items = if display_message.is_some() {
            Vec::new()
        } else {
            input_items.clone()
        };
        let attachments = attachments.clone();
        let plan_mode_enabled = plan_mode;
        let engine_id = thread.engine_id.clone();
        let model_id = effective_model_id.clone();
        let reasoning_effort = reasoning_effort.clone();
        move |db| {
            let user_blocks = build_user_blocks(
                &persisted_message,
                &persisted_input_items,
                &attachments,
                plan_mode_enabled,
                false,
            );
            db::messages::insert_user_message(
                db,
                &thread_id,
                &persisted_message,
                Some(serde_json::to_value(&user_blocks)?),
                Some(engine_id.as_str()),
                Some(model_id.as_str()),
                reasoning_effort.as_deref(),
            )?;
            let assistant_message = db::messages::insert_assistant_placeholder(
                db,
                &thread_id,
                Some(engine_id.as_str()),
                Some(model_id.as_str()),
                reasoning_effort.as_deref(),
            )?;
            db::threads::update_thread_status(db, &thread_id, ThreadStatusDto::Streaming)?;
            Ok(assistant_message)
        }
    })
    .await
    {
        Ok(assistant_message) => assistant_message,
        Err(error) => {
            state.turns.finish(&thread.id).await;
            return Err(error);
        }
    };

    let state_cloned = state.inner().clone();
    let app_handle = app.clone();
    let assistant_message_id = assistant_message.id.clone();
    let turn_input_for_task = turn_input.clone();
    let thread_for_task = thread.clone();
    let initial_turn_model_id = effective_model_id.clone();
    let initial_events = legacy_native_migration
        .into_iter()
        .filter_map(|migration| migration.notice)
        .map(LegacyNativeMigrationNotice::into_engine_event)
        .collect::<Vec<_>>();

    tokio::spawn(async move {
        run_turn(
            app_handle,
            state_cloned,
            thread_for_task,
            engine_thread_id,
            assistant_message_id,
            initial_turn_model_id,
            turn_input_for_task,
            client_turn_id,
            cancellation,
            initial_events,
        )
        .await;
    });

    Ok(assistant_message.id)
}

#[tauri::command]
pub async fn start_codex_review(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    thread_id: String,
    target: CodexReviewTargetPayload,
    delivery: Option<CodexReviewDeliveryPayload>,
) -> Result<ThreadDto, String> {
    if state.turns.get(&thread_id).await.is_some() {
        return Err(
            "A turn is already running for this thread. Cancel it before starting a review."
                .to_string(),
        );
    }

    let db = state.db.clone();
    let source_thread = run_db(db.clone(), {
        let thread_id = thread_id.clone();
        move |db| db::threads::get_thread(db, &thread_id)
    })
    .await?
    .ok_or_else(|| format!("thread not found: {thread_id}"))?;

    if source_thread.engine_id != "codex" {
        return Err("Native review is only available for Codex threads.".to_string());
    }

    let source_engine_thread_id = source_thread
        .engine_thread_id
        .clone()
        .ok_or_else(|| "Codex review requires an initialized server-backed thread.".to_string())?;
    let effective_delivery = delivery.unwrap_or(CodexReviewDeliveryPayload::Inline);
    let (target_payload, review_message, review_title) = normalize_codex_review_target(&target)?;
    let initial_turn_model_id = thread_last_model_id(source_thread.engine_metadata.as_ref())
        .unwrap_or_else(|| source_thread.model_id.clone());
    let reasoning_effort = thread_reasoning_effort(source_thread.engine_metadata.as_ref());

    let cancellation = CancellationToken::new();
    if !state
        .turns
        .try_register(&source_thread.id, cancellation.clone())
        .await
    {
        return Err(
            "A turn is already running for this thread. Cancel it before starting a review."
                .to_string(),
        );
    }

    let (review_thread, assistant_message_id) = match run_db(db.clone(), {
        let source_thread = source_thread.clone();
        let review_message = review_message.clone();
        let review_title = review_title.clone();
        let initial_turn_model_id = initial_turn_model_id.clone();
        let reasoning_effort = reasoning_effort.clone();
        let detached = matches!(effective_delivery, CodexReviewDeliveryPayload::Detached);
        move |db| {
            let review_thread = if detached {
                let created = db::threads::create_thread(
                    db,
                    &source_thread.workspace_id,
                    source_thread.repo_id.as_deref(),
                    &source_thread.engine_id,
                    &initial_turn_model_id,
                    &review_title,
                )?;
                if let Some(metadata) = clone_codex_review_metadata(
                    source_thread.engine_metadata.as_ref(),
                    &initial_turn_model_id,
                ) {
                    db::threads::update_engine_metadata(db, &created.id, &metadata)?;
                }
                created
            } else {
                source_thread.clone()
            };

            let user_blocks = build_user_blocks(&review_message, &[], &[], false, false);
            db::messages::insert_user_message(
                db,
                &review_thread.id,
                &review_message,
                Some(serde_json::to_value(&user_blocks)?),
                Some(source_thread.engine_id.as_str()),
                Some(initial_turn_model_id.as_str()),
                reasoning_effort.as_deref(),
            )?;
            let assistant_message = db::messages::insert_assistant_placeholder(
                db,
                &review_thread.id,
                Some(source_thread.engine_id.as_str()),
                Some(initial_turn_model_id.as_str()),
                reasoning_effort.as_deref(),
            )?;
            db::threads::update_thread_status(db, &review_thread.id, ThreadStatusDto::Streaming)?;
            let updated_thread = db::threads::get_thread(db, &review_thread.id)?
                .ok_or_else(|| anyhow::anyhow!("review thread not found after setup"))?;
            Ok((updated_thread, assistant_message.id))
        }
    })
    .await
    {
        Ok(result) => result,
        Err(error) => {
            state.turns.finish(&source_thread.id).await;
            return Err(error);
        }
    };

    if matches!(effective_delivery, CodexReviewDeliveryPayload::Detached) {
        if !state
            .turns
            .try_register(&review_thread.id, cancellation.clone())
            .await
        {
            log::warn!(
                "failed to register cancellation token for detached review thread {}",
                review_thread.id
            );
        }
    }

    let state_cloned = state.inner().clone();
    let app_handle = app.clone();
    let review_thread_for_task = review_thread.clone();
    let review_target_for_task = target_payload.clone();
    let source_engine_thread_id_for_task = source_engine_thread_id.clone();
    let assistant_message_id_for_task = assistant_message_id.clone();
    let delivery_label = match effective_delivery {
        CodexReviewDeliveryPayload::Inline => "inline".to_string(),
        CodexReviewDeliveryPayload::Detached => "detached".to_string(),
    };

    tokio::spawn(async move {
        run_codex_review_turn(
            app_handle,
            state_cloned,
            source_thread,
            review_thread_for_task,
            source_engine_thread_id_for_task,
            assistant_message_id_for_task,
            initial_turn_model_id,
            review_target_for_task,
            delivery_label,
            cancellation,
        )
        .await;
    });

    Ok(review_thread)
}

#[tauri::command]
pub async fn steer_message(
    state: State<'_, AppState>,
    thread_id: String,
    message: String,
    display_message: Option<String>,
    attachments: Option<Vec<ChatAttachmentPayload>>,
    input_items: Option<Vec<ChatInputItemPayload>>,
    plan_mode: Option<bool>,
) -> Result<(), String> {
    if state.turns.get(&thread_id).await.is_none() {
        return Err(
            "No active turn is running for this thread yet. Wait for Codex to start the turn before steering."
                .to_string(),
        );
    }

    let db = state.db.clone();
    let thread = run_db(db.clone(), {
        let thread_id = thread_id.clone();
        move |db| db::threads::get_thread(db, &thread_id)
    })
    .await?
    .ok_or_else(|| format!("thread not found: {thread_id}"))?;

    if thread.engine_id != "codex" {
        return Err("Mid-turn steering is only available for Codex threads.".to_string());
    }

    let engine_thread_id = thread
        .engine_thread_id
        .clone()
        .ok_or_else(|| format!("thread `{thread_id}` has no active engine thread id"))?;
    let attachments = normalize_attachments(attachments)?;
    let input_items = normalize_input_items(message.as_str(), input_items)?;
    let display_message = display_message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let plan_mode = plan_mode.unwrap_or(false);
    let turn_input = TurnInput {
        message: message.clone(),
        attachments: attachments.clone(),
        plan_mode,
        input_items: input_items.clone(),
    };
    let effective_model_id = thread_last_model_id(thread.engine_metadata.as_ref())
        .unwrap_or_else(|| thread.model_id.clone());
    let reasoning_effort = thread_reasoning_effort(thread.engine_metadata.as_ref());
    let persisted_message = display_message.clone().unwrap_or_else(|| message.clone());
    let persisted_input_items = if display_message.is_some() {
        Vec::new()
    } else {
        input_items.clone()
    };
    let user_blocks = build_user_blocks(
        &persisted_message,
        &persisted_input_items,
        &attachments,
        plan_mode,
        true,
    );

    let user_message = run_db(db.clone(), {
        let thread_id = thread.id.clone();
        let message = persisted_message.clone();
        let user_blocks = user_blocks.clone();
        let engine_id = thread.engine_id.clone();
        let model_id = effective_model_id.clone();
        let reasoning_effort = reasoning_effort.clone();
        move |db| {
            db::messages::insert_user_message(
                db,
                &thread_id,
                &message,
                Some(serde_json::to_value(&user_blocks)?),
                Some(engine_id.as_str()),
                Some(model_id.as_str()),
                reasoning_effort.as_deref(),
            )
        }
    })
    .await?;

    if let Err(error) = state
        .engines
        .steer_message(&thread, &engine_thread_id, turn_input)
        .await
    {
        let rollback_result = run_db(db, {
            let message_id = user_message.id.clone();
            move |db| db::messages::delete_message(db, &message_id)
        })
        .await;
        if let Err(rollback_error) = rollback_result {
            log::warn!(
                "failed to roll back persisted steer message {} after steer error: {}",
                user_message.id,
                rollback_error
            );
        }

        return Err(err_to_string(error));
    }

    Ok(())
}

#[tauri::command]
pub async fn cancel_turn(state: State<'_, AppState>, thread_id: String) -> Result<(), String> {
    state.turns.cancel(&thread_id).await;

    let db = state.db.clone();
    if let Some(thread) = run_db(db.clone(), {
        let thread_id = thread_id.clone();
        move |db| db::threads::get_thread(db, &thread_id)
    })
    .await?
    {
        state
            .engines
            .interrupt(&thread)
            .await
            .map_err(err_to_string)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn respond_to_approval(
    state: State<'_, AppState>,
    thread_id: String,
    approval_id: String,
    response: Value,
) -> Result<(), String> {
    approvals::respond_to_approval(state, thread_id, approval_id, response).await
}

#[tauri::command]
pub async fn get_thread_messages(
    state: State<'_, AppState>,
    thread_id: String,
) -> Result<Vec<MessageDto>, String> {
    message_query::get_thread_messages(state, thread_id).await
}

#[tauri::command]
pub async fn get_thread_messages_window(
    state: State<'_, AppState>,
    thread_id: String,
    cursor: Option<MessageWindowCursorDto>,
    limit: Option<usize>,
) -> Result<MessageWindowDto, String> {
    message_query::get_thread_messages_window(state, thread_id, cursor, limit).await
}

#[tauri::command]
pub async fn get_message_blocks(
    state: State<'_, AppState>,
    message_id: String,
) -> Result<Option<Value>, String> {
    message_query::get_message_blocks(state, message_id).await
}

#[tauri::command]
pub async fn get_action_output(
    state: State<'_, AppState>,
    message_id: String,
    action_id: String,
) -> Result<ActionOutputDto, String> {
    message_query::get_action_output(state, message_id, action_id).await
}

#[tauri::command]
pub async fn search_messages(
    state: State<'_, AppState>,
    workspace_id: String,
    query: String,
) -> Result<Vec<SearchResultDto>, String> {
    message_query::search_messages(state, workspace_id, query).await
}

fn err_to_string(error: impl std::fmt::Display) -> String {
    format!("{error:#}")
}

#[cfg(test)]
#[path = "chat/tests.rs"]
mod tests;
