use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex},
    time::Instant,
};

use anyhow::Context;
use async_trait::async_trait;
use futures::stream::BoxStream;
use panes_agent::{
    application::ports::{EventSink, ModelClient, PermissionGateway, ToolExecutor},
    domain::{
        agents::{AgentAccessLevel, AgentProfile},
        budget::TokenBudget,
        conversation::AgentMessage,
        permission::{PermissionDecision, PermissionRequest},
        provider::{ProviderKind, ProviderProfile},
        skills::{PluginManifest, SkillDefinition},
        structured_output::StructuredOutputContract,
        tools::{ToolCall, ToolResult, ToolSpec},
    },
    infrastructure::{
        anthropic::AnthropicMessagesClient,
        env_files, google_gemini::GoogleGeminiClient, memory_files,
        native_tools::{self, NativeToolExecutor},
        openai_compatible::OpenAiCompatibleClient,
        skills,
    },
    AgentEvent, AgentRuntime, AgentRuntimePorts, ModelStreamEvent, RunTurnCommand, RuntimeMetrics,
    SystemContext,
};
use rusqlite::params;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::engines::{
    ActionResult, ActionType, Engine, EngineEvent, EngineThread, ModelInfo, SandboxPolicy,
    ThreadScope, TurnCompletionStatus, TurnInput, TurnInputItem,
};
use crate::{
    db::workspaces::get_cuelight_binding_by_root,
    engines::cuelight_tools::{
        build_cuelight_system_prompt_appendix, build_cuelight_tool_specs, execute_cuelight_tool,
        CueLightThreadContext,
    },
};

#[derive(Default)]
pub struct ClaurstNativeEngine {
    threads: Arc<Mutex<HashMap<String, ThreadState>>>,
    pending_approvals: Arc<Mutex<HashMap<String, PendingApproval>>>,
    db: Option<crate::db::Database>,
}

#[derive(Debug, Clone)]
struct ThreadState {
    root_path: PathBuf,
    model: String,
    auto_approve_commands: bool,
    sandbox_mode: Option<String>,
    cuelight_context: Option<CueLightThreadContext>,
}

struct PendingApproval {
    engine_thread_id: String,
    sender: oneshot::Sender<Value>,
}

enum ClaurstModelClient {
    Anthropic(AnthropicMessagesClient),
    OpenAiCompatible(OpenAiCompatibleClient),
    Google(GoogleGeminiClient),
}

#[async_trait]
impl ModelClient for ClaurstModelClient {
    async fn stream(
        &self,
        request: panes_agent::ModelRequest,
    ) -> anyhow::Result<BoxStream<'static, ModelStreamEvent>> {
        match self {
            Self::Anthropic(client) => client.stream(request).await,
            Self::OpenAiCompatible(client) => client.stream(request).await,
            Self::Google(client) => client.stream(request).await,
        }
    }
}

impl ClaurstNativeEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_db(&mut self, db: crate::db::Database) {
        self.db = Some(db);
    }
}

// ---------------------------------------------------------------------------
// Provider Registry — 参考 vendor/claurst 的 Provider Registry 架构
// 每个模型源是独立的 Provider，根据配置 + 环境变量凭据动态注册。
// ---------------------------------------------------------------------------

use crate::provider_config::ProviderSettings;

/// 一个模型源(provider)的注册配置。
#[allow(dead_code)]
struct ProviderConfig {
    /// 标识符，用于 model ID 前缀 (e.g. "openai", "openrouter")
    id: &'static str,
    /// 检查凭据是否可用的环境变量名
    api_key_env: &'static str,
    /// 该 provider 提供的模型列表
    models: Vec<ModelInfo>,
}

/// 检查指定 provider 是否启用且有凭据可用。
fn provider_is_available(config: &ProviderConfig, settings: &ProviderSettings) -> bool {
    if !settings.is_enabled(config.id) {
        return false;
    }
    settings.resolve_api_key(config.id, config.api_key_env).is_some()
}

/// 构建所有 Provider 注册表（含全部定义，调用者负责过滤）。
fn provider_registry() -> Vec<ProviderConfig> {
    vec![
        ProviderConfig {
            id: "anthropic",
            api_key_env: "ANTHROPIC_API_KEY",
            models: vec![
                model_info("claude-opus-4-8", "Opus 4.8", "Anthropic Claude Opus", false),
                model_info("claude-sonnet-4-6", "Sonnet 4.6", "Anthropic Claude Sonnet", true),
                model_info("claude-haiku-4-5-20251001", "Haiku 4.5", "Anthropic Claude Haiku", false),
            ],
        },
        ProviderConfig {
            id: "openai",
            api_key_env: "OPENAI_API_KEY",
            models: vec![
                model_info("openai/gpt-5.5", "GPT-5.5", "OpenAI GPT-5.5", false),
                model_info("openai/gpt-5.4-mini", "GPT-5.4 Mini", "OpenAI GPT-5.4 Mini", false),
            ],
        },
        ProviderConfig {
            id: "google",
            api_key_env: "GOOGLE_API_KEY",
            models: vec![
                model_info("google/gemini-3.5-flash", "Gemini 3.5 Flash", "Google Gemini Flash", false),
                model_info("google/gemini-3.5-pro", "Gemini 3.5 Pro", "Google Gemini Pro", false),
            ],
        },
        ProviderConfig {
            id: "openrouter",
            api_key_env: "OPENROUTER_API_KEY",
            models: vec![
                model_info("openrouter/google/gemini-3.5-flash", "Gemini 3.5 Flash", "Google Gemini via OpenRouter", false),
                model_info("openrouter/qwen/qwen3.7-max", "Qwen3.7-Max", "Qwen via OpenRouter", false),
                model_info("openrouter/qwen/qwen3.7-plus", "Qwen3.7-Plus", "Qwen via OpenRouter", false),
                model_info("openrouter/deepseek/deepseek-v4-pro", "DeepSeek-V4-Pro", "DeepSeek via OpenRouter", false),
                model_info("openrouter/deepseek/deepseek-v4-flash", "DeepSeek-V4-Flash", "DeepSeek via OpenRouter", false),
                model_info("openrouter/zhipu/glm-5.2", "GLM-5.2", "Zhipu via OpenRouter", false),
                model_info("openrouter/moonshotai/kimi-k2.6", "Kimi-K2.6", "Moonshot AI via OpenRouter", false),
                model_info("openrouter/minimax/minimax-m3", "MiniMax-M3", "MiniMax via OpenRouter", false),
            ],
        },
    ]
}

fn model_info(id: &str, display_name: &str, description: &str, is_default: bool) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        display_name: display_name.to_string(),
        description: description.to_string(),
        hidden: false,
        is_default,
        upgrade: None,
        availability_nux: None,
        upgrade_info: None,
        input_modalities: vec!["text".to_string()],
        attachment_modalities: vec![],
        limits: None,
        supports_personality: false,
        default_reasoning_effort: "medium".to_string(),
        supported_reasoning_efforts: vec![],
    }
}

