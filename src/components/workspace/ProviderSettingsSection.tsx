import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Eye, EyeOff, Save } from "lucide-react";
import {
  getProviderSettings,
  setProviderConfig,
  type ProviderSettings,
} from "../../lib/ipc";
import { toast } from "../../stores/toastStore";

interface ModelDef {
  id: string;
  label: string;
}

interface ProviderDefinition {
  id: string;
  label: string;
  envKey: string;
  defaultBase: string;
  models: ModelDef[];
}

const PROVIDERS: ProviderDefinition[] = [
  {
    id: "anthropic",
    label: "Anthropic",
    envKey: "ANTHROPIC_API_KEY",
    defaultBase: "https://api.anthropic.com",
    models: [
      { id: "claude-opus-4-8", label: "Opus 4.8" },
      { id: "claude-sonnet-4-6", label: "Sonnet 4.6" },
      { id: "claude-haiku-4-5-20251001", label: "Haiku 4.5" },
    ],
  },
  {
    id: "openai",
    label: "OpenAI",
    envKey: "OPENAI_API_KEY",
    defaultBase: "https://api.openai.com",
    models: [
      { id: "openai/gpt-5.5", label: "GPT-5.5" },
      { id: "openai/gpt-5.4-mini", label: "GPT-5.4 Mini" },
    ],
  },
  {
    id: "google",
    label: "Google Gemini",
    envKey: "GOOGLE_API_KEY",
    defaultBase: "https://generativelanguage.googleapis.com",
    models: [
      { id: "google/gemini-3.5-flash", label: "Gemini 3.5 Flash" },
      { id: "google/gemini-3.5-pro", label: "Gemini 3.5 Pro" },
    ],
  },
  {
    id: "openrouter",
    label: "OpenRouter",
    envKey: "OPENROUTER_API_KEY",
    defaultBase: "https://openrouter.ai/api",
    models: [
      { id: "openrouter/google/gemini-3.5-flash", label: "Gemini 3.5 Flash" },
      { id: "openrouter/qwen/qwen3.7-max", label: "Qwen3.7-Max" },
      { id: "openrouter/qwen/qwen3.7-plus", label: "Qwen3.7-Plus" },
      { id: "openrouter/deepseek/deepseek-v4-pro", label: "DeepSeek-V4-Pro" },
      { id: "openrouter/deepseek/deepseek-v4-flash", label: "DeepSeek-V4-Flash" },
      { id: "openrouter/zhipu/glm-5.2", label: "GLM-5.2" },
      { id: "openrouter/moonshotai/kimi-k2.6", label: "Kimi-K2.6" },
      { id: "openrouter/minimax/minimax-m3", label: "MiniMax-M3" },
    ],
  },
];

interface ProviderDraft {
  enabled: boolean;
  apiKey: string;
  apiBase: string;
  models: Record<string, boolean>;
}

function initDrafts(settings: ProviderSettings): Record<string, ProviderDraft> {
  const drafts: Record<string, ProviderDraft> = {};
  for (const p of PROVIDERS) {
    const entry = settings.providers[p.id];
    drafts[p.id] = {
      enabled: entry?.enabled ?? true,
      apiKey: entry?.api_key ?? "",
      apiBase: entry?.api_base ?? "",
      models: entry?.models ?? {},
    };
  }
  return drafts;
}

