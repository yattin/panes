use super::*;

pub(super) fn workspace_write_opt_in_enabled(metadata: Option<&Value>) -> bool {
    metadata
        .and_then(|value| value.get("workspaceWriteOptIn"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn aggregate_workspace_trust_level(repos: &[RepoDto]) -> TrustLevelDto {
    if repos
        .iter()
        .any(|repo| matches!(repo.trust_level, TrustLevelDto::Restricted))
    {
        return TrustLevelDto::Restricted;
    }

    if !repos.is_empty()
        && repos
            .iter()
            .all(|repo| matches!(repo.trust_level, TrustLevelDto::Trusted))
    {
        return TrustLevelDto::Trusted;
    }

    TrustLevelDto::Standard
}

pub(super) fn approval_policy_for_engine_and_trust_level(
    engine_id: &str,
    trust_level: &TrustLevelDto,
) -> &'static str {
    match engine_id {
        "claude" => match trust_level {
            TrustLevelDto::Trusted => "trusted",
            TrustLevelDto::Standard => "standard",
            TrustLevelDto::Restricted => "restricted",
        },
        "opencode" => match trust_level {
            TrustLevelDto::Trusted | TrustLevelDto::Standard => "ask",
            TrustLevelDto::Restricted => "deny",
        },
        _ => match trust_level {
            TrustLevelDto::Trusted => "on-request",
            TrustLevelDto::Standard => "on-request",
            TrustLevelDto::Restricted => "untrusted",
        },
    }
}

pub(super) fn allow_network_for_trust_level(trust_level: &TrustLevelDto) -> bool {
    matches!(trust_level, TrustLevelDto::Trusted)
}

pub(super) fn thread_approval_policy_override_value(
    engine_id: &str,
    metadata: Option<&Value>,
) -> Result<Option<Value>, String> {
    match engine_id {
        "claude" => Ok(metadata
            .and_then(|value| value.get("claudePermissionMode"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| matches!(*value, "trusted" | "standard" | "restricted"))
            .map(|value| Value::String(value.to_string()))),
        "opencode" => Ok(metadata
            .and_then(|value| value.get("opencodePermissionMode"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| matches!(*value, "ask" | "allow" | "deny"))
            .map(|value| Value::String(value.to_string()))),
        _ => metadata
            .and_then(|value| value.get("sandboxApprovalPolicy"))
            .map(normalize_codex_approval_policy_value)
            .transpose(),
    }
}

pub(super) fn thread_allow_network_override(metadata: Option<&Value>) -> Option<bool> {
    metadata
        .and_then(|value| value.get("sandboxAllowNetwork"))
        .and_then(Value::as_bool)
}

pub(super) fn thread_sandbox_mode(metadata: Option<&Value>) -> Result<Option<String>, String> {
    let value = metadata
        .and_then(|value| value.get("sandboxMode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let Some(value) = value else {
        return Ok(None);
    };

    let normalized = match value.to_lowercase().as_str() {
        "readonly" | "read-only" | "read_only" => "read-only",
        "workspacewrite" | "workspace-write" | "workspace_write" => "workspace-write",
        "dangerfullaccess" | "danger-full-access" | "danger_full_access" => {
            "danger-full-access"
        }
        _ => {
            return Err(format!(
                "invalid sandbox mode `{value}` on thread metadata. expected one of: read-only, workspace-write, danger-full-access"
            ))
        }
    };

    Ok(Some(normalized.to_string()))
}

pub(super) fn workspace_writable_roots_from_metadata(
    metadata: Option<&Value>,
) -> Result<Option<Vec<String>>, String> {
    let Some(raw_roots) = metadata.and_then(|value| value.get("workspaceWritableRoots")) else {
        return Ok(None);
    };

    let roots = raw_roots.as_array().ok_or_else(|| {
        "invalid `workspaceWritableRoots` on thread metadata. expected an array of paths"
            .to_string()
    })?;

    let mut normalized = Vec::with_capacity(roots.len());
    for root in roots {
        let root = root.as_str().map(str::trim).filter(|value| !value.is_empty()).ok_or_else(
            || {
                "invalid `workspaceWritableRoots` on thread metadata. expected non-empty string paths"
                    .to_string()
            },
        )?;
        normalized.push(root.to_string());
    }

    Ok(Some(normalized))
}

pub(super) struct WorkspaceWritableRootsResolution {
    pub(super) roots: Vec<String>,
    pub(super) requires_confirmation: bool,
}

pub(super) fn resolve_workspace_writable_roots<'a>(
    repo_paths: impl IntoIterator<Item = &'a str>,
    workspace_root: &str,
    metadata: Option<&Value>,
) -> Result<WorkspaceWritableRootsResolution, String> {
    let available_roots: Vec<String> = repo_paths.into_iter().map(ToOwned::to_owned).collect();
    let confirmed_roots = workspace_writable_roots_from_metadata(metadata)?;

    if let Some(confirmed_roots) = confirmed_roots {
        if confirmed_roots.is_empty() {
            return Ok(WorkspaceWritableRootsResolution {
                roots: vec![workspace_root.to_string()],
                requires_confirmation: false,
            });
        }

        let available_set: std::collections::HashSet<&str> =
            available_roots.iter().map(String::as_str).collect();
        let mut filtered_roots = Vec::with_capacity(confirmed_roots.len());
        for root in confirmed_roots {
            if available_set.contains(root.as_str()) {
                filtered_roots.push(root);
            }
        }
        if !filtered_roots.is_empty() {
            return Ok(WorkspaceWritableRootsResolution {
                roots: filtered_roots,
                requires_confirmation: false,
            });
        }

        return Ok(match available_roots.len() {
            0 => WorkspaceWritableRootsResolution {
                roots: vec![workspace_root.to_string()],
                requires_confirmation: false,
            },
            1 => WorkspaceWritableRootsResolution {
                roots: available_roots,
                requires_confirmation: false,
            },
            _ => WorkspaceWritableRootsResolution {
                roots: available_roots,
                requires_confirmation: true,
            },
        });
    }

    if available_roots.is_empty() {
        Ok(WorkspaceWritableRootsResolution {
            roots: vec![workspace_root.to_string()],
            requires_confirmation: false,
        })
    } else {
        Ok(WorkspaceWritableRootsResolution {
            roots: available_roots,
            requires_confirmation: false,
        })
    }
}

pub(super) fn sandbox_mode_requires_workspace_opt_in(mode: &str) -> bool {
    !mode.eq_ignore_ascii_case("read-only")
}

pub(super) fn workspace_write_confirmation_required(
    resolution: Option<&WorkspaceWritableRootsResolution>,
    sandbox_mode: &str,
    opt_in_enabled: bool,
) -> bool {
    let Some(resolution) = resolution else {
        return false;
    };

    sandbox_mode_requires_workspace_opt_in(sandbox_mode)
        && (resolution.requires_confirmation || (resolution.roots.len() > 1 && !opt_in_enabled))
}

pub(super) fn unsupported_thread_sandbox_override_for_external_sandbox(
    sandbox_mode: Option<&str>,
    external_sandbox_active: bool,
) -> bool {
    external_sandbox_active && matches!(sandbox_mode, Some("read-only" | "workspace-write"))
}

pub(super) fn thread_reasoning_effort(metadata: Option<&Value>) -> Option<String> {
    metadata
        .and_then(|value| value.get("reasoningEffort"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

pub(super) fn thread_last_model_id(metadata: Option<&Value>) -> Option<String> {
    metadata
        .and_then(|value| value.get("lastModelId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn thread_service_tier(metadata: Option<&Value>) -> Option<String> {
    metadata
        .and_then(|value| value.get("serviceTier"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| matches!(*value, "fast" | "flex"))
        .map(ToOwned::to_owned)
}

pub(super) fn thread_personality(metadata: Option<&Value>) -> Option<String> {
    metadata
        .and_then(|value| value.get("personality"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| matches!(*value, "none" | "friendly" | "pragmatic"))
        .map(ToOwned::to_owned)
}

pub(super) fn thread_output_schema(metadata: Option<&Value>) -> Option<Value> {
    metadata
        .and_then(|value| value.get("outputSchema"))
        .cloned()
}

pub(super) fn thread_permission_profile(metadata: Option<&Value>) -> Option<Value> {
    metadata
        .and_then(|value| value.get("permissionProfile"))
        .cloned()
}

pub(super) fn thread_approvals_reviewer(metadata: Option<&Value>) -> Option<String> {
    metadata
        .and_then(|value| value.get("approvalsReviewer"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn thread_opencode_agent(metadata: Option<&Value>) -> Option<String> {
    metadata
        .and_then(|value| value.get("opencodeAgent"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn normalize_reasoning_effort_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase)
}

pub(super) fn resolve_reasoning_effort_for_model(
    model: &EngineModelDto,
    requested_effort: Option<&str>,
) -> Option<String> {
    let normalized_requested = normalize_reasoning_effort_value(requested_effort);
    if let Some(requested) = normalized_requested.as_ref() {
        if model
            .supported_reasoning_efforts
            .iter()
            .any(|option| option.reasoning_effort == *requested)
        {
            return Some(requested.clone());
        }
    }

    let normalized_default =
        normalize_reasoning_effort_value(Some(model.default_reasoning_effort.as_str()));
    if let Some(default_effort) = normalized_default.as_ref() {
        if model
            .supported_reasoning_efforts
            .iter()
            .any(|option| option.reasoning_effort == *default_effort)
        {
            return Some(default_effort.clone());
        }
    }

    model
        .supported_reasoning_efforts
        .iter()
        .map(|option| option.reasoning_effort.trim())
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or(normalized_default)
        .or(normalized_requested)
}

pub(super) fn resolve_reasoning_effort_from_catalog(
    engines: &[EngineInfoDto],
    engine_id: &str,
    model_id: &str,
    requested_effort: Option<&str>,
) -> Option<String> {
    let normalized_requested = normalize_reasoning_effort_value(requested_effort);
    let Some(model) = engines
        .iter()
        .find(|engine| engine.id == engine_id)
        .and_then(|engine| engine.models.iter().find(|model| model.id == model_id))
    else {
        return normalized_requested;
    };

    resolve_reasoning_effort_for_model(model, normalized_requested.as_deref())
}

pub(super) fn normalize_codex_approval_policy_value(value: &Value) -> Result<Value, String> {
    match value {
        Value::String(raw) => {
            let normalized = raw.trim().to_lowercase();
            let normalized = normalized.as_str();
            if matches!(
                normalized,
                "untrusted" | "on-failure" | "on-request" | "never"
            ) {
                Ok(Value::String(normalized.to_string()))
            } else {
                Err(format!(
                    "invalid approval policy `{normalized}`. expected one of: untrusted, on-failure, on-request, never"
                ))
            }
        }
        Value::Object(object) => {
            let reject = object
                .get("reject")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    "invalid structured approval policy. expected a `reject` object".to_string()
                })?;

            for required_key in ["mcp_elicitations", "rules", "sandbox_approval"] {
                if !reject.get(required_key).and_then(Value::as_bool).is_some() {
                    return Err(format!(
                        "invalid structured approval policy. missing boolean reject.{required_key}"
                    ));
                }
            }

            if reject.contains_key("request_permissions")
                && reject
                    .get("request_permissions")
                    .and_then(Value::as_bool)
                    .is_none()
            {
                return Err(
                    "invalid structured approval policy. reject.request_permissions must be a boolean"
                        .to_string(),
                );
            }

            Ok(Value::Object(object.clone()))
        }
        _ => Err(
            "invalid approval policy. expected a string mode or structured reject object"
                .to_string(),
        ),
    }
}

pub(super) fn resolve_turn_model_id(
    thread: &ThreadDto,
    requested_model_id: Option<&str>,
    engines: Option<&[EngineInfoDto]>,
) -> Result<String, String> {
    let Some(requested_model_id) = requested_model_id else {
        return Ok(thread.model_id.clone());
    };

    if requested_model_id == thread.model_id {
        return Ok(thread.model_id.clone());
    }

    if thread_last_model_id(thread.engine_metadata.as_ref()).as_deref() == Some(requested_model_id)
    {
        return Ok(requested_model_id.to_string());
    }

    if let Some(engines) = engines {
        if let Some(engine) = engines.iter().find(|engine| engine.id == thread.engine_id) {
            if engine
                .models
                .iter()
                .any(|model| model.id == requested_model_id)
            {
                return Ok(requested_model_id.to_string());
            }

            let available = engine
                .models
                .iter()
                .map(|model| model.id.clone())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "model `{requested_model_id}` is not supported by engine `{}`. available models: {available}",
                thread.engine_id
            ));
        }
    }

    Ok(requested_model_id.to_string())
}

pub(super) async fn model_supports_personality(
    state: &AppState,
    engine_id: &str,
    model_id: &str,
) -> bool {
    let Ok(engines) = state.engines.list_engines().await else {
        return false;
    };

    engines
        .iter()
        .find(|engine| engine.id == engine_id)
        .and_then(|engine| engine.models.iter().find(|model| model.id == model_id))
        .map(|model| model.supports_personality)
        .unwrap_or(false)
}
