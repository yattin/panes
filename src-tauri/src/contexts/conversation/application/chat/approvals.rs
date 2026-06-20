use super::*;

pub(super) async fn respond_to_approval(
    state: State<'_, AppState>,
    thread_id: String,
    approval_id: String,
    response: Value,
) -> Result<(), String> {
    respond_to_approval_inner(state.inner(), thread_id, approval_id, response).await
}

pub(super) async fn respond_to_approval_inner(
    state: &AppState,
    thread_id: String,
    approval_id: String,
    response: Value,
) -> Result<(), String> {
    if !response.is_object() {
        return Err("approval response must be a JSON object".to_string());
    }

    let db = state.db.clone();
    let thread = run_db(db.clone(), {
        let thread_id = thread_id.clone();
        move |db| db::threads::get_thread(db, &thread_id)
    })
    .await?
    .ok_or_else(|| format!("thread not found: {thread_id}"))?;
    let normalized_response =
        normalize_approval_response_for_engine(thread.engine_id.as_str(), response)?;
    let approval_route =
        load_approval_response_route(db.clone(), thread.engine_id.as_str(), &approval_id).await?;

    state
        .engines
        .respond_to_approval(
            &thread,
            &approval_id,
            normalized_response.clone(),
            approval_route,
        )
        .await
        .map_err(err_to_string)?;

    let decision = approval_response_decision_for_persistence(&normalized_response);
    run_db(db, {
        let approval_id = approval_id.clone();
        let thread_id = thread_id.clone();
        let decision = decision.to_string();
        move |db| {
            db::actions::answer_approval(db, &approval_id, &decision)?;
            if let Some(message_id) = db::actions::find_approval_message_id(db, &approval_id)? {
                let _ = db::messages::mark_approval_block_answered(
                    db,
                    &message_id,
                    &approval_id,
                    &decision,
                );
            }
            db::threads::update_thread_status(db, &thread_id, ThreadStatusDto::Streaming)?;
            Ok(())
        }
    })
    .await?;

    Ok(())
}

pub(super) async fn load_approval_response_route(
    db: crate::db::Database,
    engine_id: &str,
    approval_id: &str,
) -> Result<Option<ApprovalRequestRoute>, String> {
    let engine_id = engine_id.to_string();
    let approval_id = approval_id.to_string();
    run_db(db, move |db| {
        let details = db::actions::find_approval_details(db, &approval_id)?;
        Ok(details.and_then(|details| approval_response_route_for_engine(&engine_id, &details)))
    })
    .await
}

pub(super) fn approval_response_decision_for_persistence(response: &Value) -> &'static str {
    if let Some(decision) = response.get("decision").and_then(Value::as_str) {
        return match decision {
            "deny" => "decline",
            "acceptForSession" => "accept_for_session",
            "accept" => "accept",
            "decline" => "decline",
            "cancel" => "cancel",
            "accept_for_session" => "accept_for_session",
            _ => "custom",
        };
    }

    if let Some(action) = response.get("action").and_then(Value::as_str) {
        return match action {
            "accept" => "accept",
            "decline" => "decline",
            "cancel" => "cancel",
            _ => "custom",
        };
    }

    if response.get("permissions").is_some() {
        if permission_profile_is_empty(response.get("permissions")) {
            return "decline";
        }
        if matches!(
            response.get("scope").and_then(Value::as_str),
            Some("session")
        ) {
            return "accept_for_session";
        }
        return "accept";
    }

    "custom"
}

pub(super) fn permission_profile_is_empty(value: Option<&Value>) -> bool {
    fn has_granted_permission(value: &Value) -> bool {
        match value {
            Value::Bool(value) => *value,
            Value::String(value) => !value.trim().is_empty() && value != "none",
            Value::Array(items) => items.iter().any(has_granted_permission),
            Value::Object(map) => map.values().any(has_granted_permission),
            _ => false,
        }
    }

    match value {
        Some(value) => !has_granted_permission(value),
        None => true,
    }
}
