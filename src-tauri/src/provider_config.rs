// provider_config.rs — Persistent provider configuration store.
//
// Stores API keys, base URLs, and enabled/disabled state for each
// LLM provider.  Configuration is persisted to a JSON file in the
// app data directory and takes precedence over environment variables.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Prefix marker used by masked keys returned to the frontend. A saved key is
/// rendered as `MASK_PREFIX` + the last 4 characters. The `set_provider_config`
/// path treats an incoming value starting with this prefix as "unchanged".
const MASK_PREFIX: &str = "••••";

// ---------------------------------------------------------------------------
// DTO types (also used in Tauri IPC)
// ---------------------------------------------------------------------------

/// Top-level settings object exchanged with the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderSettings {
    /// provider_id → entry
    pub providers: HashMap<String, ProviderConfigEntry>,
}

/// One provider's stored configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfigEntry {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
    /// Per-model enabled/disabled overrides.  model_id → enabled.
    /// When empty (or missing) all models are enabled by default.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub models: HashMap<String, bool>,
}

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

impl ProviderSettings {
    /// Location of the JSON config file on disk.
    fn config_path() -> PathBuf {
        crate::runtime_env::app_data_dir().join("provider-config.json")
    }

    /// Load settings from disk.  Returns `Default` when the file is missing
    /// or cannot be parsed (non-fatal).
    pub fn load() -> Self {
        let path = Self::config_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist settings to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        // The file holds plaintext API keys — restrict to owner read/write.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Resolution helpers — config takes precedence over env vars
    // -----------------------------------------------------------------------

    /// Resolve the API key for a provider.
    ///
    /// Priority: stored config → environment variable → `None`.
    pub fn resolve_api_key(&self, provider_id: &str, env_var: &str) -> Option<String> {
        self.providers
            .get(provider_id)
            .and_then(|e| e.api_key.clone())
            .filter(|k| !k.trim().is_empty())
            .or_else(|| {
                std::env::var(env_var)
                    .ok()
                    .filter(|v| !v.trim().is_empty())
            })
    }

    /// Resolve the base URL for a provider.
    ///
    /// Priority: stored config → env vars (checked in order) → `None`.
    pub fn resolve_api_base(&self, provider_id: &str, env_vars: &[&str]) -> Option<String> {
        self.providers
            .get(provider_id)
            .and_then(|e| e.api_base.clone())
            .filter(|b| !b.trim().is_empty())
            .or_else(|| {
                env_vars.iter().find_map(|var| {
                    std::env::var(var)
                        .ok()
                        .filter(|v| !v.trim().is_empty())
                })
            })
    }

    /// Whether a provider is enabled.  Defaults to `true` when no entry exists.
    pub fn is_enabled(&self, provider_id: &str) -> bool {
        self.providers
            .get(provider_id)
            .map(|e| e.enabled)
            .unwrap_or(true)
    }

    /// Whether a specific model within a provider is enabled.
    /// Defaults to `true` when no override exists.
    pub fn is_model_enabled(&self, provider_id: &str, model_id: &str) -> bool {
        self.providers
            .get(provider_id)
            .and_then(|e| e.models.get(model_id))
            .copied()
            .unwrap_or(true)
    }

    /// Return a copy with every stored API key replaced by a mask.  Used for
    /// the read path to the frontend so secrets never leave the backend in
    /// plaintext.  The last 4 characters are preserved so the user can tell
    /// *which* key is stored; the rest is redacted.
    pub fn masked(&self) -> ProviderSettings {
        ProviderSettings {
            providers: self
                .providers
                .iter()
                .map(|(id, entry)| {
                    let mut entry = entry.clone();
                    entry.api_key = entry.api_key.as_ref().map(|k| mask_api_key(k));
                    (id.clone(), entry)
                })
                .collect(),
        }
    }
}

/// Mask an API key for display: `••••` + last 4 chars.  Short keys are fully
/// masked except for length 0 (which returns `None` upstream).
fn mask_api_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.len() <= 4 {
        format!("{MASK_PREFIX}")
    } else {
        format!("{MASK_PREFIX}{}", &trimmed[trimmed.len() - 4..])
    }
}

