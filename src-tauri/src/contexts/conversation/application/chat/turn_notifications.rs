use super::*;

pub(super) async fn maybe_update_thread_title(
    state: &AppState,
    thread: &ThreadDto,
    engine_thread_id: &str,
    user_message: &str,
) -> Option<ThreadDto> {
    if !should_autotitle_thread(thread) {
        return None;
    }

    let candidate = state
        .engines
        .read_thread_preview(thread, engine_thread_id)
        .await
        .as_deref()
        .and_then(normalize_thread_title)
        .or_else(|| normalize_thread_title(user_message))?;

    if candidate == thread.title {
        return None;
    }

    let updated_thread = match run_db(state.db.clone(), {
        let thread_id = thread.id.clone();
        let candidate = candidate.clone();
        move |db| {
            db::threads::update_thread_title(db, &thread_id, &candidate)?;
            db::threads::get_thread(db, &thread_id)?
                .ok_or_else(|| anyhow::anyhow!("thread not found after title update: {thread_id}"))
        }
    })
    .await
    {
        Ok(updated_thread) => updated_thread,
        Err(error) => {
            log::warn!("failed to update thread title: {error}");
            return None;
        }
    };

    if let Err(error) = state
        .engines
        .set_thread_name(thread, engine_thread_id, &candidate)
        .await
    {
        log::debug!("failed to sync thread name with engine: {error}");
    }

    Some(updated_thread)
}

pub(super) fn should_autotitle_thread(thread: &ThreadDto) -> bool {
    thread.message_count == 0 && !thread_manual_title_locked(thread.engine_metadata.as_ref())
}

pub(super) fn thread_manual_title_locked(metadata: Option<&Value>) -> bool {
    metadata
        .and_then(|value| value.get("manualTitle"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn normalize_thread_title(raw: &str) -> Option<String> {
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut title = compact.trim_matches(|c| c == '"' || c == '\'').to_string();
    if title.is_empty() {
        return None;
    }

    if title.chars().count() > MAX_THREAD_TITLE_CHARS {
        title = truncate_title(title, MAX_THREAD_TITLE_CHARS);
    }

    Some(title)
}

pub(super) fn truncate_title(value: String, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value;
    }

    if max_chars <= 3 {
        return value.chars().take(max_chars).collect::<String>();
    }

    let mut output = value.chars().take(max_chars - 3).collect::<String>();
    output.push_str("...");
    output
}

pub(super) fn normalize_chat_notification_preview(raw: &str) -> Option<String> {
    let compact = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = compact.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(truncate_title(
        trimmed.to_string(),
        MAX_CHAT_NOTIFICATION_PREVIEW_CHARS,
    ))
}

pub(super) fn chat_notification_preview(blocks: &[ContentBlock]) -> Option<String> {
    for block in blocks {
        match block {
            ContentBlock::Text {
                is_steer: Some(true),
                ..
            } => {}
            ContentBlock::Text { content, .. }
            | ContentBlock::Thinking { content, .. }
            | ContentBlock::Error { message: content } => {
                if let Some(preview) = normalize_chat_notification_preview(content) {
                    return Some(preview);
                }
            }
            ContentBlock::Notice { message, title, .. } => {
                if let Some(preview) = normalize_chat_notification_preview(message) {
                    return Some(preview);
                }
                if let Some(preview) = normalize_chat_notification_preview(title) {
                    return Some(preview);
                }
            }
            _ => {}
        }
    }

    None
}

pub(super) fn emit_chat_turn_finished(
    app: &tauri::AppHandle,
    thread: &ThreadDto,
    status: &MessageStatusDto,
    blocks: &[ContentBlock],
) {
    let event = ChatTurnFinishedEvent {
        thread_id: thread.id.clone(),
        workspace_id: thread.workspace_id.clone(),
        engine_id: thread.engine_id.clone(),
        thread_title: thread.title.clone(),
        status: match status {
            MessageStatusDto::Completed => "completed",
            MessageStatusDto::Interrupted => "interrupted",
            MessageStatusDto::Error => "error",
            MessageStatusDto::Streaming => "completed",
        }
        .to_string(),
        preview: chat_notification_preview(blocks),
    };
    let _ = app.emit("chat-turn-finished", event);
}

pub(super) fn build_final_thread_event(
    latest_thread: Option<ThreadDto>,
    fallback_thread: &ThreadDto,
) -> (ThreadUpdatedEvent, Option<ThreadDto>) {
    match latest_thread {
        Some(latest_thread) => (
            ThreadUpdatedEvent {
                thread_id: latest_thread.id.clone(),
                workspace_id: latest_thread.workspace_id.clone(),
                thread: Some(latest_thread.clone()),
            },
            Some(latest_thread),
        ),
        None => (
            ThreadUpdatedEvent {
                thread_id: fallback_thread.id.clone(),
                workspace_id: fallback_thread.workspace_id.clone(),
                thread: None,
            },
            None,
        ),
    }
}