#[async_trait]
impl Engine for ClaurstNativeEngine {
    fn id(&self) -> &str {
        "claurst-native"
    }

    fn name(&self) -> &str {
        "内置"
    }

    fn models(&self) -> Vec<ModelInfo> {
        // 确保 .env 已加载，使环境变量检查能正确反映用户配置
        if let Ok(cwd) = std::env::current_dir() {
            env_files::load_dotenv_for_dir(&cwd);
        }

        let settings = ProviderSettings::load();
        let mut models = Vec::new();
        for provider in provider_registry() {
            if !provider_is_available(&provider, &settings) {
                continue;
            }
            for model in provider.models {
                if settings.is_model_enabled(provider.id, &model.id) {
                    models.push(model);
                }
            }
        }
        models
    }

    async fn is_available(&self) -> bool {
        if let Ok(cwd) = std::env::current_dir() {
            env_files::load_dotenv_for_dir(&cwd);
        }
        let settings = ProviderSettings::load();
        provider_registry()
            .iter()
            .any(|p| provider_is_available(p, &settings))
    }

    async fn start_thread(
        &self,
        scope: ThreadScope,
        resume_engine_thread_id: Option<&str>,
        model: &str,
        sandbox: SandboxPolicy,
    ) -> Result<EngineThread, anyhow::Error> {
        let engine_thread_id = resume_engine_thread_id
            .map(str::to_string)
            .unwrap_or_else(|| format!("claurst-native-{}", Uuid::new_v4()));
        let root_path = match scope {
            ThreadScope::Repo { repo_path } => PathBuf::from(repo_path),
            ThreadScope::Workspace { root_path, .. } => PathBuf::from(root_path),
        };
        let cuelight_context = load_cuelight_context(self.db.clone(), &root_path).await;

        self.threads.lock().await.insert(
            engine_thread_id.clone(),
            ThreadState {
                root_path,
                model: model.to_string(),
                auto_approve_commands: false,
                sandbox_mode: sandbox.sandbox_mode.clone(),
                cuelight_context,
            },
        );

        Ok(EngineThread { engine_thread_id })
    }

    async fn send_message(
        &self,
        engine_thread_id: &str,
        input: TurnInput,
        event_tx: mpsc::Sender<EngineEvent>,
        cancellation: CancellationToken,
    ) -> Result<(), anyhow::Error> {
        let state = self
            .threads
            .lock()
            .await
            .get(engine_thread_id)
            .cloned()
            .with_context(|| format!("unknown claurst-native thread: {engine_thread_id}"))?;
        env_files::load_dotenv_for_dir(&state.root_path);
        let local_thread = local_thread_for_engine(self.db.as_ref(), engine_thread_id);
        let thread_metadata = local_thread
            .as_ref()
            .and_then(|thread| thread.engine_metadata.as_ref());

        if cancellation.is_cancelled() {
            let _ = event_tx
                .send(EngineEvent::TurnCompleted {
                    token_usage: None,
                    status: TurnCompletionStatus::Interrupted,
                })
                .await;
            return Ok(());
        }

        let provider_profile = provider_profile_for_thread(&state, thread_metadata, &input);
        let skill_catalog = skills::discover_skills(&state.root_path);
        let plugin_catalog = skills::discover_plugins(&state.root_path);
        let tool_specs = tool_specs_for_thread(state.cuelight_context.as_ref());
        let event_sink = TauriEventSink {
            event_tx: event_tx.clone(),
            action_starts: Arc::new(StdMutex::new(HashMap::new())),
        };
        let provider_settings = ProviderSettings::load();
        let model_client =
            model_client_for_provider(&provider_profile, &tool_specs, &provider_settings)?;
        let agent_profile = agent_profile_from_metadata(thread_metadata);
        let ports = ClaurstRuntimePorts {
            model: model_client,
            events: event_sink,
            tools: ClaurstToolExecutor {
                native: NativeToolExecutor::with_permissions(
                    state.root_path.clone(),
                    Arc::new(TauriPermissionGateway {
                        event_tx: event_tx.clone(),
                        pending_approvals: self.pending_approvals.clone(),
                        threads: self.threads.clone(),
                        engine_thread_id: engine_thread_id.to_string(),
                    }),
                )
                .with_skills(skill_catalog.clone()),
                cuelight_context: state.cuelight_context.clone(),
                root_path: state.root_path.clone(),
                sandbox_mode: state.sandbox_mode.clone(),
                agent_access: agent_profile
                    .as_ref()
                    .map(|profile| profile.access.clone())
                    .unwrap_or(AgentAccessLevel::Full),
                provider_profile: provider_profile.clone(),
                provider_settings: provider_settings.clone(),
                tool_specs: tool_specs_without_agent(&tool_specs),
                skill_catalog: skill_catalog.clone(),
                plugin_catalog: plugin_catalog.clone(),
            },
        };
        let runtime = AgentRuntime::new(ports);

        let messages = messages_for_turn(self.db.as_ref(), local_thread.as_ref(), &input)
            .unwrap_or_else(|_| vec![AgentMessage::user(input.message.clone())]);
        let memory_fragments = if provider_profile.id == "anthropic"
            || provider_profile.id == "openai"
            || provider_profile.id == "openrouter"
            || provider_profile.id == "ollama"
        {
            memory_files::load_memory_fragments(&state.root_path)
        } else {
            Vec::new()
        };
        let command = RunTurnCommand {
            conversation_id: engine_thread_id.to_string(),
            messages,
            system_context: SystemContext {
                working_directory: Some(state.root_path.to_string_lossy().into_owned()),
                custom_system_prompt: thread_metadata_string(
                    thread_metadata,
                    &["customSystemPrompt", "custom_system_prompt"],
                ),
                memory_fragments,
                append_system_prompt: state
                    .cuelight_context
                    .as_ref()
                    .map(build_cuelight_system_prompt_appendix),
                disable_memory_files: thread_metadata_bool(
                    thread_metadata,
                    &["disableMemoryFiles", "disable_memory_files", "noClaudeMd"],
                )
                .unwrap_or(false),
                provider: Some(provider_profile),
                token_budget: token_budget_from_metadata(thread_metadata),
                structured_output: structured_output_from_turn(thread_metadata),
                agent_profile,
                skill_catalog,
                plugin_catalog,
                agent_depth: 0,
                allow_nested_agents: false,
            },
            cancellation: cancellation.clone(),
        };

        let result = tokio::select! {
            result = runtime.run_turn(command) => result,
            _ = cancellation.cancelled() => {
                clear_pending_approvals_for_thread(
                    &self.pending_approvals,
                    engine_thread_id,
                ).await;
                let _ = event_tx
                    .send(EngineEvent::TurnCompleted {
                        token_usage: None,
                        status: TurnCompletionStatus::Interrupted,
                    })
                    .await;
                return Ok(());
            }
        };

        if cancellation.is_cancelled() {
            clear_pending_approvals_for_thread(&self.pending_approvals, engine_thread_id).await;
            return Ok(());
        }

        if let Err(error) = result {
            let _ = event_tx
                .send(EngineEvent::Error {
                    message: error.to_string(),
                    recoverable: true,
                })
                .await;
            let _ = event_tx
                .send(EngineEvent::TurnCompleted {
                    token_usage: None,
                    status: TurnCompletionStatus::Failed,
                })
                .await;
        }

        Ok(())
    }