/// Resolve an incoming API-key value against a stored one.
///
/// - `None`               → keep stored value (frontend sent nothing / mask)
/// - `Some(s)` if masked  → keep stored value (user did not edit the field)
/// - `Some("")`           → clear stored value
/// - `Some(s)` otherwise  → overwrite with `s`
pub fn merge_api_key(stored: Option<String>, incoming: Option<String>) -> Option<String> {
    match incoming {
        None => stored,
        Some(s) if s.starts_with(MASK_PREFIX) => stored,
        Some(s) if s.trim().is_empty() => None,
        Some(s) => Some(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_empty_and_enabled() {
        let settings = ProviderSettings::default();
        assert!(settings.providers.is_empty());
        assert!(settings.is_enabled("anthropic"));
        assert!(settings.is_enabled("openai"));
    }

    #[test]
    fn resolve_api_key_prefers_stored_value() {
        let mut settings = ProviderSettings::default();
        settings.providers.insert(
            "anthropic".to_string(),
            ProviderConfigEntry {
                enabled: true,
                api_key: Some("stored-key".to_string()),
                api_base: None,
                models: HashMap::new(),
            },
        );
        assert_eq!(
            settings.resolve_api_key("anthropic", "ANTHROPIC_API_KEY"),
            Some("stored-key".to_string())
        );
    }

    #[test]
    fn disabled_provider_returns_not_enabled() {
        let mut settings = ProviderSettings::default();
        settings.providers.insert(
            "openai".to_string(),
            ProviderConfigEntry {
                enabled: false,
                api_key: Some("key".to_string()),
                api_base: None,
                models: HashMap::new(),
            },
        );
        assert!(!settings.is_enabled("openai"));
    }

    #[test]
    fn masked_view_redacts_keys_but_keeps_tail() {
        let mut settings = ProviderSettings::default();
        settings.providers.insert(
            "anthropic".to_string(),
            ProviderConfigEntry {
                enabled: true,
                api_key: Some("sk-ant-1234".to_string()),
                api_base: Some("https://api.anthropic.com".to_string()),
                models: HashMap::new(),
            },
        );
        let masked = settings.masked();
        let entry = masked.providers.get("anthropic").unwrap();
        assert!(entry.api_key.as_ref().unwrap().starts_with("••••"));
        assert!(entry.api_key.as_ref().unwrap().ends_with("1234"));
        // The masked view must never expose the full key.
        assert_ne!(entry.api_key.as_deref(), Some("sk-ant-1234"));
    }

    #[test]
    fn masked_view_handles_empty_and_absent_keys() {
        let mut settings = ProviderSettings::default();
        settings.providers.insert(
            "a".to_string(),
            ProviderConfigEntry {
                enabled: true,
                api_key: None,
                api_base: None,
                models: HashMap::new(),
            },
        );
        let masked = settings.masked();
        assert_eq!(masked.providers.get("a").unwrap().api_key, None);
    }

    #[test]
    fn merge_api_key_keep_clear_overwrite() {
        let stored = Some("sk-original".to_string());

        // None → keep
        assert_eq!(
            merge_api_key(stored.clone(), None),
            Some("sk-original".to_string())
        );
        // Masked placeholder → keep
        assert_eq!(
            merge_api_key(stored.clone(), Some("••••inal".to_string())),
            Some("sk-original".to_string())
        );
        // Empty string → clear
        assert_eq!(merge_api_key(stored.clone(), Some(String::new())), None);
        // Whitespace-only → clear
        assert_eq!(merge_api_key(stored.clone(), Some("   ".to_string())), None);
        // New value → overwrite
        assert_eq!(
            merge_api_key(stored, Some("sk-new".to_string())),
            Some("sk-new".to_string())
        );
    }
}