export function ProviderSettingsSection() {
  const { t } = useTranslation("workspace");
  const [settings, setSettings] = useState<ProviderSettings>({ providers: {} });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState<string | null>(null);
  const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
  const [drafts, setDrafts] = useState<Record<string, ProviderDraft>>({});

  useEffect(() => {
    getProviderSettings()
      .then((s) => {
        setSettings(s);
        setDrafts(initDrafts(s));
      })
      .catch(() => {
        toast.error("Failed to load provider settings");
      })
      .finally(() => setLoading(false));
  }, []);

  const updateDraft = useCallback(
    (providerId: string, field: "enabled" | "apiKey" | "apiBase", value: string | boolean) => {
      setDrafts((prev) => ({
        ...prev,
        [providerId]: { ...prev[providerId], [field]: value },
      }));
    },
    [],
  );

  const toggleModel = useCallback((providerId: string, modelId: string) => {
    setDrafts((prev) => {
      const current = prev[providerId];
      if (!current) return prev;
      // current state: look up models map, default true
      const currentlyEnabled = current.models[modelId] ?? true;
      return {
        ...prev,
        [providerId]: {
          ...current,
          models: { ...current.models, [modelId]: !currentlyEnabled },
        },
      };
    });
  }, []);

  const handleSave = useCallback(
    async (providerId: string) => {
      const draft = drafts[providerId];
      if (!draft) return;
      setSaving(providerId);
      try {
        // API key: send null when unchanged so the backend keeps the stored
        // secret (the masked placeholder must never overwrite it).  Only when
        // the user actually edited the field do we send the value — including
        // an empty string, which the backend treats as "clear".
        const savedKey = settings.providers[providerId]?.api_key ?? null;
        const apiKeyPayload = draft.apiKey === savedKey ? null : draft.apiKey;
        await setProviderConfig(
          providerId,
          draft.enabled,
          apiKeyPayload,
          draft.apiBase || null,
          Object.keys(draft.models).length > 0 ? draft.models : null,
        );
        const refreshed = await getProviderSettings();
        setSettings(refreshed);
        setDrafts(initDrafts(refreshed));
        toast.success(t("providers.saved", { defaultValue: "Provider settings saved" }));
      } catch (err) {
        toast.error(String(err));
      } finally {
        setSaving(null);
      }
    },
    [drafts, settings, t],
  );

  const toggleShowKey = useCallback((providerId: string) => {
    setShowKeys((prev) => ({ ...prev, [providerId]: !prev[providerId] }));
  }, []);

  if (loading) {
    return <div style={{ color: "var(--text-3)", padding: "16px 0" }}>Loading...</div>;
  }

  return (
    <div className="provider-settings">
      <div className="wsp-section">
        <div className="wsp-section-label">
          {t("providers.title", { defaultValue: "Model Providers" })}
        </div>
        <div className="wsp-section-hint">
          {t("providers.hint", {
            defaultValue:
              "Configure API keys and endpoints. Stored config takes precedence over .env file.",
          })}
        </div>
      </div>

      {PROVIDERS.map((provider) => {
        const draft = drafts[provider.id] ?? {
          enabled: true,
          apiKey: "",
          apiBase: "",
          models: {},
        };
        const savedEntry = settings.providers[provider.id];
        const isDirty =
          draft.enabled !== (savedEntry?.enabled ?? true) ||
          draft.apiKey !== (savedEntry?.api_key ?? "") ||
          draft.apiBase !== (savedEntry?.api_base ?? "") ||
          JSON.stringify(draft.models) !== JSON.stringify(savedEntry?.models ?? {});

        return (
          <div
            key={provider.id}
            className={`provider-card ${!draft.enabled ? "provider-disabled" : ""}`}
          >
            <div className="provider-header">
              <label className="provider-toggle">
                <input
                  type="checkbox"
                  checked={draft.enabled}
                  onChange={(e) => updateDraft(provider.id, "enabled", e.target.checked)}
                />
                <span className="provider-name">{provider.label}</span>
              </label>
              <span className="provider-env">{provider.envKey}</span>
            </div>

            {draft.enabled && (
              <>
                <div className="provider-fields">
                  <div className="provider-field">
                    <label className="provider-field-label">API Key</label>
                    <div className="provider-field-input">
                      <input
                        type={showKeys[provider.id] ? "text" : "password"}
                        value={draft.apiKey}
                        onChange={(e) => updateDraft(provider.id, "apiKey", e.target.value)}
                        placeholder={provider.envKey}
                        className="provider-input"
                      />
                      <button
                        type="button"
                        className="provider-eye-btn"
                        onClick={() => toggleShowKey(provider.id)}
                        title={showKeys[provider.id] ? "Hide" : "Show"}
                      >
                        {showKeys[provider.id] ? <EyeOff size={13} /> : <Eye size={13} />}
                      </button>
                    </div>
                  </div>

                  <div className="provider-field">
                    <label className="provider-field-label">Base URL</label>
                    <input
                      type="text"
                      value={draft.apiBase}
                      onChange={(e) => updateDraft(provider.id, "apiBase", e.target.value)}
                      placeholder={provider.defaultBase}
                      className="provider-input"
                    />
                  </div>
                </div>

                {/* Model toggles */}
                <div className="provider-models">
                  {provider.models.map((model) => {
                    const isActive = draft.models[model.id] ?? true;
                    return (
                      <button
                        key={model.id}
                        type="button"
                        className={`provider-model-chip ${isActive ? "provider-model-active" : ""}`}
                        onClick={() => toggleModel(provider.id, model.id)}
                      >
                        <span className="provider-model-dot" />
                        {model.label}
                      </button>
                    );
                  })}
                </div>
              </>
            )}

            <div className="provider-actions">
              <button
                type="button"
                className={`provider-save-btn ${isDirty ? "provider-save-dirty" : ""}`}
                onClick={() => handleSave(provider.id)}
                disabled={saving === provider.id}
              >
                <Save size={12} />
                {saving === provider.id
                  ? t("providers.saving", { defaultValue: "Saving..." })
                  : t("providers.save", { defaultValue: "Save" })}
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