    async fn steer_message(
        &self,
        _engine_thread_id: &str,
        _input: TurnInput,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn respond_to_approval(
        &self,
        approval_id: &str,
        response: serde_json::Value,
        _route: Option<crate::engines::ApprovalRequestRoute>,
    ) -> Result<(), anyhow::Error> {
        let pending = self.pending_approvals.lock().await.remove(approval_id);
        if let Some(pending) = pending {
            let _ = pending.sender.send(response);
        }
        Ok(())
    }

    async fn interrupt(&self, engine_thread_id: &str) -> Result<(), anyhow::Error> {
        clear_pending_approvals_for_thread(&self.pending_approvals, engine_thread_id).await;
        Ok(())
    }

    async fn archive_thread(&self, engine_thread_id: &str) -> Result<(), anyhow::Error> {
        self.threads.lock().await.remove(engine_thread_id);
        clear_pending_approvals_for_thread(&self.pending_approvals, engine_thread_id).await;
        Ok(())
    }

    async fn unarchive_thread(&self, _engine_thread_id: &str) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

async fn clear_pending_approvals_for_thread(
    pending_approvals: &Arc<Mutex<HashMap<String, PendingApproval>>>,
    engine_thread_id: &str,
) {
    pending_approvals
        .lock()
        .await
        .retain(|_, pending| pending.engine_thread_id != engine_thread_id);
}

struct ClaurstRuntimePorts<E> {
    model: ClaurstModelClient,
    events: E,
    tools: ClaurstToolExecutor,
}

impl<E> AgentRuntimePorts for ClaurstRuntimePorts<E>
where
    E: EventSink,
{
    type Model = ClaurstModelClient;
    type Events = E;
    type Tools = ClaurstToolExecutor;

    fn model(&self) -> &Self::Model {
        &self.model
    }

    fn events(&self) -> &Self::Events {
        &self.events
    }

    fn tools(&self) -> &Self::Tools {
        &self.tools
    }
}

struct TauriEventSink {
    event_tx: mpsc::Sender<EngineEvent>,
    action_starts: Arc<StdMutex<HashMap<String, Instant>>>,
}

#[async_trait]
impl EventSink for TauriEventSink {
    async fn emit(&self, event: AgentEvent) -> anyhow::Result<()> {
        match event {
            AgentEvent::ActionStarted {
                action_id,
                action_type,
                input,
            } => {
                if let Ok(mut starts) = self.action_starts.lock() {
                    starts.insert(action_id.clone(), Instant::now());
                }
                self.event_tx
                    .send(EngineEvent::ActionStarted {
                        action_id,
                        engine_action_id: None,
                        action_type: map_action_type(&action_type),
                        summary: action_type.clone(),
                        display_label: cue_light_tool_label(&action_type).map(str::to_string),
                        display_subtitle: None,
                        details: input,
                    })
                    .await?;
            }
            AgentEvent::ActionCompleted {
                action_id,
                output,
                is_error,
            } => {
                let duration_ms = self
                    .action_starts
                    .lock()
                    .ok()
                    .and_then(|mut starts| starts.remove(&action_id))
                    .map(|started| {
                        u64::try_from(started.elapsed().as_millis())
                            .unwrap_or(u64::MAX)
                            .max(1)
                    })
                    .unwrap_or(0);
                self.event_tx
                    .send(EngineEvent::ActionCompleted {
                        action_id,
                        result: ActionResult {
                            success: !is_error,
                            output: if is_error { None } else { Some(output.clone()) },
                            error: if is_error { Some(output) } else { None },
                            diff: None,
                            duration_ms,
                        },
                    })
                    .await?;
            }
            AgentEvent::TurnCompleted {
                token_usage,
                metrics,
            } => {
                self.event_tx
                    .send(EngineEvent::TranscriptEntry {
                        entry_type: "turn_completed_metrics".to_string(),
                        data: runtime_metrics_json(&metrics),
                    })
                    .await?;
                self.event_tx
                    .send(EngineEvent::TurnCompleted {
                        token_usage: token_usage.map(|usage| crate::engines::TokenUsage {
                            input: usage.input,
                            output: usage.output,
                            reasoning: usage.reasoning,
                            cache_read: usage.cache_read,
                            cache_write: usage.cache_write,
                            cost_usd: usage.cost_usd,
                        }),
                        status: TurnCompletionStatus::Completed,
                    })
                    .await?;
            }
            other => {
                self.event_tx.send(map_agent_event(other)).await?;
            }
        }
        Ok(())
    }
}

struct TauriPermissionGateway {
    event_tx: mpsc::Sender<EngineEvent>,
    pending_approvals: Arc<Mutex<HashMap<String, PendingApproval>>>,
    threads: Arc<Mutex<HashMap<String, ThreadState>>>,
    engine_thread_id: String,
}

#[async_trait]
impl PermissionGateway for TauriPermissionGateway {
    async fn request(&self, request: PermissionRequest) -> anyhow::Result<PermissionDecision> {
        if request.action_type != "execute_command" {
            return Ok(PermissionDecision::Allow);
        }

        if self
            .threads
            .lock()
            .await
            .get(&self.engine_thread_id)
            .map(|state| state.auto_approve_commands)
            .unwrap_or(false)
        {
            return Ok(PermissionDecision::Allow);
        }

        let approval_id = request.action_id.clone();
        let (tx, rx) = oneshot::channel::<Value>();
        self.pending_approvals.lock().await.insert(
            approval_id.clone(),
            PendingApproval {
                engine_thread_id: self.engine_thread_id.clone(),
                sender: tx,
            },
        );

        self.event_tx
            .send(EngineEvent::ApprovalRequested {
                approval_id: approval_id.clone(),
                action_type: map_action_type(&request.action_type),
                summary: request.summary,
                details: request.details,
            })
            .await?;

        let decision = match rx.await {
            Ok(response) => interpret_approval(&response),
            Err(_) => PermissionDecision::Deny,
        };

        self.pending_approvals.lock().await.remove(&approval_id);

        if matches!(decision, PermissionDecision::AllowForSession) {
            if let Some(state) = self.threads.lock().await.get_mut(&self.engine_thread_id) {
                state.auto_approve_commands = true;
            }
        }

        Ok(decision)
    }
}

struct ClaurstToolExecutor {
    native: NativeToolExecutor,
    cuelight_context: Option<CueLightThreadContext>,
    root_path: PathBuf,
    sandbox_mode: Option<String>,
    agent_access: AgentAccessLevel,
    provider_profile: ProviderProfile,
    provider_settings: ProviderSettings,
    tool_specs: Vec<ToolSpec>,
    skill_catalog: Vec<SkillDefinition>,
    plugin_catalog: Vec<PluginManifest>,
}

#[async_trait]
impl ToolExecutor for ClaurstToolExecutor {
    async fn execute(
        &self,
        call: ToolCall,
        cancellation: &CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        if cancellation.is_cancelled() {
            return Ok(ToolResult {
                tool_use_id: call.id,
                content: "tool execution cancelled".to_string(),
                is_error: true,
            });
        }

        if !tool_allowed_for_agent(&self.agent_access, &call.name) {
            return Ok(ToolResult {
                tool_use_id: call.id,
                content: format!("tool `{}` is not allowed for this agent", call.name),
                is_error: true,
            });
        }

        if call.name == "agent" {
            return self.execute_sync_agent(call, cancellation).await;
        }

        if call.name.starts_with("cuelight_") {
            let Some(context) = &self.cuelight_context else {
                return Ok(ToolResult {
                    tool_use_id: call.id,
                    content: "CueLight project is not bound to this workspace".to_string(),
                    is_error: true,
                });
            };

            let (success, output) = execute_cuelight_tool(
                &call.name,
                &call.input,
                context,
                Some(&self.root_path),
                self.sandbox_mode.as_deref(),
            )
            .await;
            return Ok(ToolResult {
                tool_use_id: call.id,
                content: output,
                is_error: !success,
            });
        }

        self.native.execute(call, cancellation).await
    }
}

impl ClaurstToolExecutor {
    async fn execute_sync_agent(
        &self,
        call: ToolCall,
        cancellation: &CancellationToken,
    ) -> anyhow::Result<ToolResult> {
        let Some(prompt) = call.input.get("prompt").and_then(Value::as_str) else {
            return Ok(ToolResult {
                tool_use_id: call.id,
                content: "agent requires input.prompt".to_string(),
                is_error: true,
            });
        };
        let agent_profile = call
            .input
            .get("agent")
            .and_then(Value::as_str)
            .map(agent_profile_from_name)
            .unwrap_or_else(AgentProfile::build);
        let mut provider_profile = self.provider_profile.clone();
        if let Some(model) = call.input.get("model").and_then(Value::as_str) {
            provider_profile.model = model.to_string();
        }
        let max_turns = call
            .input
            .get("max_turns")
            .or_else(|| call.input.get("maxTurns"))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .or(agent_profile.max_turns)
            .or(Some(8));
        let working_directory = call
            .input
            .get("working_directory")
            .or_else(|| call.input.get("workingDirectory"))
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| self.root_path.clone());
        if !working_directory.starts_with(&self.root_path) {
            return Ok(ToolResult {
                tool_use_id: call.id,
                content: "agent working_directory must stay inside workspace".to_string(),
                is_error: true,
            });
        }

        let model =
            model_client_for_provider(&provider_profile, &self.tool_specs, &self.provider_settings)?;
        let sub_events = LocalEventSink::default();
        let ports = ClaurstRuntimePorts {
            model,
            events: sub_events.clone(),
            tools: ClaurstToolExecutor {
                native: NativeToolExecutor::new(working_directory.clone())
                    .with_skills(self.skill_catalog.clone()),
                cuelight_context: self.cuelight_context.clone(),
                root_path: working_directory.clone(),
                sandbox_mode: self.sandbox_mode.clone(),
                agent_access: agent_profile.access.clone(),
                provider_profile: provider_profile.clone(),
                provider_settings: self.provider_settings.clone(),
                tool_specs: self.tool_specs.clone(),
                skill_catalog: self.skill_catalog.clone(),
                plugin_catalog: self.plugin_catalog.clone(),
            },
        };
        let runtime = AgentRuntime::new(ports);
        let mut context =
            SystemContext::new(Some(working_directory.to_string_lossy().into_owned()));
        context.agent_profile = Some(agent_profile);
        context.provider = Some(provider_profile);
        context.token_budget = Some(TokenBudget {
            max_turns,
            ..TokenBudget::default()
        });
        context.skill_catalog = self.skill_catalog.clone();
        context.plugin_catalog = self.plugin_catalog.clone();
        context.agent_depth = 1;
        context.allow_nested_agents = false;

        let result = runtime
            .run_turn(RunTurnCommand {
                conversation_id: format!("subagent-{}", call.id),
                messages: vec![AgentMessage::user(prompt.to_string())],
                system_context: context,
                cancellation: cancellation.clone(),
            })
            .await;

        match result {
            Ok(outcome) => {
                let metrics = sub_events
                    .events()
                    .into_iter()
                    .find_map(|event| match event {
                        AgentEvent::TurnCompleted { metrics, .. } => Some(metrics),
                        _ => None,
                    });
                Ok(ToolResult {
                    tool_use_id: call.id,
                    content: serde_json::json!({
                        "assistant_text": outcome.assistant_text,
                        "metrics": metrics.map(|metrics| runtime_metrics_json(&metrics)),
                    })
                    .to_string(),
                    is_error: false,
                })
            }
            Err(error) => Ok(ToolResult {
                tool_use_id: call.id,
                content: error.to_string(),
                is_error: true,
            }),
        }
    }
}

#[derive(Clone, Default)]
struct LocalEventSink {
    events: Arc<StdMutex<Vec<AgentEvent>>>,
}

impl LocalEventSink {
    fn events(&self) -> Vec<AgentEvent> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl EventSink for LocalEventSink {
    async fn emit(&self, event: AgentEvent) -> anyhow::Result<()> {
        self.events
            .lock()
            .map_err(|_| anyhow::anyhow!("local event sink poisoned"))?
            .push(event);
        Ok(())
    }
}

fn tool_allowed_for_agent(access: &AgentAccessLevel, tool_name: &str) -> bool {
    match access {
        AgentAccessLevel::Full => true,
        AgentAccessLevel::ReadOnly => {
            !matches!(
                tool_name,
                "file_write" | "write_file" | "file_edit" | "execute_command" | "agent"
            ) && !is_cuelight_mutation(tool_name)
        }
        AgentAccessLevel::SearchOnly => matches!(
            tool_name,
            "file_read" | "read_file" | "list_files" | "search" | "grep" | "glob"
        ),
    }
}

fn is_cuelight_mutation(tool_name: &str) -> bool {
    tool_name.starts_with("cuelight_create_")
        || tool_name.starts_with("cuelight_update_")
        || tool_name.starts_with("cuelight_delete_")
        || matches!(
            tool_name,
            "cuelight_upload_file" | "cuelight_generate_image" | "cuelight_generate_video"
        )
}

async fn load_cuelight_context(
    db: Option<crate::db::Database>,
    root_path: &PathBuf,
) -> Option<CueLightThreadContext> {
    let db = db?;
    let root = root_path.to_string_lossy().to_string();
    let binding = tokio::task::spawn_blocking(move || {
        get_cuelight_binding_by_root(&db, &root).ok().flatten()
    })
    .await
    .ok()
    .flatten()?;

    CueLightThreadContext::from_binding(&binding).await.ok()
}

fn tool_specs_for_thread(cuelight_context: Option<&CueLightThreadContext>) -> Vec<ToolSpec> {
    let mut specs = native_tools::tool_specs();
    if cuelight_context.is_some() {
        specs.extend(build_cuelight_tool_specs());
    }
    specs.push(agent_tool_spec());
    specs
}

fn tool_specs_without_agent(specs: &[ToolSpec]) -> Vec<ToolSpec> {
    specs
        .iter()
        .filter(|spec| spec.name != "agent")
        .cloned()
        .collect()
}

fn agent_tool_spec() -> ToolSpec {
    ToolSpec {
        name: "agent".to_string(),
        description: "Run a synchronous sub-agent with its own prompt and return its result."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string" },
                "agent": { "type": "string" },
                "model": { "type": "string" },
                "max_turns": { "type": "integer" },
                "working_directory": { "type": "string" }
            },
            "required": ["prompt"]
        }),
    }
}

