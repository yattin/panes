use super::*;

pub(super) fn is_coalescable_stream_event(event: &EngineEvent) -> bool {
    matches!(
        event,
        EngineEvent::TextDelta { .. }
            | EngineEvent::ThinkingDelta { .. }
            | EngineEvent::ActionOutputDelta { .. }
            | EngineEvent::ActionProgressUpdated { .. }
    )
}

pub(super) fn coalesced_event_content_len(event: &EngineEvent) -> usize {
    match event {
        EngineEvent::TextDelta { content }
        | EngineEvent::ThinkingDelta { content }
        | EngineEvent::ActionOutputDelta { content, .. } => content.len(),
        EngineEvent::ActionProgressUpdated { message, .. } => message.len(),
        _ => 0,
    }
}

pub(super) fn same_output_stream(left: &OutputStream, right: &OutputStream) -> bool {
    matches!(
        (left, right),
        (OutputStream::Stdout, OutputStream::Stdout)
            | (OutputStream::Stderr, OutputStream::Stderr)
            | (OutputStream::Stdin, OutputStream::Stdin)
    )
}

#[allow(clippy::result_large_err)]
pub(super) fn try_coalesce_stream_events(
    previous: EngineEvent,
    next: EngineEvent,
) -> Result<EngineEvent, (EngineEvent, EngineEvent)> {
    match (previous, next) {
        (
            EngineEvent::TextDelta { mut content },
            EngineEvent::TextDelta {
                content: next_content,
            },
        ) => {
            content.push_str(&next_content);
            Ok(EngineEvent::TextDelta { content })
        }
        (
            EngineEvent::ThinkingDelta { mut content },
            EngineEvent::ThinkingDelta {
                content: next_content,
            },
        ) => {
            content.push_str(&next_content);
            Ok(EngineEvent::ThinkingDelta { content })
        }
        (
            EngineEvent::ActionOutputDelta {
                action_id,
                stream,
                mut content,
            },
            EngineEvent::ActionOutputDelta {
                action_id: next_action_id,
                stream: next_stream,
                content: next_content,
            },
        ) => {
            if action_id == next_action_id && same_output_stream(&stream, &next_stream) {
                content.push_str(&next_content);
                Ok(EngineEvent::ActionOutputDelta {
                    action_id,
                    stream,
                    content,
                })
            } else {
                Err((
                    EngineEvent::ActionOutputDelta {
                        action_id,
                        stream,
                        content,
                    },
                    EngineEvent::ActionOutputDelta {
                        action_id: next_action_id,
                        stream: next_stream,
                        content: next_content,
                    },
                ))
            }
        }
        (
            EngineEvent::ActionProgressUpdated {
                action_id,
                message: _,
            },
            EngineEvent::ActionProgressUpdated {
                action_id: next_action_id,
                message: next_message,
            },
        ) if action_id == next_action_id => Ok(EngineEvent::ActionProgressUpdated {
            action_id,
            message: next_message,
        }),
        (previous, next) => Err((previous, next)),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn process_stream_event(
    app: &tauri::AppHandle,
    state: &AppState,
    thread: &ThreadDto,
    assistant_message_id: &str,
    stream_event_topic: &str,
    approval_event_topic: &str,
    event: &EngineEvent,
    blocks: &mut Vec<ContentBlock>,
    action_index: &mut HashMap<String, usize>,
    approval_index: &mut HashMap<String, usize>,
    max_output_chars: usize,
) -> EventProgress {
    let mut normalized_event = event.clone();
    match &mut normalized_event {
        EngineEvent::ActionOutputDelta { content, .. } => {
            *content = trim_action_output_delta_content(content);
        }
        EngineEvent::ActionCompleted { result, .. } => {
            truncate_action_result_output(result, max_output_chars);
        }
        EngineEvent::DiffUpdated { diff, .. } => {
            *diff = truncate_chars(diff, STREAMED_DIFF_MAX_CHARS);
        }
        _ => {}
    }

    let _ = app.emit(stream_event_topic, &normalized_event);
    if matches!(&normalized_event, EngineEvent::ApprovalRequested { .. }) {
        let _ = app.emit(approval_event_topic, &normalized_event);
    }

    if state.config.debug.persist_engine_event_logs {
        let log_event = engine_event_for_debug_log(&normalized_event);
        if let Ok(value) = serde_json::to_value(&log_event) {
            if let Err(error) = run_db(state.db.clone(), {
                let thread_id = thread.id.clone();
                let assistant_message_id = assistant_message_id.to_string();
                let value = value.clone();
                move |db| {
                    db::actions::append_event_log(db, &thread_id, &assistant_message_id, &value)
                }
            })
            .await
            {
                log::warn!("failed to append engine event log: {error}");
            }
        }
    }

    match &normalized_event {
        EngineEvent::ActionStarted {
            action_id,
            engine_action_id,
            action_type,
            summary,
            details,
            ..
        } => {
            if let Err(error) = run_db(state.db.clone(), {
                let action_id = action_id.clone();
                let thread_id = thread.id.clone();
                let assistant_message_id = assistant_message_id.to_string();
                let engine_action_id = engine_action_id.clone();
                let action_type = action_type.clone();
                let summary = summary.clone();
                let details = details.clone();
                move |db| {
                    db::actions::insert_action_started(
                        db,
                        &action_id,
                        &thread_id,
                        &assistant_message_id,
                        engine_action_id.as_deref(),
                        &action_type,
                        &summary,
                        &details,
                    )
                }
            })
            .await
            {
                log::warn!("failed to persist action start: {error}");
            }
        }
        EngineEvent::ActionCompleted { action_id, result } => {
            if let Err(error) = run_db(state.db.clone(), {
                let action_id = action_id.clone();
                let result = result.clone();
                move |db| db::actions::update_action_completed(db, &action_id, &result)
            })
            .await
            {
                log::warn!("failed to persist action completion: {error}");
            }
        }
        EngineEvent::ApprovalRequested {
            approval_id,
            action_type,
            summary,
            details,
        } => {
            if let Err(error) = run_db(state.db.clone(), {
                let approval_id = approval_id.clone();
                let thread_id = thread.id.clone();
                let assistant_message_id = assistant_message_id.to_string();
                let action_type = action_type.clone();
                let summary = summary.clone();
                let details = details.clone();
                move |db| {
                    db::actions::insert_approval(
                        db,
                        &approval_id,
                        &thread_id,
                        &assistant_message_id,
                        &action_type,
                        &summary,
                        &details,
                    )
                }
            })
            .await
            {
                log::warn!("failed to persist approval: {error}");
            }
        }
        _ => {}
    }

    apply_event_to_blocks(
        blocks,
        action_index,
        approval_index,
        &normalized_event,
        max_output_chars,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_stream_progress(
    progress: EventProgress,
    message_status: &mut MessageStatusDto,
    thread_status: &mut ThreadStatusDto,
    turn_model_id: &mut String,
    token_usage: &mut Option<(u64, u64)>,
    blocks_dirty: &mut bool,
    message_state_dirty: &mut bool,
    thread_status_dirty: &mut bool,
    turn_model_dirty: &mut bool,
) -> bool {
    if progress.blocks_changed {
        *blocks_dirty = true;
    }

    if let Some(status) = progress.message_status {
        if *message_status != status {
            *message_status = status;
            *message_state_dirty = true;
        }
    }

    if let Some(status) = progress.thread_status {
        if *thread_status != status {
            *thread_status = status;
            *thread_status_dirty = true;
        }
    }

    if let Some(next_turn_model_id) = progress.turn_model_id {
        if *turn_model_id != next_turn_model_id {
            *turn_model_id = next_turn_model_id;
            *turn_model_dirty = true;
        }
    }

    if let Some(tokens) = progress.token_usage {
        *token_usage = Some(tokens);
    }

    progress.force_persist
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn flush_stream_state(
    state: &AppState,
    thread: &ThreadDto,
    assistant_message_id: &str,
    blocks: &[ContentBlock],
    message_status: &MessageStatusDto,
    thread_status: &ThreadStatusDto,
    turn_model_id: &str,
    blocks_dirty: &mut bool,
    message_state_dirty: &mut bool,
    thread_status_dirty: &mut bool,
    turn_model_dirty: &mut bool,
    last_persisted_thread_status: &mut ThreadStatusDto,
    last_persist_at: &mut Instant,
    last_blocks_persist_at: &mut Instant,
    force: bool,
) {
    if !*blocks_dirty && !*message_state_dirty && !*thread_status_dirty && !*turn_model_dirty {
        return;
    }

    let now = Instant::now();

    if *thread_status_dirty && *last_persisted_thread_status == *thread_status {
        *thread_status_dirty = false;
    }

    let should_flush_state =
        force || now.duration_since(*last_persist_at) >= STREAM_DB_FLUSH_INTERVAL;
    let should_flush_blocks =
        force || now.duration_since(*last_blocks_persist_at) >= STREAM_DB_BLOCKS_FLUSH_INTERVAL;

    if !should_flush_blocks && !should_flush_state {
        return;
    }

    let mut did_flush_state = false;
    let mut did_flush_blocks = false;

    if *blocks_dirty && should_flush_blocks {
        match serde_json::to_string(blocks) {
            Ok(blocks_json) => {
                if let Err(error) = run_db(state.db.clone(), {
                    let assistant_message_id = assistant_message_id.to_string();
                    let message_status = message_status.clone();
                    let turn_model_id = turn_model_id.to_string();
                    move |db| {
                        db::messages::update_assistant_blocks_json(
                            db,
                            &assistant_message_id,
                            &blocks_json,
                            message_status,
                            Some(turn_model_id.as_str()),
                        )
                    }
                })
                .await
                {
                    log::warn!("failed to persist assistant stream blocks: {error}");
                } else {
                    *blocks_dirty = false;
                    *message_state_dirty = false;
                    *turn_model_dirty = false;
                    did_flush_blocks = true;
                    did_flush_state = true;
                }
            }
            Err(error) => {
                log::warn!("failed to serialize assistant stream blocks: {error}");
            }
        }
    } else if *message_state_dirty && should_flush_state {
        if let Err(error) = run_db(state.db.clone(), {
            let assistant_message_id = assistant_message_id.to_string();
            let message_status = message_status.clone();
            move |db| {
                db::messages::update_assistant_status(db, &assistant_message_id, message_status)
            }
        })
        .await
        {
            log::warn!("failed to persist assistant stream status: {error}");
        } else {
            *message_state_dirty = false;
            did_flush_state = true;
        }
    }

    if *turn_model_dirty && should_flush_state {
        if let Err(error) = run_db(state.db.clone(), {
            let assistant_message_id = assistant_message_id.to_string();
            let turn_model_id = turn_model_id.to_string();
            move |db| {
                db::messages::update_assistant_turn_model_id(
                    db,
                    &assistant_message_id,
                    &turn_model_id,
                )
            }
        })
        .await
        {
            log::warn!("failed to persist assistant turn model id during stream: {error}");
        } else {
            *turn_model_dirty = false;
            did_flush_state = true;
        }
    }

    if *thread_status_dirty && should_flush_state && *last_persisted_thread_status != *thread_status
    {
        if let Err(error) = run_db(state.db.clone(), {
            let thread_id = thread.id.clone();
            let thread_status = thread_status.clone();
            move |db| db::threads::update_thread_status(db, &thread_id, thread_status)
        })
        .await
        {
            log::warn!("failed to persist thread status during stream: {error}");
        } else {
            *last_persisted_thread_status = thread_status.clone();
            *thread_status_dirty = false;
            did_flush_state = true;
        }
    }

    if did_flush_blocks {
        *last_blocks_persist_at = now;
    }
    if did_flush_state {
        *last_persist_at = now;
    }
}
