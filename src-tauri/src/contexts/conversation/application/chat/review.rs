use super::*;

pub(super) fn normalize_codex_review_target(
    target: &CodexReviewTargetPayload,
) -> Result<(Value, String, String), String> {
    match target {
        CodexReviewTargetPayload::UncommittedChanges => Ok((
            serde_json::json!({
                "type": "uncommittedChanges",
            }),
            "Review uncommitted changes.".to_string(),
            "Review: Uncommitted changes".to_string(),
        )),
        CodexReviewTargetPayload::BaseBranch { branch } => {
            let branch = branch.trim();
            if branch.is_empty() {
                return Err("Base branch review requires a branch name.".to_string());
            }
            Ok((
                serde_json::json!({
                    "type": "baseBranch",
                    "branch": branch,
                }),
                format!("Review changes against base branch `{branch}`."),
                truncate_title(format!("Review: {branch}"), MAX_THREAD_TITLE_CHARS),
            ))
        }
        CodexReviewTargetPayload::Commit { sha, title } => {
            let sha = sha.trim();
            if sha.is_empty() {
                return Err("Commit review requires a commit SHA.".to_string());
            }
            let short_sha = sha.chars().take(12).collect::<String>();
            let normalized_title = title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let message = match normalized_title.as_deref() {
                Some(title) => format!("Review commit `{sha}`: {title}"),
                None => format!("Review commit `{sha}`."),
            };
            let target = match normalized_title {
                Some(title) => serde_json::json!({
                    "type": "commit",
                    "sha": sha,
                    "title": title,
                }),
                None => serde_json::json!({
                    "type": "commit",
                    "sha": sha,
                }),
            };
            Ok((
                target,
                message,
                truncate_title(format!("Review: {short_sha}"), MAX_THREAD_TITLE_CHARS),
            ))
        }
        CodexReviewTargetPayload::Custom { instructions } => {
            let instructions = instructions.trim();
            if instructions.is_empty() {
                return Err("Custom review requires instructions.".to_string());
            }
            Ok((
                serde_json::json!({
                    "type": "custom",
                    "instructions": instructions,
                }),
                instructions.to_string(),
                "Review: Custom".to_string(),
            ))
        }
    }
}

pub(super) fn clone_codex_review_metadata(
    existing: Option<&Value>,
    model_id: &str,
) -> Option<Value> {
    let mut metadata = existing.cloned().unwrap_or_else(|| serde_json::json!({}));
    if !metadata.is_object() {
        metadata = serde_json::json!({});
    }

    let object = metadata.as_object_mut()?;
    object.remove("manualTitle");
    object.remove("manualTitleUpdatedAt");
    object.remove("codexPreview");
    object.remove("codexThreadStatus");
    object.remove("codexThreadActiveFlags");
    object.remove("codexSyncRequired");
    object.remove("codexSyncReason");
    object.insert(
        "lastModelId".to_string(),
        Value::String(model_id.to_string()),
    );
    Some(metadata)
}