fn model_client_for_provider(
    provider: &ProviderProfile,
    tool_specs: &[ToolSpec],
    settings: &ProviderSettings,
) -> anyhow::Result<ClaurstModelClient> {
    let resolved = resolve_provider_credentials(provider, settings)?;
    match provider.kind {
        ProviderKind::Anthropic => Ok(ClaurstModelClient::Anthropic(
            AnthropicMessagesClient::new(resolved.api_key, resolved.api_base, provider.model.clone())
                .with_tool_specs(
                    tool_specs
                        .iter()
                        .cloned()
                        .map(tool_spec_to_anthropic)
                        .collect(),
                ),
        )),
        ProviderKind::OpenAiCompatible => Ok(ClaurstModelClient::OpenAiCompatible(
            OpenAiCompatibleClient::new(
                Some(resolved.api_key),
                resolved.api_base,
                provider.model.clone(),
            )
            .with_tool_specs(tool_specs.to_vec()),
        )),
        ProviderKind::Google => Ok(ClaurstModelClient::Google(
            GoogleGeminiClient::new(resolved.api_key, resolved.api_base, provider.model.clone())
                .with_tool_specs(tool_specs_to_gemini(tool_specs)),
        )),
    }
}

/// Resolved (key, base) pair for a provider, taking saved `ProviderSettings`
/// first and falling back to environment variables.  This is the single place
/// where stored config becomes the request-time credential — previously each
/// client's `from_env` only read env vars, so UI-entered keys never reached
/// the API call.
struct ResolvedCredentials {
    api_key: String,
    api_base: String,
}

