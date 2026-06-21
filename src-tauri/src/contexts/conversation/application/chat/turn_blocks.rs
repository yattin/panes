use super::*;

pub(super) fn apply_event_to_blocks(
    blocks: &mut Vec<ContentBlock>,
    action_index: &mut HashMap<String, usize>,
    approval_index: &mut HashMap<String, usize>,
    event: &EngineEvent,
    max_output_chars: usize,
) -> EventProgress {
    let mut progress = EventProgress::default();

    match event {
        EngineEvent::TurnStarted { .. } => {
            progress.thread_status = Some(ThreadStatusDto::Streaming);
        }
        EngineEvent::TurnCompleted {
            token_usage,
            status,
        } => {
            progress.force_persist = true;
            match status {
                TurnCompletionStatus::Completed => {
                    progress.message_status = Some(MessageStatusDto::Completed);
                    progress.thread_status = Some(ThreadStatusDto::Completed);
                }
                TurnCompletionStatus::Interrupted => {
                    progress.message_status = Some(MessageStatusDto::Interrupted);
                    progress.thread_status = Some(ThreadStatusDto::Idle);
                }
                TurnCompletionStatus::Failed => {
                    progress.message_status = Some(MessageStatusDto::Error);
                    progress.thread_status = Some(ThreadStatusDto::Error);
                }
            }
            progress.token_usage = token_usage
                .as_ref()
                .map(|usage| (usage.input, usage.output));
        }
        EngineEvent::TextDelta { content } => {
            progress.blocks_changed = append_text_delta(blocks, content);
        }
        EngineEvent::ThinkingDelta { content } => {
            progress.blocks_changed = append_thinking_delta(blocks, content);
        }
        EngineEvent::ActionStarted {
            action_id,
            engine_action_id,
            action_type,
            summary,
            display_label,
            display_subtitle,
            details,
        } => {
            let block = ContentBlock::Action {
                action_id: action_id.to_string(),
                engine_action_id: engine_action_id.clone(),
                action_type: action_type.as_str().to_string(),
                summary: summary.to_string(),
                display_label: display_label.clone(),
                display_subtitle: display_subtitle.clone(),
                details: value_to_raw(details),
                output_chunks: Vec::new(),
                status: "running".to_string(),
                result: None,
            };
            progress.blocks_changed = upsert_action_block(blocks, action_index, action_id, block);
        }
        EngineEvent::ActionOutputDelta {
            action_id,
            stream,
            content,
        } => {
            if let Some(index) = action_index.get(action_id).copied() {
                if let Some(ContentBlock::Action {
                    output_chunks,
                    details,
                    ..
                }) = blocks.get_mut(index)
                {
                    let stream_name = match stream {
                        OutputStream::Stdout => "stdout",
                        OutputStream::Stderr => "stderr",
                        OutputStream::Stdin => "stdin",
                    };
                    let chunk_content = truncate_chars(content, max_output_chars);
                    if chunk_content.is_empty() {
                        return progress;
                    }

                    if let Some(previous_chunk) = output_chunks.last_mut() {
                        if previous_chunk.stream == stream_name {
                            previous_chunk.content.push_str(&chunk_content);
                        } else {
                            output_chunks.push(ActionOutputChunk {
                                stream: stream_name.to_string(),
                                content: chunk_content,
                            });
                        }
                    } else {
                        output_chunks.push(ActionOutputChunk {
                            stream: stream_name.to_string(),
                            content: chunk_content,
                        });
                    }

                    if trim_action_output_chunks(output_chunks, max_output_chars) {
                        mark_output_truncated(details);
                    }
                    progress.blocks_changed = true;
                }
            }
        }
        EngineEvent::ActionProgressUpdated { action_id, message } => {
            if let Some(index) = action_index.get(action_id).copied() {
                if let Some(ContentBlock::Action { details, .. }) = blocks.get_mut(index) {
                    progress.blocks_changed = update_action_progress(details, message);
                }
            }
        }
        EngineEvent::ActionCompleted { action_id, result } => {
            if let Some(index) = action_index.get(action_id).copied() {
                if let Some(ContentBlock::Action {
                    status,
                    result: block_result,
                    ..
                }) = blocks.get_mut(index)
                {
                    *status = if result.success { "done" } else { "error" }.to_string();
                    *block_result = Some(ActionBlockResult {
                        success: result.success,
                        output: result.output.clone(),
                        error: result.error.clone(),
                        diff: result.diff.clone(),
                        duration_ms: result.duration_ms,
                    });
                    progress.blocks_changed = true;
                }
            }
        }
        EngineEvent::DiffUpdated { diff, scope } => {
            let scope = match scope {
                crate::engines::DiffScope::Turn => "turn",
                crate::engines::DiffScope::File => "file",
                crate::engines::DiffScope::Workspace => "workspace",
            }
            .to_string();

            let latest_matching_index =
                blocks.iter().enumerate().rev().find_map(|(index, block)| {
                    if let ContentBlock::Diff {
                        scope: block_scope, ..
                    } = block
                    {
                        if block_scope == &scope {
                            return Some(index);
                        }
                    }
                    None
                });

            if let Some(latest_matching_index) = latest_matching_index {
                let mut next_blocks = Vec::with_capacity(blocks.len());
                for (index, mut block) in blocks.drain(..).enumerate() {
                    if let ContentBlock::Diff {
                        diff: block_diff,
                        scope: block_scope,
                    } = &mut block
                    {
                        if block_scope == &scope {
                            if index == latest_matching_index {
                                if block_diff != diff {
                                    *block_diff = diff.to_string();
                                }
                                next_blocks.push(block);
                            }
                            continue;
                        }
                    }
                    next_blocks.push(block);
                }
                *blocks = next_blocks;
            } else {
                blocks.push(ContentBlock::Diff {
                    diff: diff.to_string(),
                    scope,
                });
            }
            progress.blocks_changed = true;
        }
        EngineEvent::ModelRerouted {
            from_model,
            to_model,
            reason,
        } => {
            let block = ContentBlock::Notice {
                kind: "model_rerouted".to_string(),
                level: "info".to_string(),
                title: "Model rerouted".to_string(),
                message: format_model_reroute_notice(from_model, to_model, reason),
            };
            progress.blocks_changed = upsert_notice_block(
                blocks,
                action_index,
                approval_index,
                "model_rerouted",
                block,
            );
            progress.turn_model_id = Some(to_model.to_string());
            progress.force_persist = true;
        }
        EngineEvent::Notice {
            kind,
            level,
            title,
            message,
        } => {
            let block = ContentBlock::Notice {
                kind: kind.to_string(),
                level: level.to_string(),
                title: title.to_string(),
                message: message.to_string(),
            };
            progress.blocks_changed =
                upsert_notice_block(blocks, action_index, approval_index, kind, block);
            progress.force_persist = true;
        }
        EngineEvent::ApprovalRequested {
            approval_id,
            action_type,
            summary,
            details,
        } => {
            let block = ContentBlock::Approval {
                approval_id: approval_id.to_string(),
                action_type: action_type.as_str().to_string(),
                summary: summary.to_string(),
                details: value_to_raw(details),
                status: "pending".to_string(),
                decision: None,
            };
            progress.blocks_changed =
                upsert_approval_block(blocks, approval_index, approval_id, block);
            progress.thread_status = Some(ThreadStatusDto::AwaitingApproval);
            progress.force_persist = true;
        }
        EngineEvent::Error {
            message,
            recoverable,
        } => {
            blocks.push(ContentBlock::Error {
                message: message.to_string(),
            });
            progress.blocks_changed = true;
            if !recoverable {
                progress.message_status = Some(MessageStatusDto::Error);
                progress.thread_status = Some(ThreadStatusDto::Error);
                progress.force_persist = true;
            }
        }
        EngineEvent::UsageLimitsUpdated { .. } | EngineEvent::TranscriptEntry { .. } => {}
    }

    progress
}

