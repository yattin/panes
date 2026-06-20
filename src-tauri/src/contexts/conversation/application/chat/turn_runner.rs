use super::*;

pub(super) async fn run_turn(
    app: tauri::AppHandle,
    state: AppState,
    thread: crate::models::ThreadDto,
    engine_thread_id: String,
    assistant_message_id: String,
    initial_turn_model_id: String,
    turn_input: TurnInput,
    client_turn_id: Option<String>,
    cancellation: CancellationToken,
) {
    let max_output_chars = state.config.debug.max_action_output_chars;
    let (event_tx, mut event_rx) = mpsc::channel::<EngineEvent>(ENGINE_EVENT_QUEUE_CAPACITY);

    let engines = state.engines.clone();
    let thread_for_engine = thread.clone();
    let input_for_engine = turn_input.clone();
    let engine_thread_for_engine = engine_thread_id.clone();
    let cancellation_for_engine = cancellation.clone();

    let engine_task = tokio::spawn(async move {
        engines
            .send_message(
                &thread_for_engine,
                &engine_thread_for_engine,
                input_for_engine,
                event_tx,
                cancellation_for_engine,
            )
            .await
    });

    let mut blocks: Vec<ContentBlock> = Vec::new();
    let mut action_index: HashMap<String, usize> = HashMap::new();
    let mut approval_index: HashMap<String, usize> = HashMap::new();
    let mut message_status = MessageStatusDto::Streaming;
    let mut thread_status = ThreadStatusDto::Streaming;
    let mut turn_model_id = initial_turn_model_id;
    let mut token_usage: Option<(u64, u64)> = None;
    let mut blocks_dirty = false;
    let mut message_state_dirty = false;
    let mut thread_status_dirty = false;
    let mut turn_model_dirty = false;
    let mut last_persist_at = Instant::now();
    let mut last_blocks_persist_at = Instant::now();
    let mut last_persisted_thread_status = thread_status.clone();
    let stream_event_topic = format!("stream-event-{}", thread.id);
    let approval_event_topic = format!("approval-request-{}", thread.id);
    let mut pending_event: Option<EngineEvent> = None;

    let initial_turn_started_event = EngineEvent::TurnStarted { client_turn_id };
    let initial_progress = process_stream_event(
        &app,
        &state,
        &thread,
        &assistant_message_id,
        &stream_event_topic,
        &approval_event_topic,
        &initial_turn_started_event,
        &mut blocks,
        &mut action_index,
        &mut approval_index,
        max_output_chars,
    )
    .await;
    let initial_force_persist = apply_stream_progress(
        initial_progress,
        &mut message_status,
        &mut thread_status,
        &mut turn_model_id,
        &mut token_usage,
        &mut blocks_dirty,
        &mut message_state_dirty,
        &mut thread_status_dirty,
        &mut turn_model_dirty,
    );
    flush_stream_state(
        &state,
        &thread,
        &assistant_message_id,
        &blocks,
        &message_status,
        &thread_status,
        &turn_model_id,
        &mut blocks_dirty,
        &mut message_state_dirty,
        &mut thread_status_dirty,
        &mut turn_model_dirty,
        &mut last_persisted_thread_status,
        &mut last_persist_at,
        &mut last_blocks_persist_at,
        initial_force_persist,
    )
    .await;

    loop {
        let incoming_event = if pending_event.is_some() {
            match tokio::time::timeout(STREAM_EVENT_COALESCE_IDLE_FLUSH_INTERVAL, event_rx.recv())
                .await
            {
                Ok(event) => event,
                Err(_) => {
                    if let Some(event) = pending_event.take() {
                        let progress = process_stream_event(
                            &app,
                            &state,
                            &thread,
                            &assistant_message_id,
                            &stream_event_topic,
                            &approval_event_topic,
                            &event,
                            &mut blocks,
                            &mut action_index,
                            &mut approval_index,
                            max_output_chars,
                        )
                        .await;
                        let force_persist = apply_stream_progress(
                            progress,
                            &mut message_status,
                            &mut thread_status,
                            &mut turn_model_id,
                            &mut token_usage,
                            &mut blocks_dirty,
                            &mut message_state_dirty,
                            &mut thread_status_dirty,
                            &mut turn_model_dirty,
                        );
                        flush_stream_state(
                            &state,
                            &thread,
                            &assistant_message_id,
                            &blocks,
                            &message_status,
                            &thread_status,
                            &turn_model_id,
                            &mut blocks_dirty,
                            &mut message_state_dirty,
                            &mut thread_status_dirty,
                            &mut turn_model_dirty,
                            &mut last_persisted_thread_status,
                            &mut last_persist_at,
                            &mut last_blocks_persist_at,
                            force_persist,
                        )
                        .await;
                    }
                    continue;
                }
            }
        } else {
            event_rx.recv().await
        };

        let Some(incoming_event) = incoming_event else {
            break;
        };

        let mut current_event = incoming_event;

        loop {
            if let Some(previous_event) = pending_event.take() {
                match try_coalesce_stream_events(previous_event, current_event) {
                    Ok(merged_event) => {
                        if coalesced_event_content_len(&merged_event)
                            >= STREAM_EVENT_COALESCE_MAX_CHARS
                        {
                            let progress = process_stream_event(
                                &app,
                                &state,
                                &thread,
                                &assistant_message_id,
                                &stream_event_topic,
                                &approval_event_topic,
                                &merged_event,
                                &mut blocks,
                                &mut action_index,
                                &mut approval_index,
                                max_output_chars,
                            )
                            .await;
                            let force_persist = apply_stream_progress(
                                progress,
                                &mut message_status,
                                &mut thread_status,
                                &mut turn_model_id,
                                &mut token_usage,
                                &mut blocks_dirty,
                                &mut message_state_dirty,
                                &mut thread_status_dirty,
                                &mut turn_model_dirty,
                            );
                            flush_stream_state(
                                &state,
                                &thread,
                                &assistant_message_id,
                                &blocks,
                                &message_status,
                                &thread_status,
                                &turn_model_id,
                                &mut blocks_dirty,
                                &mut message_state_dirty,
                                &mut thread_status_dirty,
                                &mut turn_model_dirty,
                                &mut last_persisted_thread_status,
                                &mut last_persist_at,
                                &mut last_blocks_persist_at,
                                force_persist,
                            )
                            .await;
                        } else {
                            pending_event = Some(merged_event);
                        }
                        break;
                    }
                    Err((unmerged_previous_event, unmerged_current_event)) => {
                        let progress = process_stream_event(
                            &app,
                            &state,
                            &thread,
                            &assistant_message_id,
                            &stream_event_topic,
                            &approval_event_topic,
                            &unmerged_previous_event,
                            &mut blocks,
                            &mut action_index,
                            &mut approval_index,
                            max_output_chars,
                        )
                        .await;
                        let force_persist = apply_stream_progress(
                            progress,
                            &mut message_status,
                            &mut thread_status,
                            &mut turn_model_id,
                            &mut token_usage,
                            &mut blocks_dirty,
                            &mut message_state_dirty,
                            &mut thread_status_dirty,
                            &mut turn_model_dirty,
                        );
                        flush_stream_state(
                            &state,
                            &thread,
                            &assistant_message_id,
                            &blocks,
                            &message_status,
                            &thread_status,
                            &turn_model_id,
                            &mut blocks_dirty,
                            &mut message_state_dirty,
                            &mut thread_status_dirty,
                            &mut turn_model_dirty,
                            &mut last_persisted_thread_status,
                            &mut last_persist_at,
                            &mut last_blocks_persist_at,
                            force_persist,
                        )
                        .await;
                        current_event = unmerged_current_event;
                    }
                }
            } else if is_coalescable_stream_event(&current_event) {
                pending_event = Some(current_event);
                break;
            } else {
                let progress = process_stream_event(
                    &app,
                    &state,
                    &thread,
                    &assistant_message_id,
                    &stream_event_topic,
                    &approval_event_topic,
                    &current_event,
                    &mut blocks,
                    &mut action_index,
                    &mut approval_index,
                    max_output_chars,
                )
                .await;
                let force_persist = apply_stream_progress(
                    progress,
                    &mut message_status,
                    &mut thread_status,
                    &mut turn_model_id,
                    &mut token_usage,
                    &mut blocks_dirty,
                    &mut message_state_dirty,
                    &mut thread_status_dirty,
                    &mut turn_model_dirty,
                );
                flush_stream_state(
                    &state,
                    &thread,
                    &assistant_message_id,
                    &blocks,
                    &message_status,
                    &thread_status,
                    &turn_model_id,
                    &mut blocks_dirty,
                    &mut message_state_dirty,
                    &mut thread_status_dirty,
                    &mut turn_model_dirty,
                    &mut last_persisted_thread_status,
                    &mut last_persist_at,
                    &mut last_blocks_persist_at,
                    force_persist,
                )
                .await;
                break;
            }
        }
    }

    if let Some(event) = pending_event.take() {
        let progress = process_stream_event(
            &app,
            &state,
            &thread,
            &assistant_message_id,
            &stream_event_topic,
            &approval_event_topic,
            &event,
            &mut blocks,
            &mut action_index,
            &mut approval_index,
            max_output_chars,
        )
        .await;
        let force_persist = apply_stream_progress(
            progress,
            &mut message_status,
            &mut thread_status,
            &mut turn_model_id,
            &mut token_usage,
            &mut blocks_dirty,
            &mut message_state_dirty,
            &mut thread_status_dirty,
            &mut turn_model_dirty,
        );
        flush_stream_state(
            &state,
            &thread,
            &assistant_message_id,
            &blocks,
            &message_status,
            &thread_status,
            &turn_model_id,
            &mut blocks_dirty,
            &mut message_state_dirty,
            &mut thread_status_dirty,
            &mut turn_model_dirty,
            &mut last_persisted_thread_status,
            &mut last_persist_at,
            &mut last_blocks_persist_at,
            force_persist,
        )
        .await;
    }

    match engine_task.await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            blocks.push(ContentBlock::Error {
                message: format!("Engine error: {error}"),
            });
            blocks_dirty = true;
            if message_status != MessageStatusDto::Error {
                message_status = MessageStatusDto::Error;
                message_state_dirty = true;
            }
            if thread_status != ThreadStatusDto::Error {
                thread_status = ThreadStatusDto::Error;
                thread_status_dirty = true;
            }
            let _ = app.emit(
                &stream_event_topic,
                EngineEvent::Error {
                    message: format!("{error}"),
                    recoverable: false,
                },
            );
        }
        Err(error) => {
            blocks.push(ContentBlock::Error {
                message: format!("Engine task join error: {error}"),
            });
            blocks_dirty = true;
            if message_status != MessageStatusDto::Error {
                message_status = MessageStatusDto::Error;
                message_state_dirty = true;
            }
            if thread_status != ThreadStatusDto::Error {
                thread_status = ThreadStatusDto::Error;
                thread_status_dirty = true;
            }
        }
    }

    if cancellation.is_cancelled() && matches!(message_status, MessageStatusDto::Streaming) {
        message_status = MessageStatusDto::Interrupted;
        message_state_dirty = true;
        thread_status = ThreadStatusDto::Idle;
        thread_status_dirty = true;
    }

    flush_stream_state(
        &state,
        &thread,
        &assistant_message_id,
        &blocks,
        &message_status,
        &thread_status,
        &turn_model_id,
        &mut blocks_dirty,
        &mut message_state_dirty,
        &mut thread_status_dirty,
        &mut turn_model_dirty,
        &mut last_persisted_thread_status,
        &mut last_persist_at,
        &mut last_blocks_persist_at,
        true,
    )
    .await;

    state.turns.finish(&thread.id).await;

    if let Err(error) = run_db(state.db.clone(), {
        let assistant_message_id = assistant_message_id.clone();
        let message_status = message_status.clone();
        let token_usage = token_usage;
        move |db| {
            db::messages::complete_assistant_message(
                db,
                &assistant_message_id,
                message_status,
                token_usage,
                Some(turn_model_id.as_str()),
            )
        }
    })
    .await
    {
        log::warn!("failed to complete assistant message: {error}");
    }

    if matches!(message_status, MessageStatusDto::Completed) {
        if let Err(error) = run_db(state.db.clone(), {
            let thread_id = thread.id.clone();
            let token_usage = token_usage;
            move |db| db::threads::bump_message_counters(db, &thread_id, token_usage)
        })
        .await
        {
            log::warn!("failed to bump thread counters: {error}");
        }
    }

    let _ =
        maybe_update_thread_title(&state, &thread, &engine_thread_id, &turn_input.message).await;

    let latest_thread = run_db(state.db.clone(), {
        let thread_id = thread.id.clone();
        move |db| db::threads::get_thread(db, &thread_id)
    })
    .await
    .unwrap_or_else(|error| {
        log::warn!("failed to load thread before final thread-updated emit: {error}");
        None
    });

    let (thread_updated_event, final_thread) = build_final_thread_event(latest_thread, &thread);
    let _ = app.emit("thread-updated", thread_updated_event);
    if let Some(final_thread) = final_thread.as_ref() {
        emit_chat_turn_finished(&app, final_thread, &message_status, &blocks);
    }
}