/// Pure credential resolver — kept free of I/O so it is unit-testable.
///
/// Priority: stored config (`ProviderSettings`) → environment variable →
/// provider default.  `api_key` has no default; missing both store and env is
/// an error.
fn resolve_provider_credentials(
    provider: &ProviderProfile,
    settings: &ProviderSettings,
) -> anyhow::Result<ResolvedCredentials> {
    let (key_env, base_envs, default_base): (&str, &[&str], &str) = match provider.kind {
        ProviderKind::Anthropic => (
            "ANTHROPIC_API_KEY",
            &["ANTHROPIC_BASE_URL", "ANTHROPIC_API_BASE", "CLAURST_API_BASE"][..],
            "https://api.anthropic.com",
        ),
        ProviderKind::Google => (
            "GOOGLE_API_KEY",
            &["GOOGLE_BASE_URL", "GOOGLE_API_BASE"][..],
            "https://generativelanguage.googleapis.com",
        ),
        ProviderKind::OpenAiCompatible => {
            let key_env = match provider.id.as_str() {
                "openrouter" => "OPENROUTER_API_KEY",
                "ollama" => "", // no key for local ollama
                _ => "",
            };
            let default_base = match provider.id.as_str() {
                "openrouter" => "https://openrouter.ai/api",
                "ollama" => "http://localhost:11434",
                _ => "https://api.openai.com",
            };
            (key_env, &[][..], default_base)
        }
    };

    // Ollama has no api key.  For other OpenAI-compatible providers without a
    // stored key, fall back to the conventional env var names.
    let api_key = settings.resolve_api_key(&provider.id, key_env).or_else(|| {
        if key_env.is_empty() {
            match provider.id.as_str() {
                "ollama" => None,
                _ => std::env::var("OPENAI_COMPATIBLE_API_KEY")
                    .ok()
                    .filter(|v| !v.trim().is_empty())
                    .or_else(|| {
                        std::env::var("OPENAI_API_KEY")
                            .ok()
                            .filter(|v| !v.trim().is_empty())
                    }),
            }
        } else {
            None
        }
    });

    let api_base = settings
        .resolve_api_base(&provider.id, base_envs)
        .unwrap_or_else(|| default_base.to_string());

    match provider.kind {
        ProviderKind::OpenAiCompatible if provider.id == "ollama" => {
            // Ollama needs no key.
            Ok(ResolvedCredentials { api_key: String::new(), api_base })
        }
        _ => {
            let api_key =
                api_key.with_context(|| format!("missing API key for provider `{}`", provider.id))?;
            Ok(ResolvedCredentials { api_key, api_base })
        }
    }
}

/// Gemini `functionDeclarations` expect a slightly different shape than the
/// raw `ToolSpec`.  Convert here so the client stays generic.
fn tool_specs_to_gemini(specs: &[ToolSpec]) -> Vec<Value> {
    specs
        .iter()
        .map(|spec| {
            json!({
                "name": spec.name,
                "description": spec.description,
                "parameters": spec.input_schema,
            })
        })
        .collect()
}

fn tool_spec_to_anthropic(spec: ToolSpec) -> Value {
    serde_json::json!({
        "name": spec.name,
        "description": spec.description,
        "input_schema": spec.input_schema,
    })
}

fn provider_profile_for_thread(
    state: &ThreadState,
    metadata: Option<&Value>,
    input: &TurnInput,
) -> ProviderProfile {
    let raw_model = input_model_override(input).unwrap_or_else(|| state.model.clone());
    let (mut provider_id, mut model) = raw_model
        .split_once('/')
        .map(|(provider, model)| (provider.to_string(), model.to_string()))
        .unwrap_or_else(|| ("anthropic".to_string(), raw_model));
    if let Some(provider) =
        thread_metadata_string(metadata, &["provider", "modelProvider", "model_provider"])
    {
        provider_id = provider;
    }
    if let Some(model_override) =
        thread_metadata_string(metadata, &["model", "modelId", "model_id"])
    {
        model = model_override;
    }
    let mut profile = ProviderProfile::infer(provider_id, model);
    if let Some(api_base) = thread_metadata_string(metadata, &["apiBase", "api_base"]) {
        profile.api_base = Some(api_base);
    }
    if let Some(api_key_env) = thread_metadata_string(metadata, &["apiKeyEnv", "api_key_env"]) {
        profile.api_key_env = Some(api_key_env);
    }
    profile
}

fn input_model_override(_input: &TurnInput) -> Option<String> {
    None
}