pub(super) fn append_text_delta(blocks: &mut Vec<ContentBlock>, content: &str) -> bool {
    if content.is_empty() {
        return false;
    }

    if let Some(ContentBlock::Text {
        content: current, ..
    }) = blocks.last_mut()
    {
        current.push_str(content);
        return true;
    }

    blocks.push(ContentBlock::Text {
        content: content.to_string(),
        plan_mode: None,
        is_steer: None,
    });
    true
}

pub(super) fn append_thinking_delta(blocks: &mut Vec<ContentBlock>, content: &str) -> bool {
    if content.is_empty() {
        return false;
    }

    if let Some(ContentBlock::Thinking {
        content: current, ..
    }) = blocks.last_mut()
    {
        current.push_str(content);
        return true;
    }

    blocks.push(ContentBlock::Thinking {
        content: content.to_string(),
        started_at: None,
        duration_ms: None,
    });
    true
}

pub(super) fn update_action_progress(details: &mut Box<RawValue>, message: &str) -> bool {
    let mut value: Value = serde_json::from_str(details.get())
        .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));

    let current_message = value
        .get("progressMessage")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let current_kind = value
        .get("progressKind")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    if current_message.as_deref() == Some(message) && current_kind.as_deref() == Some("mcp") {
        return false;
    }

    if !value.is_object() {
        value = Value::Object(serde_json::Map::new());
    }

    if let Some(details_object) = value.as_object_mut() {
        details_object.insert("progressKind".to_string(), Value::String("mcp".to_string()));
        details_object.insert(
            "progressMessage".to_string(),
            Value::String(message.to_string()),
        );
        *details = value_to_raw(&value);
        return true;
    }

    false
}

