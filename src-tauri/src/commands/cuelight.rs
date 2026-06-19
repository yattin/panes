use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::db;
use crate::models::CueLightBindingDto;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// CueLight HTTP proxy
// ---------------------------------------------------------------------------

/// 通用 CueLight API 代理命令。
/// 前端通过此命令代理所有对 CueLight 服务器的 HTTP 请求，解决 CORS 限制并统一鉴权。
#[tauri::command]
pub async fn cuelight_proxy(
    method: String,
    server_url: String,
    path: String,
    auth_token: Option<String>,
    body: Option<Value>,
    query: Option<HashMap<String, String>>,
) -> Result<Value, String> {
    let client = reqwest::Client::new();

    let base_url = format!("{}{}", server_url.trim_end_matches('/'), path);

    let url = if let Some(params) = &query {
        let pairs: Vec<(&str, &str)> = params
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        reqwest::Url::parse_with_params(&base_url, &pairs)
            .map_err(|e| format!("invalid URL: {}", e))?
            .to_string()
    } else {
        base_url
    };

    let mut request = match method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        other => return Err(format!("unsupported HTTP method: {}", other)),
    };

    request = request.header("Content-Type", "application/json");

    if let Some(token) = &auth_token {
        if !token.is_empty() {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
    }

    if let Some(json_body) = &body {
        request = request.json(json_body);
    }

    let response = request
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("CueLight request failed: {}", e))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("failed to read CueLight response: {}", e))?;

    if !status.is_success() {
        return Err(format!(
            "CueLight API error ({}): {}",
            status.as_u16(),
            text.chars().take(500).collect::<String>()
        ));
    }

    if text.is_empty() {
        return Ok(Value::Null);
    }

    serde_json::from_str(&text)
        .map_err(|_| format!("invalid JSON from CueLight: {}", text.chars().take(200).collect::<String>()))
}

// ---------------------------------------------------------------------------
// CueLight project binding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CueLightBindingInput {
    pub project_id: String,
    pub project_name: String,
}

/// 将 CueLight 项目绑定到工作区
#[tauri::command]
pub async fn bind_cuelight_project(
    state: State<'_, AppState>,
    workspace_id: String,
    binding: CueLightBindingInput,
) -> Result<(), String> {
    let dto = CueLightBindingDto {
        project_id: binding.project_id,
        project_name: binding.project_name,
        bound_at: chrono::Utc::now().to_rfc3339(),
    };

    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        db::workspaces::set_cuelight_binding(&db, &workspace_id, &dto)
    })
    .await
    .map_err(|e| format!("task join error: {}", e))?
    .map_err(|e| format!("failed to save CueLight binding: {}", e))
}

/// 解除工作区的 CueLight 项目绑定
#[tauri::command]
pub async fn unbind_cuelight_project(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<(), String> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        db::workspaces::clear_cuelight_binding(&db, &workspace_id)
    })
    .await
    .map_err(|e| format!("task join error: {}", e))?
    .map_err(|e| format!("failed to clear CueLight binding: {}", e))
}

/// 获取工作区的 CueLight 绑定信息
#[tauri::command]
pub async fn get_cuelight_binding(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<Option<CueLightBindingDto>, String> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        db::workspaces::get_cuelight_binding(&db, &workspace_id)
    })
    .await
    .map_err(|e| format!("task join error: {}", e))?
    .map_err(|e| format!("failed to read CueLight binding: {}", e))
}

/// 设置全局 CueLight 认证 Token（由前端调用）
#[tauri::command]
pub async fn set_cuelight_auth_token(token: String) -> Result<(), String> {
    crate::engines::cuelight_tools::set_global_auth_token(token);
    Ok(())
}