fn token_budget_from_metadata(metadata: Option<&Value>) -> Option<TokenBudget> {
    let metadata = metadata?;
    let budget = metadata
        .get("tokenBudget")
        .or_else(|| metadata.get("token_budget"))?;
    Some(TokenBudget {
        max_turns: json_u64(budget, &["maxTurns", "max_turns"])
            .and_then(|value| u32::try_from(value).ok()),
        max_input_tokens: json_u64(budget, &["maxInputTokens", "max_input_tokens"]),
        max_output_tokens: json_u64(budget, &["maxOutputTokens", "max_output_tokens"]),
        max_total_tokens: json_u64(budget, &["maxTotalTokens", "max_total_tokens"]),
        max_cost_usd: json_f64(budget, &["maxCostUsd", "max_cost_usd"]),
    })
}

fn structured_output_from_turn(metadata: Option<&Value>) -> Option<StructuredOutputContract> {
    let schema = metadata
        .and_then(|value| {
            value
                .get("outputSchema")
                .or_else(|| value.get("output_schema"))
        })
        .cloned()?;
    Some(StructuredOutputContract::json_schema(
        "panes_output",
        schema,
    ))
}

fn agent_profile_from_metadata(metadata: Option<&Value>) -> Option<AgentProfile> {
    let raw = thread_metadata_string(metadata, &["agent", "claurstAgent", "claurst_agent"])?;
    let mut profile = agent_profile_from_name(&raw);
    if let Some(prompt) = thread_metadata_string(metadata, &["agentPrompt", "agent_prompt"]) {
        profile.prompt_prefix = Some(prompt);
    }
    Some(profile)
}

fn agent_profile_from_name(raw: &str) -> AgentProfile {
    match raw {
        "build" => AgentProfile::build(),
        "plan" => AgentProfile::plan(),
        "explore" => AgentProfile::explore(),
        other => AgentProfile {
            name: other.to_string(),
            prompt_prefix: None,
            model: None,
            access: AgentAccessLevel::Full,
            max_turns: None,
        },
    }
}

fn thread_metadata_string(metadata: Option<&Value>, keys: &[&str]) -> Option<String> {
    let metadata = metadata?;
    keys.iter()
        .find_map(|key| metadata.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn thread_metadata_bool(metadata: Option<&Value>, keys: &[&str]) -> Option<bool> {
    let metadata = metadata?;
    keys.iter()
        .find_map(|key| metadata.get(*key).and_then(Value::as_bool))
}

fn json_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn json_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_f64))
}

fn local_thread_for_engine(
    db: Option<&crate::db::Database>,
    engine_thread_id: &str,
) -> Option<crate::models::ThreadDto> {
    let db = db?;
    crate::db::threads::find_thread_by_engine_thread_id(db, "claurst-native", engine_thread_id)
        .ok()
        .flatten()
}