pub(super) fn upsert_action_block(
    blocks: &mut Vec<ContentBlock>,
    action_index: &mut HashMap<String, usize>,
    action_id: &str,
    block: ContentBlock,
) -> bool {
    if let Some(index) = action_index.get(action_id).copied() {
        if let Some(existing) = blocks.get_mut(index) {
            *existing = block;
            return true;
        }
    }

    let index = blocks.len();
    blocks.push(block);
    action_index.insert(action_id.to_string(), index);
    true
}

pub(super) fn upsert_approval_block(
    blocks: &mut Vec<ContentBlock>,
    approval_index: &mut HashMap<String, usize>,
    approval_id: &str,
    block: ContentBlock,
) -> bool {
    if let Some(index) = approval_index.get(approval_id).copied() {
        if let Some(existing) = blocks.get_mut(index) {
            *existing = block;
            return true;
        }
    }

    let index = blocks.len();
    blocks.push(block);
    approval_index.insert(approval_id.to_string(), index);
    true
}

pub(super) fn upsert_notice_block(
    blocks: &mut Vec<ContentBlock>,
    action_index: &mut HashMap<String, usize>,
    approval_index: &mut HashMap<String, usize>,
    kind: &str,
    block: ContentBlock,
) -> bool {
    if let Some(index) = blocks.iter().position(|existing| {
        matches!(
            existing,
            ContentBlock::Notice {
                kind: existing_kind,
                ..
            } if existing_kind == kind
        )
    }) {
        if let Some(existing) = blocks.get_mut(index) {
            *existing = block;
            return true;
        }
    }

    blocks.insert(0, block);
    rebuild_block_indexes(blocks, action_index, approval_index);
    true
}

pub(super) fn rebuild_block_indexes(
    blocks: &[ContentBlock],
    action_index: &mut HashMap<String, usize>,
    approval_index: &mut HashMap<String, usize>,
) {
    action_index.clear();
    approval_index.clear();

    for (index, block) in blocks.iter().enumerate() {
        match block {
            ContentBlock::Action { action_id, .. } => {
                action_index.insert(action_id.clone(), index);
            }
            ContentBlock::Approval { approval_id, .. } => {
                approval_index.insert(approval_id.clone(), index);
            }
            _ => {}
        }
    }
}

