#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    Anthropic,
    OpenAiCompatible,
    Google,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderProfile {
    pub id: String,
    pub kind: ProviderKind,
    pub model: String,
    pub api_base: Option<String>,
    pub api_key_env: Option<String>,
}

impl ProviderProfile {
    pub fn infer(provider_id: impl Into<String>, model: impl Into<String>) -> Self {
        let id = provider_id.into();
        let model = model.into();
        let kind = match id.as_str() {
            "anthropic" => ProviderKind::Anthropic,
            "google" => ProviderKind::Google,
            _ => ProviderKind::OpenAiCompatible,
        };
        let api_key_env = match id.as_str() {
            "anthropic" => Some("ANTHROPIC_API_KEY".to_string()),
            "openai" => Some("OPENAI_API_KEY".to_string()),
            "google" => Some("GOOGLE_API_KEY".to_string()),
            "openrouter" => Some("OPENROUTER_API_KEY".to_string()),
            "ollama" => None,
            _ => Some("OPENAI_API_KEY".to_string()),
        };
        let api_base = match id.as_str() {
            "openai" => Some("https://api.openai.com".to_string()),
            "google" => Some("https://generativelanguage.googleapis.com".to_string()),
            "openrouter" => Some("https://openrouter.ai/api".to_string()),
            "ollama" => Some("http://localhost:11434".to_string()),
            _ => None,
        };

        Self {
            id,
            kind,
            model,
            api_base,
            api_key_env,
        }
    }
}