pub(super) async fn run_codex_review_turn(
    app: tauri::AppHandle,
    state: AppState,
    source_thread: crate::models::ThreadDto,
    review_thread: crate::models::ThreadDto,
    source_engine_thread_id: String,
    assistant_message_id: String,
    initial_turn_model_id: String,
    target: Value,
    delivery: String,
    cancellation: CancellationToken,
) {
    let max_output_chars = state.config.debug.max_action_output_chars;
    let (event_tx, mut event_rx) = mpsc::channel::<EngineEvent>(ENGINE_EVENT_QUEUE_CAPACITY);
    let (started_tx, started_rx) = oneshot::channel();

    let engines = state.engines.clone();
    let source_engine_thread_id_for_engine = source_engine_thread_id.clone();
    let target_for_engine = target.clone();
    let delivery_for_engine = delivery.clone();
    let cancellation_for_engine = cancellation.clone();

    let engine_task = tokio::spawn(async move {
        engines
            .start_codex_review(
                &source_engine_thread_id_for_engine,
                target_for_engine,
                Some(delivery_for_engine.as_str()),
                event_tx,
                cancellation_for_engine,
                started_tx,
            )
            .await
    });

    let state_for_started = state.clone();
    let app_for_started = app.clone();
    let review_thread_for_started = review_thread.clone();
    let started_task = tokio::spawn(async move {
        let Ok(started) = started_rx.await else {
            return;
        };

        let updated_thread = match run_db(state_for_started.db.clone(), {
            let review_thread_id = review_thread_for_started.id.clone();
            let review_thread_engine_id = review_thread_for_started.engine_thread_id.clone();
            let review_thread_model_id = review_thread_for_started.model_id.clone();
            let review_thread_metadata = review_thread_for_started.engine_metadata.clone();
            let review_thread_status = review_thread_for_started.status.clone();
            let review_thread_title = review_thread_for_started.title.clone();
            let review_thread_workspace_id = review_thread_for_started.workspace_id.clone();
            let review_thread_repo_id = review_thread_for_started.repo_id.clone();
            move |db| {
                if review_thread_engine_id.as_deref() == Some(started.review_thread_id.as_str()) {
                    return db::threads::get_thread(db, &review_thread_id)?.ok_or_else(|| {
                        anyhow::anyhow!(
                            "review thread not found after review/start: {review_thread_id}"
                        )
                    });
                }

                db::threads::set_engine_thread_id(db, &review_thread_id, &started.review_thread_id)?;
                let current = db::threads::get_thread(db, &review_thread_id)?.ok_or_else(|| {
                    anyhow::anyhow!("review thread not found after engine thread update")
                })?;
                let metadata = current.engine_metadata.or(review_thread_metadata.clone());
                db::threads::update_thread_runtime_snapshot(
                    db,
                    &review_thread_id,
                    Some(&review_thread_title),
                    Some(review_thread_status.clone()),
                    metadata.as_ref(),
                )?;
                db::threads::get_thread(db, &review_thread_id)?.ok_or_else(|| {
                    anyhow::anyhow!(
                        "review thread not found after runtime snapshot update: {review_thread_workspace_id}:{review_thread_repo_id:?}:{review_thread_model_id}"
                    )
                })
            }
        })
        .await
        {
            Ok(thread) => thread,
            Err(error) => {
                log::warn!("failed to persist codex review thread id: {error}");
                return;
            }
        };

        let _ = app_for_started.emit(
            "thread-updated",
            ThreadUpdatedEvent {
                thread_id: updated_thread.id.clone(),
                workspace_id: updated_thread.workspace_id.clone(),
                thread: Some(updated_thread),
            },
        );
    });

    let mut blocks: Vec<ContentBlock> = Vec::new();
    let mut action_index: HashMap<String, usize> = HashMap::new();
    let mut approval_index: HashMap<String, usize> = HashMap::new();
    let mut message_status = MessageStatusDto::Streaming;
    let mut thread_status = ThreadStatusDto::Streaming;
    let mut turn_model_id = initial_turn_model_id;
    let mut token_usage: Option<(u64, u64)> = None;
    let mut blocks_dirty = false;
    let mut message_state_dirty = false;
    let mut thread_status_dirty = false;
    let mut turn_model_dirty = false;
    let mut last_persist_at = Instant::now();
    let mut last_blocks_persist_at = Instant::now();
    let mut last_persisted_thread_status = thread_status.clone();
    let stream_event_topic = format!("stream-event-{}", review_thread.id);
    let approval_event_topic = format!("approval-request-{}", review_thread.id);
    let mut pending_event: Option<EngineEvent> = None;

    let initial_turn_started_event = EngineEvent::TurnStarted {
        client_turn_id: None,
    };
    let initial_progress = process_stream_event(
        &app,
        &state,
        &review_thread,
        &assistant_message_id,
        &stream_event_topic,
        &approval_event_topic,
        &initial_turn_started_event,
        &mut blocks,
        &mut action_index,
        &mut approval_index,
        max_output_chars,
    )
    .await;
    let initial_force_persist = apply_stream_progress(
        initial_progress,
        &mut message_status,
        &mut thread_status,
        &mut turn_model_id,
        &mut token_usage,
        &mut blocks_dirty,
        &mut message_state_dirty,
        &mut thread_status_dirty,
        &mut turn_model_dirty,
    );
    flush_stream_state(
        &state,
        &review_thread,
        &assistant_message_id,
        &blocks,
        &message_status,
        &thread_status,
        &turn_model_id,
        &mut blocks_dirty,
        &mut message_state_dirty,
        &mut thread_status_dirty,
        &mut turn_model_dirty,
        &mut last_persisted_thread_status,
        &mut last_persist_at,
        &mut last_blocks_persist_at,
        initial_force_persist,
    )
    .await;

    if let Err(error) = started_task.await {
        log::warn!("failed to join codex review start task: {error}");
    }

    loop {
        let incoming_event = if pending_event.is_some() {
            match tokio::time::timeout(STREAM_EVENT_COALESCE_IDLE_FLUSH_INTERVAL, event_rx.recv())
                .await
            {
                Ok(event) => event,
                Err(_) => {
                    if let Some(event) = pending_event.take() {
                        let progress = process_stream_event(
                            &app,
                            &state,
                            &review_thread,
                            &assistant_message_id,
                            &stream_event_topic,
                            &approval_event_topic,
                            &event,
                            &mut blocks,
                            &mut action_index,
                            &mut approval_index,
                            max_output_chars,
                        )
                        .await;
                        let force_persist = apply_stream_progress(
                            progress,
                            &mut message_status,
                            &mut thread_status,
                            &mut turn_model_id,
                            &mut token_usage,
                            &mut blocks_dirty,
                            &mut message_state_dirty,
                            &mut thread_status_dirty,
                            &mut turn_model_dirty,
                        );
                        flush_stream_state(
                            &state,
                            &review_thread,
                            &assistant_message_id,
                            &blocks,
                            &message_status,
                            &thread_status,
                            &turn_model_id,
                            &mut blocks_dirty,
                            &mut message_state_dirty,
                            &mut thread_status_dirty,
                            &mut turn_model_dirty,
                            &mut last_persisted_thread_status,
                            &mut last_persist_at,
                            &mut last_blocks_persist_at,
                            force_persist,
                        )
                        .await;
                    }
                    continue;
                }
            }
        } else {
            event_rx.recv().await
        };

        let Some(incoming_event) = incoming_event else {
            break;
        };

        let mut current_event = incoming_event;

        loop {
            if let Some(previous_event) = pending_event.take() {
                match try_coalesce_stream_events(previous_event, current_event) {
                    Ok(merged_event) => {
                        if coalesced_event_content_len(&merged_event)
                            >= STREAM_EVENT_COALESCE_MAX_CHARS
                        {
                            let progress = process_stream_event(
                                &app,
                                &state,
                                &review_thread,
                                &assistant_message_id,
                                &stream_event_topic,
                                &approval_event_topic,
                                &merged_event,
                                &mut blocks,
                                &mut action_index,
                                &mut approval_index,
                                max_output_chars,
                            )
                            .await;
                            let force_persist = apply_stream_progress(
                                progress,
                                &mut message_status,
                                &mut thread_status,
                                &mut turn_model_id,
                                &mut token_usage,
                                &mut blocks_dirty,
                                &mut message_state_dirty,
                                &mut thread_status_dirty,
                                &mut turn_model_dirty,
                            );
                            flush_stream_state(
                                &state,
                                &review_thread,
                                &assistant_message_id,
                                &blocks,
                                &message_status,
                                &thread_status,
                                &turn_model_id,
                                &mut blocks_dirty,
                                &mut message_state_dirty,
                                &mut thread_status_dirty,
                                &mut turn_model_dirty,
                                &mut last_persisted_thread_status,
                                &mut last_persist_at,
                                &mut last_blocks_persist_at,
                                force_persist,
                            )
                            .await;
                        } else {
                            pending_event = Some(merged_event);
                        }
                        break;
                    }
                    Err((unmerged_previous_event, unmerged_current_event)) => {
                        let progress = process_stream_event(
                            &app,
                            &state,
                            &review_thread,
                            &assistant_message_id,
                            &stream_event_topic,
                            &approval_event_topic,
                            &unmerged_previous_event,
                            &mut blocks,
                            &mut action_index,
                            &mut approval_index,
                            max_output_chars,
                        )
                        .await;
                        let force_persist = apply_stream_progress(
                            progress,
                            &mut message_status,
                            &mut thread_status,
                            &mut turn_model_id,
                            &mut token_usage,
                            &mut blocks_dirty,
                            &mut message_state_dirty,
                            &mut thread_status_dirty,
                            &mut turn_model_dirty,
                        );
                        flush_stream_state(
                            &state,
                            &review_thread,
                            &assistant_message_id,
                            &blocks,
                            &message_status,
                            &thread_status,
                            &turn_model_id,
                            &mut blocks_dirty,
                            &mut message_state_dirty,
                            &mut thread_status_dirty,
                            &mut turn_model_dirty,
                            &mut last_persisted_thread_status,
                            &mut last_persist_at,
                            &mut last_blocks_persist_at,
                            force_persist,
                        )
                        .await;
                        current_event = unmerged_current_event;
                    }
                }
            } else if is_coalescable_stream_event(&current_event) {
                pending_event = Some(current_event);
                break;
            } else {
                let progress = process_stream_event(
                    &app,
                    &state,
                    &review_thread,
                    &assistant_message_id,
                    &stream_event_topic,
                    &approval_event_topic,
                    &current_event,
                    &mut blocks,
                    &mut action_index,
                    &mut approval_index,
                    max_output_chars,
                )
                .await;
                let force_persist = apply_stream_progress(
                    progress,
                    &mut message_status,
                    &mut thread_status,
                    &mut turn_model_id,
                    &mut token_usage,
                    &mut blocks_dirty,
                    &mut message_state_dirty,
                    &mut thread_status_dirty,
                    &mut turn_model_dirty,
                );
                flush_stream_state(
                    &state,
                    &review_thread,
                    &assistant_message_id,
                    &blocks,
                    &message_status,
                    &thread_status,
                    &turn_model_id,
                    &mut blocks_dirty,
                    &mut message_state_dirty,
                    &mut thread_status_dirty,
                    &mut turn_model_dirty,
                    &mut last_persisted_thread_status,
                    &mut last_persist_at,
                    &mut last_blocks_persist_at,
                    force_persist,
                )
                .await;
                break;
            }
        }
    }

    if let Some(event) = pending_event.take() {
        let progress = process_stream_event(
            &app,
            &state,
            &review_thread,
            &assistant_message_id,
            &stream_event_topic,
            &approval_event_topic,
            &event,
            &mut blocks,
            &mut action_index,
            &mut approval_index,
            max_output_chars,
        )
        .await;
        let force_persist = apply_stream_progress(
            progress,
            &mut message_status,
            &mut thread_status,
            &mut turn_model_id,
            &mut token_usage,
            &mut blocks_dirty,
            &mut message_state_dirty,
            &mut thread_status_dirty,
            &mut turn_model_dirty,
        );
        flush_stream_state(
            &state,
            &review_thread,
            &assistant_message_id,
            &blocks,
            &message_status,
            &thread_status,
            &turn_model_id,
            &mut blocks_dirty,
            &mut message_state_dirty,
            &mut thread_status_dirty,
            &mut turn_model_dirty,
            &mut last_persisted_thread_status,
            &mut last_persist_at,
            &mut last_blocks_persist_at,
            force_persist,
        )
        .await;
    }

    match engine_task.await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            blocks.push(ContentBlock::Error {
                message: format!("Engine error: {error}"),
            });
            blocks_dirty = true;
            if message_status != MessageStatusDto::Error {
                message_status = MessageStatusDto::Error;
                message_state_dirty = true;
            }
            if thread_status != ThreadStatusDto::Error {
                thread_status = ThreadStatusDto::Error;
                thread_status_dirty = true;
            }
            let _ = app.emit(
                &stream_event_topic,
                EngineEvent::Error {
                    message: format!("{error}"),
                    recoverable: false,
                },
            );
        }
        Err(error) => {
            blocks.push(ContentBlock::Error {
                message: format!("Engine task join error: {error}"),
            });
            blocks_dirty = true;
            if message_status != MessageStatusDto::Error {
                message_status = MessageStatusDto::Error;
                message_state_dirty = true;
            }
            if thread_status != ThreadStatusDto::Error {
                thread_status = ThreadStatusDto::Error;
                thread_status_dirty = true;
            }
        }
    }

    if cancellation.is_cancelled() && matches!(message_status, MessageStatusDto::Streaming) {
        message_status = MessageStatusDto::Interrupted;
        message_state_dirty = true;
        thread_status = ThreadStatusDto::Idle;
        thread_status_dirty = true;
    }

    flush_stream_state(
        &state,
        &review_thread,
        &assistant_message_id,
        &blocks,
        &message_status,
        &thread_status,
        &turn_model_id,
        &mut blocks_dirty,
        &mut message_state_dirty,
        &mut thread_status_dirty,
        &mut turn_model_dirty,
        &mut last_persisted_thread_status,
        &mut last_persist_at,
        &mut last_blocks_persist_at,
        true,
    )
    .await;

    state.turns.finish(&source_thread.id).await;
    state.turns.finish(&review_thread.id).await;

    if let Err(error) = run_db(state.db.clone(), {
        let assistant_message_id = assistant_message_id.clone();
        let message_status = message_status.clone();
        let token_usage = token_usage;
        move |db| {
            db::messages::complete_assistant_message(
                db,
                &assistant_message_id,
                message_status,
                token_usage,
                Some(turn_model_id.as_str()),
            )
        }
    })
    .await
    {
        log::warn!("failed to complete review assistant message: {error}");
    }

    if matches!(message_status, MessageStatusDto::Completed) {
        if let Err(error) = run_db(state.db.clone(), {
            let thread_id = review_thread.id.clone();
            let token_usage = token_usage;
            move |db| db::threads::bump_message_counters(db, &thread_id, token_usage)
        })
        .await
        {
            log::warn!("failed to bump review thread counters: {error}");
        }
    }

    let latest_review_thread = run_db(state.db.clone(), {
        let review_thread_id = review_thread.id.clone();
        move |db| db::threads::get_thread(db, &review_thread_id)
    })
    .await
    .unwrap_or_else(|error| {
        log::warn!("failed to load review thread before final thread-updated emit: {error}");
        None
    });
    let (thread_updated_event, final_review_thread) =
        build_final_thread_event(latest_review_thread, &review_thread);
    let _ = app.emit("thread-updated", thread_updated_event);
    if let Some(final_review_thread) = final_review_thread.as_ref() {
        emit_chat_turn_finished(&app, final_review_thread, &message_status, &blocks);
    }
}
