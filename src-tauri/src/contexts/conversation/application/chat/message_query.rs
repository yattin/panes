use super::*;

pub(super) async fn get_thread_messages(
    state: State<'_, AppState>,
    thread_id: String,
) -> Result<Vec<MessageDto>, String> {
    run_db(state.db.clone(), move |db| {
        db::messages::get_thread_messages(db, &thread_id)
    })
    .await
}

pub(super) async fn get_thread_messages_window(
    state: State<'_, AppState>,
    thread_id: String,
    cursor: Option<MessageWindowCursorDto>,
    limit: Option<usize>,
) -> Result<MessageWindowDto, String> {
    let requested_limit = limit.unwrap_or(MESSAGE_WINDOW_DEFAULT_LIMIT);
    let clamped_limit = requested_limit.clamp(1, MESSAGE_WINDOW_MAX_LIMIT);

    run_db(state.db.clone(), move |db| {
        db::messages::get_thread_messages_window(db, &thread_id, cursor.as_ref(), clamped_limit)
    })
    .await
}

pub(super) async fn get_message_blocks(
    state: State<'_, AppState>,
    message_id: String,
) -> Result<Option<Value>, String> {
    run_db(state.db.clone(), move |db| {
        db::messages::get_message_blocks(db, &message_id)
    })
    .await
}

pub(super) async fn get_action_output(
    state: State<'_, AppState>,
    message_id: String,
    action_id: String,
) -> Result<ActionOutputDto, String> {
    run_db(state.db.clone(), move |db| {
        db::messages::get_action_output(db, &message_id, &action_id)
    })
    .await
}

pub(super) async fn search_messages(
    state: State<'_, AppState>,
    workspace_id: String,
    query: String,
) -> Result<Vec<SearchResultDto>, String> {
    run_db(state.db.clone(), move |db| {
        db::messages::search_messages(db, &workspace_id, &query)
    })
    .await
}