pub(super) fn format_model_reroute_notice(
    from_model: &str,
    to_model: &str,
    reason: &str,
) -> String {
    format!("Switched from {from_model} to {to_model} ({reason}).")
}

pub(super) fn trim_action_output_chunks(
    output_chunks: &mut Vec<ActionOutputChunk>,
    max_output_chars: usize,
) -> bool {
    let mut truncated = false;

    if output_chunks.len() > ACTION_OUTPUT_MAX_CHUNKS {
        let overflow = output_chunks.len() - ACTION_OUTPUT_MAX_CHUNKS;
        output_chunks.drain(0..overflow);
        truncated = true;
    }

    let max_chars = max_output_chars.max(1);
    let total_chars: usize = output_chunks.iter().map(|chunk| chunk.content.len()).sum();
    if total_chars <= max_chars {
        return truncated;
    }

    let target_chars = max_chars.saturating_mul(2) / 3;
    let chars_to_trim = total_chars.saturating_sub(target_chars.max(1));
    if chars_to_trim == 0 {
        return truncated;
    }

    let mut remaining_to_trim = chars_to_trim;
    let mut remove_count = 0usize;
    for chunk in output_chunks.iter_mut() {
        if remaining_to_trim == 0 {
            break;
        }
        let chunk_len = chunk.content.len();
        if chunk_len <= remaining_to_trim {
            remaining_to_trim -= chunk_len;
            remove_count += 1;
            continue;
        }

        chunk.content = trim_string_start_bytes(&chunk.content, remaining_to_trim);
        remaining_to_trim = 0;
        truncated = true;
    }

    if remove_count > 0 {
        output_chunks.drain(0..remove_count);
        truncated = true;
    }

    truncated
}

pub(super) fn engine_event_for_debug_log(event: &EngineEvent) -> EngineEvent {
    match event {
        EngineEvent::ActionOutputDelta {
            action_id,
            stream,
            content,
        } => EngineEvent::ActionOutputDelta {
            action_id: action_id.clone(),
            stream: stream.clone(),
            content: truncate_chars_within_limit(content, ENGINE_EVENT_LOG_ACTION_OUTPUT_MAX_CHARS),
        },
        _ => event.clone(),
    }
}

pub(super) fn trim_string_start_bytes(value: &str, bytes_to_trim: usize) -> String {
    if bytes_to_trim == 0 {
        return value.to_string();
    }
    if bytes_to_trim >= value.len() {
        return String::new();
    }

    let start = value
        .char_indices()
        .map(|(index, _)| index)
        .find(|index| *index >= bytes_to_trim)
        .unwrap_or(value.len());
    value[start..].to_string()
}

pub(super) fn mark_output_truncated(details: &mut Box<RawValue>) {
    let mut value: Value = serde_json::from_str(details.get())
        .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));

    if !value.is_object() {
        value = Value::Object(serde_json::Map::new());
    }

    if let Some(details_object) = value.as_object_mut() {
        details_object.insert("outputTruncated".to_string(), Value::Bool(true));
        *details = value_to_raw(&value);
    }
}

pub(super) fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut output = value.chars().take(max_chars).collect::<String>();
    output.push_str(TRUNCATED_SUFFIX);
    output
}

pub(super) fn truncate_chars_within_limit(value: &str, max_chars: usize) -> String {
    let max_chars = max_chars.max(1);
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    if max_chars <= TRUNCATED_SUFFIX.len() {
        return value.chars().take(max_chars).collect();
    }

    let mut output = value
        .chars()
        .take(max_chars - TRUNCATED_SUFFIX.len())
        .collect::<String>();
    output.push_str(TRUNCATED_SUFFIX);
    output
}

pub(super) fn truncate_action_result_output(
    result: &mut crate::engines::events::ActionResult,
    max_chars: usize,
) {
    let Some(output) = result.output.as_ref() else {
        return;
    };

    let truncated = truncate_chars(output, max_chars.max(1));
    if truncated != *output {
        result.output = Some(truncated);
    }
}
