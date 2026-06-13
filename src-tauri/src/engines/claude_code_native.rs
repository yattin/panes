use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use claude_code_rs::api::ApiClient;
use claude_code_rs::config::Settings;
use claude_code_rs::ChatMessage as ClaudeChatMessage;

use crate::engines::events::{TokenUsage, TurnCompletionStatus};
use crate::engines::{
    ApprovalRequestRoute, Engine, EngineEvent, EngineThread, ModelInfo, SandboxPolicy, ThreadScope,
    TurnInput,
};

/// 单个会话的对话历史
#[derive(Default)]
struct ThreadState {
    history: Vec<ClaudeChatMessage>,
}

/// Claude Code Native 引擎 - 基于 claude-code-rust 库的内置 agent
///
/// 该引擎将 claude-code-rust 作为 Rust 库直接嵌入到 Panes 后端，
/// 通过其 `ApiClient` 与兼容 OpenAI Chat Completions 协议的后端通信。
pub struct ClaudeCodeNativeEngine {
    threads: Arc<Mutex<HashMap<String, ThreadState>>>,
}

impl Default for ClaudeCodeNativeEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeCodeNativeEngine {
    pub fn new() -> Self {
        Self {
            threads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 构造 ApiClient，优先使用 claude-code-rust 的已保存配置，
    /// 缺省时回退到默认配置（API key 由用户在 claude-code 配置中设置）。
    fn build_client(model: &str) -> ApiClient {
        let mut settings = Settings::load().unwrap_or_default();
        if !model.is_empty() {
            settings.model = model.to_string();
        }
        ApiClient::new(settings)
    }
}

#[async_trait]
impl Engine for ClaudeCodeNativeEngine {
    fn id(&self) -> &str {
        "claude-code-native"
    }

    fn name(&self) -> &str {
        "Claude Code (Native)"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-opus-4".to_string(),
                display_name: "Claude Opus 4".to_string(),
                description: "最强大的模型".to_string(),
                hidden: false,
                is_default: true,
                upgrade: None,
                availability_nux: None,
                upgrade_info: None,
                input_modalities: vec!["text".to_string()],
                attachment_modalities: vec!["text".to_string()],
                limits: None,
                supports_personality: false,
                default_reasoning_effort: "medium".to_string(),
                supported_reasoning_efforts: vec![],
            },
            ModelInfo {
                id: "claude-sonnet-4".to_string(),
                display_name: "Claude Sonnet 4".to_string(),
                description: "性能与速度均衡".to_string(),
                hidden: false,
                is_default: false,
                upgrade: None,
                availability_nux: None,
                upgrade_info: None,
                input_modalities: vec!["text".to_string()],
                attachment_modalities: vec!["text".to_string()],
                limits: None,
                supports_personality: false,
                default_reasoning_effort: "medium".to_string(),
                supported_reasoning_efforts: vec![],
            },
        ]
    }

    async fn is_available(&self) -> bool {
        // 当 claude-code-rust 配置存在且包含 API key 时认为可用。
        // 即使没有配置，引擎本身也已就绪，只是发送消息时会提示配置 API key。
        true
    }

    async fn start_thread(
        &self,
        _scope: ThreadScope,
        resume_engine_thread_id: Option<&str>,
        _model: &str,
        _sandbox: SandboxPolicy,
    ) -> Result<EngineThread, anyhow::Error> {
        let thread_id = resume_engine_thread_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        self.threads
            .lock()
            .await
            .entry(thread_id.clone())
            .or_default();

        Ok(EngineThread {
            engine_thread_id: thread_id,
        })
    }

    async fn send_message(
        &self,
        engine_thread_id: &str,
        input: TurnInput,
        event_tx: tokio::sync::mpsc::Sender<EngineEvent>,
        cancellation: CancellationToken,
    ) -> Result<(), anyhow::Error> {
        let _ = event_tx
            .send(EngineEvent::TurnStarted {
                client_turn_id: None,
            })
            .await;

        // 计划模式下追加系统提示，要求仅规划不执行。
        let mut history = {
            let mut guard = self.threads.lock().await;
            let state = guard.entry(engine_thread_id.to_string()).or_default();
            state.history.push(ClaudeChatMessage::user(input.message.clone()));
            state.history.clone()
        };

        if input.plan_mode {
            history.insert(
                0,
                ClaudeChatMessage::system(
                    "计划模式：只进行规划，不要执行任何编辑或命令。".to_string(),
                ),
            );
        }

        let model = {
            // 用户在 claude-code 配置中选定的模型；start_thread 未持久化模型，
            // 因此这里使用配置默认值。
            Settings::load().map(|s| s.model).unwrap_or_default()
        };
        let client = Self::build_client(&model);

        // 在取消与 API 调用之间竞争。
        let chat_future = client.chat(history.clone(), None);

        let response = tokio::select! {
            _ = cancellation.cancelled() => {
                let _ = event_tx
                    .send(EngineEvent::TurnCompleted {
                        token_usage: None,
                        status: TurnCompletionStatus::Interrupted,
                    })
                    .await;
                return Ok(());
            }
            result = chat_future => result,
        };

        match response {
            Ok(chat_response) => {
                let assistant_text = chat_response
                    .choices
                    .first()
                    .and_then(|choice| choice.message.content.clone())
                    .unwrap_or_default();

                if !assistant_text.is_empty() {
                    let _ = event_tx
                        .send(EngineEvent::TextDelta {
                            content: assistant_text.clone(),
                        })
                        .await;

                    // 将助手回复写入会话历史，供后续轮次使用。
                    let mut guard = self.threads.lock().await;
                    if let Some(state) = guard.get_mut(engine_thread_id) {
                        state
                            .history
                            .push(ClaudeChatMessage::assistant(assistant_text));
                    }
                }

                let token_usage = chat_response.usage.map(|usage| TokenUsage {
                    input: usage.prompt_tokens as u64,
                    output: usage.completion_tokens as u64,
                    reasoning: None,
                    cache_read: None,
                    cache_write: None,
                    cost_usd: None,
                });

                let _ = event_tx
                    .send(EngineEvent::TurnCompleted {
                        token_usage,
                        status: TurnCompletionStatus::Completed,
                    })
                    .await;
            }
            Err(error) => {
                let _ = event_tx
                    .send(EngineEvent::Error {
                        message: format!("Claude Code Native 请求失败: {error}"),
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
        }

        Ok(())
    }

    async fn steer_message(
        &self,
        _engine_thread_id: &str,
        _input: TurnInput,
    ) -> Result<(), anyhow::Error> {
        // 该引擎暂不支持轮次中插话（steering）。
        Ok(())
    }

    async fn respond_to_approval(
        &self,
        _approval_id: &str,
        _response: serde_json::Value,
        _route: Option<ApprovalRequestRoute>,
    ) -> Result<(), anyhow::Error> {
        // 当前实现不发起审批请求，因此无需处理审批响应。
        Ok(())
    }

    async fn interrupt(&self, _engine_thread_id: &str) -> Result<(), anyhow::Error> {
        // 中断通过 send_message 中的 CancellationToken 处理。
        Ok(())
    }

    async fn archive_thread(&self, engine_thread_id: &str) -> Result<(), anyhow::Error> {
        self.threads.lock().await.remove(engine_thread_id);
        Ok(())
    }

    async fn unarchive_thread(&self, _engine_thread_id: &str) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