fn messages_for_turn(
    db: Option<&crate::db::Database>,
    thread: Option<&crate::models::ThreadDto>,
    input: &TurnInput,
) -> anyhow::Result<Vec<AgentMessage>> {
    let current_message = expanded_turn_message(input);
    let (Some(db), Some(thread)) = (db, thread) else {
        return Ok(vec![AgentMessage::user(current_message)]);
    };
    let mut messages = Vec::new();
    for message in crate::db::messages::get_thread_messages(db, &thread.id)? {
        if message.status.as_str() == "streaming" {
            continue;
        }
        let content = message
            .content
            .clone()
            .or_else(|| text_from_blocks(message.blocks.as_ref()))
            .unwrap_or_default();
        if content.trim().is_empty() {
            continue;
        }
        match message.role.as_str() {
            "user" => messages.push(AgentMessage::user(content)),
            "assistant" => {
                messages.push(AgentMessage {
                    role: panes_agent::domain::conversation::Role::Assistant,
                    content: vec![panes_agent::domain::conversation::MessageContent::Text(
                        content,
                    )],
                });
                messages.extend(transcript_tool_messages_for_message(db, &message.id)?);
            }
            _ => {}
        }
    }

    let last_user_matches = messages
        .last()
        .and_then(|message| message.content.first())
        .and_then(|content| match content {
            panes_agent::domain::conversation::MessageContent::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .map(|text| text == current_message)
        .unwrap_or(false);
    if !last_user_matches {
        messages.push(AgentMessage::user(current_message));
    }
    Ok(messages)
}

fn transcript_tool_messages_for_message(
    db: &crate::db::Database,
    message_id: &str,
) -> anyhow::Result<Vec<AgentMessage>> {
    let conn = db.connect()?;
    let mut stmt = conn.prepare(
        "SELECT event_json FROM engine_event_logs
         WHERE message_id = ?1
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![message_id], |row| row.get::<_, String>(0))?;
    let mut messages = Vec::new();
    for row in rows {
        let raw = row?;
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("TranscriptEntry") {
            continue;
        }
        let Some(entry_type) = value.get("entry_type").and_then(Value::as_str) else {
            continue;
        };
        let Some(data) = value.get("data") else {
            continue;
        };
        match entry_type {
            "tool_use" => {
                let Some(id) = data.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(name) = data.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let input = data.get("input").cloned().unwrap_or(Value::Null);
                messages.push(AgentMessage::assistant_tool_use(id, name, input));
            }
            "tool_result" => {
                let Some(tool_use_id) = data.get("tool_use_id").and_then(Value::as_str) else {
                    continue;
                };
                let content = data
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let is_error = data
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                messages.push(AgentMessage::tool_result(tool_use_id, content, is_error));
            }
            _ => {}
        }
    }
    Ok(messages)
}

fn expanded_turn_message(input: &TurnInput) -> String {
    let mut parts = Vec::new();
    for item in &input.input_items {
        match item {
            TurnInputItem::Text { text } if !text.trim().is_empty() => {
                parts.push(text.clone());
            }
            TurnInputItem::Skill { name, path } => {
                let prompt = std::fs::read_to_string(path)
                    .unwrap_or_else(|_| format!("Skill `{name}` could not be loaded from {path}."));
                parts.push(format!("Skill `{name}` from `{path}`:\n{prompt}"));
            }
            TurnInputItem::Mention { name, path } => {
                parts.push(format!("Mention `{name}`: {path}"));
            }
            _ => {}
        }
    }
    if parts.is_empty() {
        input.message.clone()
    } else {
        parts.join("\n\n")
    }
}

fn text_from_blocks(blocks: Option<&Value>) -> Option<String> {
    let blocks = blocks?.as_array()?;
    let text = blocks
        .iter()
        .filter_map(|block| {
            let block_type = block.get("type").and_then(Value::as_str)?;
            if block_type != "text" && block_type != "reasoning" {
                return None;
            }
            block
                .get("text")
                .or_else(|| block.get("content"))
                .and_then(Value::as_str)
        })
        .collect::<Vec<_>>()
        .join("\n");
    (!text.trim().is_empty()).then_some(text)
}

fn interpret_approval(value: &Value) -> PermissionDecision {
    let raw = value
        .get("decision")
        .or_else(|| value.get("action"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_lowercase()
        .replace(['-', '_'], "");

    match raw.as_str() {
        "accept" | "allow" => PermissionDecision::Allow,
        "acceptforsession" => PermissionDecision::AllowForSession,
        _ => PermissionDecision::Deny,
    }
}

fn map_agent_event(event: AgentEvent) -> EngineEvent {
    match event {
        AgentEvent::TurnStarted { conversation_id } => EngineEvent::TurnStarted {
            client_turn_id: Some(conversation_id),
        },
        AgentEvent::TextDelta { content } => EngineEvent::TextDelta { content },
        AgentEvent::ThinkingDelta { content } => EngineEvent::ThinkingDelta { content },
        AgentEvent::ActionStarted {
            action_id,
            action_type,
            input,
        } => EngineEvent::ActionStarted {
            action_id,
            engine_action_id: None,
            action_type: map_action_type(&action_type),
            summary: action_type.clone(),
            display_label: cue_light_tool_label(&action_type).map(str::to_string),
            display_subtitle: None,
            details: input,
        },
        AgentEvent::ActionCompleted {
            action_id,
            output,
            is_error,
        } => EngineEvent::ActionCompleted {
            action_id,
            result: ActionResult {
                success: !is_error,
                output: if is_error { None } else { Some(output.clone()) },
                error: if is_error { Some(output) } else { None },
                diff: None,
                duration_ms: 0,
            },
        },
        AgentEvent::ModelTurnStarted { turn_index } => EngineEvent::TranscriptEntry {
            entry_type: "model_turn_started".to_string(),
            data: serde_json::json!({ "turn_index": turn_index }),
        },
        AgentEvent::ModelTurnCompleted {
            turn_index,
            used_tool,
            token_usage,
        } => EngineEvent::TranscriptEntry {
            entry_type: "model_turn_completed".to_string(),
            data: serde_json::json!({
                "turn_index": turn_index,
                "used_tool": used_tool,
                "token_usage": token_usage.map(|usage| serde_json::json!({
                    "input": usage.input,
                    "output": usage.output,
                    "reasoning": usage.reasoning,
                    "cache_read": usage.cache_read,
                    "cache_write": usage.cache_write,
                    "cost_usd": usage.cost_usd,
                })),
            }),
        },
        AgentEvent::TranscriptEntry { entry_type, data } => {
            EngineEvent::TranscriptEntry { entry_type, data }
        }
        AgentEvent::Error {
            message,
            recoverable,
        } => EngineEvent::Error {
            message,
            recoverable,
        },
        AgentEvent::TurnCompleted {
            token_usage,
            metrics: _,
        } => EngineEvent::TurnCompleted {
            token_usage: token_usage.map(|usage| crate::engines::TokenUsage {
                input: usage.input,
                output: usage.output,
                reasoning: usage.reasoning,
                cache_read: usage.cache_read,
                cache_write: usage.cache_write,
                cost_usd: usage.cost_usd,
            }),
            status: TurnCompletionStatus::Completed,
        },
    }
}

fn runtime_metrics_json(metrics: &RuntimeMetrics) -> Value {
    serde_json::json!({
        "model_turn_count": metrics.model_turn_count,
        "tool_call_count": metrics.tool_call_count,
        "errored_tool_call_count": metrics.errored_tool_call_count,
        "tool_counts": metrics.tool_counts,
    })
}

fn map_action_type(tool_name: &str) -> ActionType {
    match tool_name {
        "file_read" | "read_file" => ActionType::FileRead,
        "file_write" | "write_file" => ActionType::FileWrite,
        "file_edit" => ActionType::FileEdit,
        "execute_command" => ActionType::Command,
        "list_files" | "search" | "grep" | "glob" => ActionType::Search,
        _ => ActionType::Other,
    }
}

fn cue_light_tool_label(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "cuelight_project_status" => Some("查看项目状态"),
        "cuelight_get_story_bible" => Some("读取故事设计"),
        "cuelight_update_story_bible" => Some("更新故事设计"),
        "cuelight_get_visual_bible" => Some("读取视觉设计"),
        "cuelight_update_visual_bible" => Some("更新视觉设计"),
        "cuelight_list_characters" => Some("列出角色资产"),
        "cuelight_get_character" => Some("读取角色资产"),
        "cuelight_create_character" => Some("创建角色资产"),
        "cuelight_update_character" => Some("更新角色资产"),
        "cuelight_delete_character" => Some("删除角色资产"),
        "cuelight_list_scenes" => Some("列出场景资产"),
        "cuelight_get_scene" => Some("读取场景资产"),
        "cuelight_create_scene" => Some("创建场景资产"),
        "cuelight_update_scene" => Some("更新场景资产"),
        "cuelight_delete_scene" => Some("删除场景资产"),
        "cuelight_list_props" => Some("列出道具资产"),
        "cuelight_get_prop" => Some("读取道具资产"),
        "cuelight_create_prop" => Some("创建道具资产"),
        "cuelight_update_prop" => Some("更新道具资产"),
        "cuelight_delete_prop" => Some("删除道具资产"),
        "cuelight_list_episodes" => Some("列出分集剧本"),
        "cuelight_get_episode" => Some("读取分集剧本"),
        "cuelight_create_episode" => Some("创建分集剧本"),
        "cuelight_update_episode" => Some("更新分集剧本"),
        "cuelight_delete_episode" => Some("删除分集剧本"),
        "cuelight_list_storyboards" => Some("列出分镜规划"),
        "cuelight_get_storyboard" => Some("读取分镜规划"),
        "cuelight_create_storyboard" => Some("创建分镜规划"),
        "cuelight_update_storyboard" => Some("更新分镜规划"),
        "cuelight_delete_storyboard" => Some("删除分镜规划"),
        "cuelight_batch_update_storyboards" => Some("批量更新分镜规划"),
        "cuelight_upload_file" => Some("上传参考文件"),
        "cuelight_generate_image" => Some("生成图片"),
        "cuelight_generate_video" => Some("生成视频"),
        "cuelight_task_status" => Some("查询任务状态"),
        "cuelight_list_models" => Some("查看生成模型"),
        "cuelight_download_original_script" => Some("下载剧本原文"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn cancelled_turn_completes_as_interrupted_before_loading_model_credentials() {
        let engine = ClaurstNativeEngine::new();
        let thread = engine
            .start_thread(
                ThreadScope::Workspace {
                    root_path: std::env::temp_dir().to_string_lossy().into_owned(),
                    writable_roots: vec![],
                },
                None,
                AnthropicMessagesClient::default_model(),
                SandboxPolicy {
                    writable_roots: vec![],
                    allow_network: false,
                    approval_policy: None,
                    permission_profile: None,
                    approvals_reviewer: None,
                    reasoning_effort: None,
                    sandbox_mode: Some("workspace-write".to_string()),
                    service_tier: None,
                    personality: None,
                    output_schema: None,
                    opencode_agent: None,
                },
            )
            .await
            .expect("thread should start");
        let (tx, mut rx) = mpsc::channel(4);
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        engine
            .send_message(
                &thread.engine_thread_id,
                TurnInput {
                    message: "hello".to_string(),
                    attachments: vec![],
                    plan_mode: false,
                    input_items: vec![],
                },
                tx,
                cancellation,
            )
            .await
            .expect("cancelled turn should return cleanly");

        match rx.recv().await {
            Some(EngineEvent::TurnCompleted { status, .. }) => {
                assert_eq!(status, TurnCompletionStatus::Interrupted);
            }
            other => panic!("expected interrupted turn completion, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn permission_gateway_remembers_accept_for_session_per_thread() {
        let thread_id = "thread-accept-session".to_string();
        let threads = Arc::new(Mutex::new(HashMap::from([(
            thread_id.clone(),
            ThreadState {
                root_path: std::env::temp_dir(),
                model: AnthropicMessagesClient::default_model().to_string(),
                auto_approve_commands: false,
                sandbox_mode: Some("workspace-write".to_string()),
                cuelight_context: None,
            },
        )])));
        let pending_approvals = Arc::new(Mutex::new(HashMap::new()));
        let (event_tx, mut event_rx) = mpsc::channel(4);

        let first_gateway = TauriPermissionGateway {
            event_tx: event_tx.clone(),
            pending_approvals: pending_approvals.clone(),
            threads: threads.clone(),
            engine_thread_id: thread_id.clone(),
        };
        let first_request = PermissionRequest {
            action_id: "approval-1".to_string(),
            action_type: "execute_command".to_string(),
            summary: "cargo test".to_string(),
            details: json!({ "command": "cargo test" }),
        };

        let decision_task = tokio::spawn(async move { first_gateway.request(first_request).await });

        match event_rx.recv().await {
            Some(EngineEvent::ApprovalRequested { approval_id, .. }) => {
                assert_eq!(approval_id, "approval-1");
            }
            other => panic!("expected approval request, got {other:?}"),
        }

        let pending = pending_approvals
            .lock()
            .await
            .remove("approval-1")
            .expect("approval should be pending");
        pending
            .sender
            .send(json!({ "decision": "accept_for_session" }))
            .expect("approval response should send");

        let first_decision = decision_task
            .await
            .expect("permission task should join")
            .expect("permission request should complete");
        assert_eq!(first_decision, PermissionDecision::AllowForSession);
        assert!(
            threads
                .lock()
                .await
                .get(&thread_id)
                .expect("thread should exist")
                .auto_approve_commands
        );

        let second_gateway = TauriPermissionGateway {
            event_tx,
            pending_approvals,
            threads,
            engine_thread_id: thread_id,
        };
        let second_decision = second_gateway
            .request(PermissionRequest {
                action_id: "approval-2".to_string(),
                action_type: "execute_command".to_string(),
                summary: "cargo check".to_string(),
                details: json!({ "command": "cargo check" }),
            })
            .await
            .expect("second permission request should complete");

        assert_eq!(second_decision, PermissionDecision::Allow);
        assert!(
            event_rx.try_recv().is_err(),
            "session approval should skip future approval prompts for the same thread"
        );
    }

    #[tokio::test]
    async fn event_sink_labels_cuelight_actions_and_records_duration() {
        let (event_tx, mut event_rx) = mpsc::channel(4);
        let sink = TauriEventSink {
            event_tx,
            action_starts: Arc::new(StdMutex::new(HashMap::new())),
        };

        sink.emit(AgentEvent::ActionStarted {
            action_id: "action-visual".to_string(),
            action_type: "cuelight_get_visual_bible".to_string(),
            input: json!({}),
        })
        .await
        .expect("action start should emit");

        tokio::time::sleep(std::time::Duration::from_millis(2)).await;

        sink.emit(AgentEvent::ActionCompleted {
            action_id: "action-visual".to_string(),
            output: "{}".to_string(),
            is_error: false,
        })
        .await
        .expect("action completion should emit");

        match event_rx.recv().await {
            Some(EngineEvent::ActionStarted {
                summary,
                display_label,
                ..
            }) => {
                assert_eq!(summary, "cuelight_get_visual_bible");
                assert_eq!(display_label.as_deref(), Some("读取视觉设计"));
            }
            other => panic!("expected action started, got {other:?}"),
        }

        match event_rx.recv().await {
            Some(EngineEvent::ActionCompleted { result, .. }) => {
                assert!(result.duration_ms > 0);
            }
            other => panic!("expected action completed, got {other:?}"),
        }
    }

    #[test]
    fn expanded_turn_message_embeds_skill_prompt_items() {
        let root = std::env::temp_dir().join(format!("panes-skill-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("temp root");
        let skill_path = root.join("SKILL.md");
        std::fs::write(&skill_path, "Use the project conventions.").expect("skill");

        let message = expanded_turn_message(&TurnInput {
            message: "fallback".to_string(),
            attachments: vec![],
            plan_mode: false,
            input_items: vec![
                TurnInputItem::Text {
                    text: "Implement feature".to_string(),
                },
                TurnInputItem::Skill {
                    name: "repo-skill".to_string(),
                    path: skill_path.to_string_lossy().into_owned(),
                },
            ],
        });

        assert!(message.contains("Implement feature"));
        assert!(message.contains("Skill `repo-skill`"));
        assert!(message.contains("Use the project conventions."));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_credentials_uses_stored_key_for_anthropic() {
        // Stored config must take precedence and avoid any env-var dependency,
        // which is the whole point of the fix.
        let mut settings = ProviderSettings::default();
        settings.providers.insert(
            "anthropic".to_string(),
            crate::provider_config::ProviderConfigEntry {
                enabled: true,
                api_key: Some("sk-stored-anthropic".to_string()),
                api_base: Some("https://custom.anthropic.example".to_string()),
                models: HashMap::new(),
            },
        );
        let profile = ProviderProfile::infer("anthropic", "claude-sonnet-4-6");

        let resolved = resolve_provider_credentials(&profile, &settings).expect("should resolve");

        assert_eq!(resolved.api_key, "sk-stored-anthropic");
        assert_eq!(resolved.api_base, "https://custom.anthropic.example");
    }

    #[test]
    fn resolve_credentials_falls_back_to_default_base_when_unset() {
        let mut settings = ProviderSettings::default();
        settings.providers.insert(
            "google".to_string(),
            crate::provider_config::ProviderConfigEntry {
                enabled: true,
                api_key: Some("google-key".to_string()),
                api_base: None,
                models: HashMap::new(),
            },
        );
        let profile = ProviderProfile::infer("google", "gemini-3.5-flash");

        let resolved = resolve_provider_credentials(&profile, &settings).expect("should resolve");

        assert_eq!(resolved.api_key, "google-key");
        assert_eq!(
            resolved.api_base,
            "https://generativelanguage.googleapis.com"
        );
    }

    #[test]
    fn resolve_credentials_ollama_needs_no_key() {
        // Ollama is keyless; it should resolve to an empty key + its default
        // base URL regardless of stored config or env vars.
        let settings = ProviderSettings::default();
        let profile = ProviderProfile::infer("ollama", "llama3.2");

        let resolved = resolve_provider_credentials(&profile, &settings).expect("should resolve");

        assert_eq!(resolved.api_key, "");
        assert_eq!(resolved.api_base, "http://localhost:11434");
    }
}
