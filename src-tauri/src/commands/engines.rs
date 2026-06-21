use std::time::Instant;

use anyhow::Context;
use tauri::State;
use tokio::process::Command;

#[cfg(not(target_os = "windows"))]
use crate::runtime_env;
use crate::{
    models::{
        CodexAppDto, CodexSkillDto, EngineCheckResultDto, EngineHealthDto, EngineInfoDto,
        OpenCodeRuntimeCatalogDto,
    },
    process_utils,
    state::AppState,
};

#[tauri::command]
pub async fn list_engines(state: State<'_, AppState>) -> Result<Vec<EngineInfoDto>, String> {
    state.engines.list_engines().await.map_err(err_to_string)
}

#[tauri::command]
pub async fn engine_health(
    state: State<'_, AppState>,
    engine_id: String,
) -> Result<EngineHealthDto, String> {
    state
        .engines
        .health(&engine_id)
        .await
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn prewarm_engine(state: State<'_, AppState>, engine_id: String) -> Result<(), String> {
    state
        .engines
        .prewarm(&engine_id)
        .await
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn list_codex_skills(
    state: State<'_, AppState>,
    cwd: String,
) -> Result<Vec<CodexSkillDto>, String> {
    state
        .engines
        .list_codex_skills(cwd.trim())
        .await
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn list_native_skills(
    state: State<'_, AppState>,
    cwd: String,
) -> Result<Vec<CodexSkillDto>, String> {
    state
        .engines
        .list_native_skills(cwd.trim())
        .await
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn list_codex_apps(state: State<'_, AppState>) -> Result<Vec<CodexAppDto>, String> {
    state.engines.list_codex_apps().await.map_err(err_to_string)
}

#[tauri::command]
pub async fn get_opencode_runtime_catalog(
    state: State<'_, AppState>,
    cwd: String,
) -> Result<OpenCodeRuntimeCatalogDto, String> {
    let cwd = cwd.trim();
    if cwd.is_empty() {
        return Err("cwd is required".to_string());
    }
    state
        .engines
        .opencode_runtime_catalog(cwd)
        .await
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn run_engine_check(
    state: State<'_, AppState>,
    engine_id: String,
    command: String,
) -> Result<EngineCheckResultDto, String> {
    let health = state
        .engines
        .health(&engine_id)
        .await
        .map_err(err_to_string)?;
    let is_allowed = health
        .checks
        .iter()
        .chain(health.fixes.iter())
        .any(|value| value == &command);

    if !is_allowed {
        return Err("command is not allowed for this engine check".to_string());
    }

    execute_engine_check_command(&command)
        .await
        .map_err(err_to_string)
}

async fn execute_engine_check_command(command: &str) -> anyhow::Result<EngineCheckResultDto> {
    let started = Instant::now();

    let output = build_shell_command(command)
        .output()
        .await
        .with_context(|| format!("failed to execute check command: `{command}`"))?;

    let duration_ms = started.elapsed().as_millis();

    Ok(EngineCheckResultDto {
        command: command.to_string(),
        success: output.status.success(),
        exit_code: output.status.code(),
        stdout: truncate_output(&String::from_utf8_lossy(&output.stdout), 12_000),
        stderr: truncate_output(&String::from_utf8_lossy(&output.stderr), 12_000),
        duration_ms,
    })
}

#[cfg(target_os = "windows")]
fn build_shell_command(command: &str) -> Command {
    let mut cmd = Command::new("cmd");
    process_utils::configure_tokio_command(&mut cmd);
    cmd.arg("/C").arg(command);
    cmd
}

#[cfg(not(target_os = "windows"))]
fn build_shell_command(command: &str) -> Command {
    let spec = runtime_env::command_shell_for_string(command);
    let mut cmd = Command::new(&spec.program);
    process_utils::configure_tokio_command(&mut cmd);
    cmd.args(&spec.args);
    if let Some(augmented_path) = runtime_env::augmented_path_with_prepend(
        spec.program
            .parent()
            .into_iter()
            .map(|value| value.to_path_buf()),
    ) {
        cmd.env("PATH", augmented_path);
    }
    cmd
}

fn truncate_output(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_string();
    }

    let mut out = chars.into_iter().take(max_chars).collect::<String>();
    out.push_str("\n...[truncated]");
    out
}

fn err_to_string(error: impl std::fmt::Display) -> String {
    format!("{error:#}")
}

/// Deprecated: claude-code-native manual compaction was removed with the
/// claude_code_rs runtime. The command is kept for old frontend compatibility.
#[tauri::command]
pub async fn compact_native_thread(
    state: State<'_, AppState>,
    engine_thread_id: String,
) -> Result<(usize, usize), String> {
    state
        .engines
        .compact_native_thread(&engine_thread_id)
        .await
        .map_err(err_to_string)
}

/// Deprecated: old claude-code-native in-memory history no longer exists.
#[tauri::command]
pub async fn get_native_history_tokens(
    state: State<'_, AppState>,
    engine_thread_id: String,
) -> Result<usize, String> {
    Ok(state
        .engines
        .get_native_history_tokens(&engine_thread_id)
        .await)
}

/// 获取上下文最大限制（token 数）
#[tauri::command]
pub fn get_context_max_tokens() -> usize {
    crate::engines::EngineManager::get_context_max_tokens()
}

// ---------------------------------------------------------------------------
// Provider configuration
// ---------------------------------------------------------------------------

use crate::provider_config::{merge_api_key, ProviderConfigEntry, ProviderSettings};

/// Load all provider settings from disk.  API keys are masked before they
/// cross the IPC boundary so secrets never reach the frontend in plaintext.
#[tauri::command]
pub async fn get_provider_settings() -> Result<ProviderSettings, String> {
    Ok(ProviderSettings::load().masked())
}

/// Save configuration for a single provider.
///
/// `api_key` semantics:
/// - `None` or a masked placeholder (`••••…`) → keep the stored value as-is
/// - empty string → clear the stored key
/// - any other value → overwrite
///
/// The whole read-modify-write is guarded by `provider_settings_lock` so
/// concurrent saves cannot clobber each other.
#[tauri::command]
pub async fn set_provider_config(
    state: State<'_, AppState>,
    provider_id: String,
    enabled: bool,
    api_key: Option<String>,
    api_base: Option<String>,
    models: Option<std::collections::HashMap<String, bool>>,
) -> Result<(), String> {
    let _guard = state.provider_settings_lock.lock().await;
    let mut settings = ProviderSettings::load();
    let entry = settings.providers.entry(provider_id).or_insert_with(|| {
        ProviderConfigEntry {
            enabled: true,
            api_key: None,
            api_base: None,
            models: std::collections::HashMap::new(),
        }
    });
    entry.enabled = enabled;
    entry.api_key = merge_api_key(entry.api_key.take(), api_key);
    entry.api_base = api_base;
    if let Some(m) = models {
        entry.models = m;
    }
    settings.save().map_err(|e| e.to_string())
}
