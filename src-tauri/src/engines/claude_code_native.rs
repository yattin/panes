use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::{oneshot, Mutex};
use tokio_util::sync::CancellationToken;

use claude_code_rs::api::{ApiClient, ToolCall as ClaudeToolCall, ToolCallFunction, ToolDefinition};
use claude_code_rs::config::Settings;
use claude_code_rs::tools::{
    ExecuteCommandTool, FileEditTool, FileReadTool, FileWriteTool, ListFilesTool, SearchTool,
    TaskManagementTool, Tool,
};
use claude_code_rs::ChatMessage as ClaudeChatMessage;

use crate::engines::events::{ActionType, ActionResult, TokenUsage, TurnCompletionStatus};
use crate::engines::{
    ApprovalRequestRoute, Engine, EngineEvent, EngineThread, ModelInfo, SandboxPolicy, ThreadScope,
    TurnInput,
};

/// 工具输出回喂给 LLM 时的最大长度（字节），避免单次读取撑爆上下文。
const MAX_TOOL_OUTPUT_BYTES: usize = 64 * 1024;
/// 单轮 send_message 内允许的最多 agent 循环轮数，防止失控。
const MAX_AGENT_ROUNDS: usize = 12;
/// execute_command 默认超时（秒）。
const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 60;

/// 单个会话的对话历史与工作目录
struct ThreadState {
    history: Vec<ClaudeChatMessage>,
    /// 关联的工作目录（来自 ThreadScope），工具相对路径以此为根。
    root_path: Option<PathBuf>,
    /// Panes UI 选择的模型（来自 start_thread 的 model 参数）。
    model: Option<String>,
    /// 沙箱模式（read-only / workspace-write / danger-full-access），决定写工具是否放行。
    sandbox_mode: Option<String>,
    /// 每会话任务列表（task_management 工具的持久存储）。
    tasks: Option<Arc<TaskManagementTool>>,
    /// 用户在审批中选择 accept_for_session 后置 true，后续 execute_command 自动放行。
    auto_approve_commands: bool,
}

impl Default for ThreadState {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            root_path: None,
            model: None,
            sandbox_mode: None,
            tasks: None,
            auto_approve_commands: false,
        }
    }
}

/// Claude Code Native 引擎 - 基于 claude-code-rust 库的内置 agent
///
/// 该引擎将 claude-code-rust 作为 Rust 库直接嵌入到 Panes 后端，
/// 通过其 `ApiClient` 与兼容 OpenAI Chat Completions 协议的后端通信。
pub struct ClaudeCodeNativeEngine {
    threads: Arc<Mutex<HashMap<String, ThreadState>>>,
    /// 待处理的命令审批：approval_id -> oneshot sender。respond_to_approval 唤醒。
    pending_approvals: Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>>,
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
            pending_approvals: Arc::new(Mutex::new(HashMap::new())),
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

    /// 从 ThreadScope 提取工作目录绝对路径。
    fn root_path_from_scope(scope: &ThreadScope) -> Option<PathBuf> {
        match scope {
            ThreadScope::Repo { repo_path } => Some(PathBuf::from(repo_path)),
            ThreadScope::Workspace { root_path, .. } => Some(PathBuf::from(root_path)),
        }
    }

    /// 构造注入每轮的 system prompt：工作目录 + 可用工具 + 可选计划模式约束。
    fn build_system_prompt(
        root_path: Option<&Path>,
        allow_writes: bool,
        plan_mode: bool,
    ) -> String {
        let mut parts = Vec::new();
        if let Some(root) = root_path {
            parts.push(format!(
                "You are running inside the Panes Native engine. Your working directory is:\n{}",
                root.display()
            ));
            if allow_writes {
                parts.push(
                    "You have tools: `file_read` (read a file), `list_files` (list a directory), `search` (regex grep), `file_write` (create/overwrite a file), `file_edit` (find-and-replace in a file), `execute_command` (run a shell command in the working directory — each run needs user approval), and `task_management` (create/update/list/complete tasks). \
                     Paths are relative to the working directory (or absolute within it). \
                     When the user asks about or wants to change files, use these tools to inspect and edit them directly. \
                     Use `execute_command` for builds, tests, git, and other shell tasks. \
                     Always prefer reading the actual file over guessing."
                        .to_string(),
                );
            } else {
                parts.push(
                    "You have read-only tools: `file_read`, `list_files`, `search` (regex grep), and `task_management`. \
                     Paths are relative to the working directory (or absolute within it). \
                     This session is read-only: you cannot create, edit, or delete files, and `execute_command` is unavailable. \
                     Always prefer reading the actual file over guessing."
                        .to_string(),
                );
            }
        } else {
            parts.push(
                "You are running inside the Panes Native engine with no working directory attached; \
                 file tools are unavailable."
                    .to_string(),
            );
        }
        if plan_mode {
            parts.push(
                "Plan mode is ON: only produce a plan, do not attempt to execute edits or commands."
                    .to_string(),
            );
        }
        parts.join("\n\n")
    }

    /// 暴露给 LLM 的工具定义。`allow_writes=false`（read-only 沙箱）时仅含读取/搜索/task 工具，
    /// 不暴露写工具与 execute_command，避免它尝试注定被拒的操作。
    fn build_tool_definitions(allow_writes: bool) -> Vec<ToolDefinition> {
        let mut raw = vec![
            FileReadTool::new().tool_definition(),
            ListFilesTool::new().tool_definition(),
            SearchTool::new().tool_definition(),
            TaskManagementTool::new().tool_definition(),
        ];
        if allow_writes {
            raw.push(FileWriteTool::new().tool_definition());
            raw.push(FileEditTool::new().tool_definition());
            raw.push(ExecuteCommandTool::new().tool_definition());
        }
        raw.into_iter()
            .filter_map(|v| serde_json::from_value::<ToolDefinition>(v).ok())
            .collect()
    }

    /// 将 LLM 请求的路径解析到工作目录内，拒绝越界（防止 `../` 逃逸）。
    /// 已存在的路径直接 canonicalize；不存在的路径（如 file_write 新建文件）
    /// 回退到 canonicalize 父目录再拼文件名，只要父目录在 root 内即放行。
    fn resolve_within_root(root: Option<&Path>, requested: &str) -> Result<PathBuf, String> {
        let root = root
            .ok_or_else(|| "no working directory associated with this thread".to_string())?;
        let root_canonical = root
            .canonicalize()
            .map_err(|e| format!("working directory is not accessible: {e}"))?;
        let requested_path = Path::new(requested);
        let candidate = if requested_path.is_absolute() {
            requested_path.to_path_buf()
        } else {
            root_canonical.join(requested_path)
        };
        // 已存在：直接规范化整路径。
        let canonical = match candidate.canonicalize() {
            Ok(c) => c,
            Err(_) => {
                // 不存在（新建文件/目录）：规范化父目录，再拼回文件名。
                let parent = candidate.parent().ok_or_else(|| {
                    format!("path has no parent component: {}", candidate.display())
                })?;
                let file_name = candidate.file_name().ok_or_else(|| {
                    format!("path has no file name component: {}", candidate.display())
                })?;
                let parent_canonical = parent.canonicalize().map_err(|_| {
                    format!("parent directory does not exist: {}", parent.display())
                })?;
                parent_canonical.join(file_name)
            }
        };
        if !canonical.starts_with(&root_canonical) {
            return Err(format!(
                "path escapes the working directory: {}",
                requested
            ));
        }
        Ok(canonical)
    }

    /// 执行单个工具调用，返回 (success, 输出文本)。路径会先经 resolve_within_root 约束。
    /// 写工具受 `sandbox_mode` 控制：read-only 沙箱下直接拒绝。
    /// 注意：execute_command 的审批门在 agent 循环里（send_message），此处只负责执行。
    async fn execute_native_tool(
        name: &str,
        args: &serde_json::Value,
        root: Option<&Path>,
        sandbox_mode: Option<&str>,
        tasks: Option<&TaskManagementTool>,
    ) -> (bool, String) {
        let writes_disabled = sandbox_mode == Some("read-only");
        match name {
            "file_read" => {
                let requested = args["file_path"].as_str().unwrap_or("");
                match Self::resolve_within_root(root, requested) {
                    Ok(path) => {
                        let input = serde_json::json!({ "file_path": path.to_string_lossy() });
                        match FileReadTool::new().execute(input).await {
                            Ok(out) => (true, truncate_output(&out.content)),
                            Err(err) => (false, err.message),
                        }
                    }
                    Err(err) => (false, err),
                }
            }
            "list_files" => {
                let requested = args["path"].as_str().unwrap_or(".");
                match Self::resolve_within_root(root, requested) {
                    Ok(path) => {
                        let input = serde_json::json!({ "path": path.to_string_lossy() });
                        match ListFilesTool::new().execute(input).await {
                            Ok(out) => (true, truncate_output(&out.content)),
                            Err(err) => (false, err.message),
                        }
                    }
                    Err(err) => (false, err),
                }
            }
            "search" => {
                let requested = args["path"].as_str().unwrap_or(".");
                match Self::resolve_within_root(root, requested) {
                    Ok(path) => {
                        let mut input = serde_json::json!({ "path": path.to_string_lossy() });
                        if let Some(pattern) = args.get("pattern").cloned() {
                            input["pattern"] = pattern;
                        }
                        if let Some(file_pattern) = args.get("file_pattern").cloned() {
                            input["file_pattern"] = file_pattern;
                        }
                        match SearchTool::new().execute(input).await {
                            Ok(out) => (true, truncate_output(&out.content)),
                            Err(err) => (false, err.message),
                        }
                    }
                    Err(err) => (false, err),
                }
            }
            "file_write" | "file_edit" if writes_disabled => (
                false,
                "write tools are disabled in read-only sandbox".to_string(),
            ),
            "file_write" => {
                let requested = args["file_path"].as_str().unwrap_or("");
                match Self::resolve_within_root(root, requested) {
                    Ok(path) => {
                        let mut input = serde_json::json!({ "file_path": path.to_string_lossy() });
                        if let Some(content) = args.get("content").cloned() {
                            input["content"] = content;
                        }
                        match FileWriteTool::new().execute(input).await {
                            Ok(_) => (true, format!("wrote {}", path.display())),
                            Err(err) => (false, err.message),
                        }
                    }
                    Err(err) => (false, err),
                }
            }
            "file_edit" => {
                let requested = args["file_path"].as_str().unwrap_or("");
                match Self::resolve_within_root(root, requested) {
                    Ok(path) => {
                        let mut input = serde_json::json!({ "file_path": path.to_string_lossy() });
                        if let Some(v) = args.get("old_content").cloned() {
                            input["old_content"] = v;
                        }
                        if let Some(v) = args.get("new_content").cloned() {
                            input["new_content"] = v;
                        }
                        match FileEditTool::new().execute(input).await {
                            Ok(_) => (true, format!("edited {}", path.display())),
                            Err(err) => (false, err.message),
                        }
                    }
                    Err(err) => (false, err),
                }
            }
            "execute_command" => {
                let command = args["command"].as_str().unwrap_or("");
                if command.trim().is_empty() {
                    return (false, "command is required".to_string());
                }
                let timeout_secs = args["timeout"]
                    .as_u64()
                    .unwrap_or(DEFAULT_COMMAND_TIMEOUT_SECS);
                // 平台感知 shell：Windows 用 cmd /C，其余用 sh -c；cwd 设为工作目录。
                let mut cmd = if cfg!(windows) {
                    let mut c = tokio::process::Command::new("cmd");
                    c.arg("/C").arg(command);
                    c
                } else {
                    let mut c = tokio::process::Command::new("sh");
                    c.arg("-c").arg(command);
                    c
                };
                if let Some(root) = root {
                    cmd.current_dir(root);
                }
                match tokio::time::timeout(Duration::from_secs(timeout_secs), cmd.output()).await {
                    Ok(Ok(output)) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if output.status.success() {
                            (true, truncate_output(&stdout))
                        } else {
                            let combined =
                                format!("exit {}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}", output.status);
                            (false, truncate_output(&combined))
                        }
                    }
                    Ok(Err(e)) => (false, format!("failed to execute command: {e}")),
                    Err(_) => (false, format!("command timed out after {timeout_secs}s")),
                }
            }
            "task_management" => {
                let Some(tool) = tasks else {
                    return (false, "task store not available for this thread".to_string());
                };
                match tool.execute(args.clone()).await {
                    Ok(out) => (true, truncate_output(&out.content)),
                    Err(err) => (false, err.message),
                }
            }
            other => (false, format!("unknown tool: {other}")),
        }
    }

    /// 将本地历史写回线程状态，供后续轮次/会话恢复使用。
    async fn persist_history(
        &self,
        engine_thread_id: &str,
        history: Vec<ClaudeChatMessage>,
    ) -> Result<(), anyhow::Error> {
        let mut guard = self.threads.lock().await;
        if let Some(state) = guard.get_mut(engine_thread_id) {
            state.history = history;
        }
        Ok(())
    }

    /// 对 execute_command 发起审批。auto_approve 为 true 时直接放行（accept_for_session 后）。
    /// 返回 Approved / Denied / Interrupted（被取消）。
    async fn request_command_approval(
        &self,
        call: &ClaudeToolCall,
        command_preview: &str,
        args: &Value,
        event_tx: &tokio::sync::mpsc::Sender<EngineEvent>,
        cancellation: &CancellationToken,
        auto_approve: bool,
        engine_thread_id: &str,
    ) -> CommandApproval {
        if auto_approve {
            return CommandApproval::Approved;
        }
        let approval_id = call.id.clone();
        let (tx, rx) = oneshot::channel::<Value>();
        self.pending_approvals
            .lock()
            .await
            .insert(approval_id.clone(), tx);

        let _ = event_tx
            .send(EngineEvent::ApprovalRequested {
                approval_id: approval_id.clone(),
                action_type: ActionType::Command,
                summary: format!("$ {command_preview}"),
                details: args.clone(),
            })
            .await;

        let outcome = tokio::select! {
            _ = cancellation.cancelled() => CommandApproval::Interrupted,
            resp = rx => match resp {
                Ok(value) => match interpret_approval(&value) {
                    ApprovalDecision::Accept => CommandApproval::Approved,
                    ApprovalDecision::AcceptForSession => {
                        // 本会话后续命令自动放行。
                        if let Some(state) = self.threads.lock().await.get_mut(engine_thread_id) {
                            state.auto_approve_commands = true;
                        }
                        CommandApproval::Approved
                    }
                    ApprovalDecision::Deny => CommandApproval::Denied,
                },
                Err(_) => CommandApproval::Denied, // sender 被丢弃（如取消）
            },
        };

        // 兜底清理：若仍在表中（被取消等），移除避免泄漏。
        self.pending_approvals.lock().await.remove(&approval_id);
        outcome
    }
}

/// 命令审批结果。
enum CommandApproval {
    Approved,
    Denied,
    Interrupted,
}

/// 审批响应判定。
enum ApprovalDecision {
    Accept,
    AcceptForSession,
    Deny,
}

/// 解析审批响应 Value 为决定。支持 decision/action 两种键，
/// accept/allow 视为通过；accept_for_session/acceptForSession 视为本会话通过；
/// 其余（decline/deny/cancel/空/未知）一律拒绝（安全默认）。
fn interpret_approval(value: &Value) -> ApprovalDecision {
    let raw = value
        .get("decision")
        .or_else(|| value.get("action"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_lowercase();
    match raw.as_str() {
        "accept" | "allow" => ApprovalDecision::Accept,
        "accept_for_session" | "acceptforsession" => ApprovalDecision::AcceptForSession,
        _ => ApprovalDecision::Deny,
    }
}

fn truncate_output(s: &str) -> String {
    if s.len() <= MAX_TOOL_OUTPUT_BYTES {
        return s.to_string();
    }
    let mut cut = s[..MAX_TOOL_OUTPUT_BYTES].to_string();
    cut.push_str("\n...[output truncated]");
    cut
}

fn action_type_for(tool_name: &str) -> ActionType {
    match tool_name {
        "file_read" => ActionType::FileRead,
        "file_write" => ActionType::FileWrite,
        "file_edit" => ActionType::FileEdit,
        "execute_command" => ActionType::Command,
        "list_files" | "search" => ActionType::Search,
        _ => ActionType::Other,
    }
}

/// 流式累积的单个工具调用（按 index 聚合 SSE 分片）。
#[derive(Default)]
struct AccumulatedToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

/// 为 ActionStarted 生成简短 summary，例如 `file_read STORY_PLAN.md`。
fn short_args_summary(tool_name: &str, args: &serde_json::Value) -> String {
    // execute_command：直接取命令字符串（截断）。
    if tool_name == "execute_command" {
        return args["command"]
            .as_str()
            .map(|s| {
                let s = s.trim();
                if s.len() > 60 {
                    format!("{}…", &s[..60])
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_default();
    }
    let key = match tool_name {
        "file_read" | "file_write" | "file_edit" => "file_path",
        "list_files" | "search" => "path",
        "task_management" => "operation",
        _ => return String::new(),
    };
    args[key]
        .as_str()
        .map(|s| {
            std::path::Path::new(s)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| s.to_string())
        })
        .unwrap_or_default()
}

#[async_trait]
impl Engine for ClaudeCodeNativeEngine {
    fn id(&self) -> &str {
        "claude-code-native"
    }

    fn name(&self) -> &str {
        "Native"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-sonnet-4-6".to_string(),
                display_name: "Claude Sonnet 4.6".to_string(),
                description: "性能与速度均衡".to_string(),
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
                id: "claude-opus-4-6".to_string(),
                display_name: "Claude Opus 4.6".to_string(),
                description: "最强大的模型".to_string(),
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
        // 仅当 ~/.panes-agent/settings.json 配置了 api_key 时认为引擎可用。
        // 不读任何环境变量；缺 key 时 onboarding/ModelPicker 健康检查会显示不可用。
        Settings::load()
            .map(|s| s.api.get_api_key().is_some())
            .unwrap_or(false)
    }

    async fn start_thread(
        &self,
        scope: ThreadScope,
        resume_engine_thread_id: Option<&str>,
        model: &str,
        sandbox: SandboxPolicy,
    ) -> Result<EngineThread, anyhow::Error> {
        let thread_id = resume_engine_thread_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let root_path = Self::root_path_from_scope(&scope);

        let mut guard = self.threads.lock().await;
        let state = guard.entry(thread_id.clone()).or_default();
        state.root_path = root_path.clone();
        state.sandbox_mode = sandbox.sandbox_mode.clone();
        // 有工作目录时为该会话分配独立的任务列表。
        if root_path.is_some() {
            state.tasks = Some(Arc::new(TaskManagementTool::new()));
        }
        if !model.is_empty() {
            state.model = Some(model.to_string());
        }

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
        use futures::StreamExt;

        let _ = event_tx
            .send(EngineEvent::TurnStarted {
                client_turn_id: None,
            })
            .await;

        // 取出历史并追加本轮用户消息；同时取出工作目录、线程模型、沙箱模式、任务列表与审批标志。
        let (mut history, root_path, thread_model, sandbox_mode, tasks, mut auto_approve_commands) = {
            let mut guard = self.threads.lock().await;
            let state = guard.entry(engine_thread_id.to_string()).or_default();
            state.history.push(ClaudeChatMessage::user(input.message.clone()));
            (
                state.history.clone(),
                state.root_path.clone(),
                state.model.clone(),
                state.sandbox_mode.clone(),
                state.tasks.clone(),
                state.auto_approve_commands,
            )
        };

        let root_ref = root_path.as_deref();
        let allow_writes = sandbox_mode.as_deref() != Some("read-only");
        let system_prompt = Self::build_system_prompt(root_ref, allow_writes, input.plan_mode);

        let model = thread_model
            .filter(|m| !m.is_empty())
            .or_else(|| Settings::load().ok().map(|s| s.model))
            .unwrap_or_default();
        let client = Self::build_client(&model);

        // 有工作目录时才挂工具；read-only 沙箱下不暴露写工具。无目录则纯文本。
        let tool_defs: Vec<ToolDefinition> = if root_ref.is_some() {
            Self::build_tool_definitions(allow_writes)
        } else {
            Vec::new()
        };

        let mut final_token_usage: Option<TokenUsage> = None;

        // agent 循环：每轮流式取一个回复，若模型请求工具则执行后继续，否则结束。
        for _round in 0..MAX_AGENT_ROUNDS {
            // 本轮消息：system prompt + 当前历史。system 不入库，每轮重新注入。
            let mut messages = Vec::with_capacity(history.len() + 1);
            messages.push(ClaudeChatMessage::system(system_prompt.clone()));
            messages.extend(history.iter().cloned());

            // 建立流式连接，与取消信号竞争。
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
                result = client.chat_stream(
                    messages,
                    if tool_defs.is_empty() { None } else { Some(tool_defs.clone()) },
                ) => match result {
                    Ok(r) => r,
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
                        return Ok(());
                    }
                },
            };

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                let _ = event_tx
                    .send(EngineEvent::Error {
                        message: format!("Claude Code Native 请求失败 ({status}): {body}"),
                        recoverable: true,
                    })
                    .await;
                let _ = event_tx
                    .send(EngineEvent::TurnCompleted {
                        token_usage: None,
                        status: TurnCompletionStatus::Failed,
                    })
                    .await;
                return Ok(());
            }

            // 按 SSE 增量消费流：累积文本 delta（即时 emit）与工具调用分片（按 index 聚合）。
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut assistant_text = String::new();
            let mut tool_acc: Vec<AccumulatedToolCall> = Vec::new();
            let mut saw_tool_calls_finish = false;
            let mut interrupted = false;

            loop {
                tokio::select! {
                    _ = cancellation.cancelled() => {
                        interrupted = true;
                        break;
                    }
                    chunk = stream.next() => match chunk {
                        None => break,
                        Some(Err(error)) => {
                            let _ = event_tx
                                .send(EngineEvent::Error {
                                    message: format!("Claude Code Native 流读取失败: {error}"),
                                    recoverable: true,
                                })
                                .await;
                            let _ = event_tx
                                .send(EngineEvent::TurnCompleted {
                                    token_usage: None,
                                    status: TurnCompletionStatus::Failed,
                                })
                                .await;
                            return Ok(());
                        }
                        Some(Ok(bytes)) => {
                            buffer.push_str(&String::from_utf8_lossy(&bytes));
                            while let Some(pos) = buffer.find('\n') {
                                let line = buffer[..pos].trim().to_string();
                                buffer = buffer[pos + 1..].to_string();
                                if line.is_empty() || !line.starts_with("data: ") {
                                    continue;
                                }
                                let data = &line[6..];
                                if data == "[DONE]" {
                                    continue;
                                }
                                let value: serde_json::Value = match serde_json::from_str(data) {
                                    Ok(v) => v,
                                    Err(_) => continue,
                                };
                                let choice = &value["choices"][0];

                                // 文本增量：即时下发，保留打字机效果。
                                if let Some(content) = choice["delta"]["content"].as_str() {
                                    if !content.is_empty() {
                                        assistant_text.push_str(content);
                                        let _ = event_tx
                                            .send(EngineEvent::TextDelta {
                                                content: content.to_string(),
                                            })
                                            .await;
                                    }
                                }

                                // 工具调用分片：按 index 聚合 id/name/arguments。
                                if let Some(calls) = choice["delta"]["tool_calls"].as_array() {
                                    for call in calls {
                                        let idx = call["index"].as_u64().unwrap_or(0) as usize;
                                        while tool_acc.len() <= idx {
                                            tool_acc.push(AccumulatedToolCall::default());
                                        }
                                        if let Some(id) = call["id"].as_str() {
                                            tool_acc[idx].id = Some(id.to_string());
                                        }
                                        if let Some(name) = call["function"]["name"].as_str() {
                                            tool_acc[idx].name = Some(name.to_string());
                                        }
                                        if let Some(args) = call["function"]["arguments"].as_str() {
                                            tool_acc[idx].arguments.push_str(args);
                                        }
                                    }
                                }

                                if let Some(reason) = choice["finish_reason"].as_str() {
                                    if reason == "tool_calls" {
                                        saw_tool_calls_finish = true;
                                    }
                                }

                                // usage 通常随最后一个 chunk 单独返回（choices 为空）。
                                if let Some(usage) = value.get("usage").filter(|u| u.is_object()) {
                                    final_token_usage = Some(TokenUsage {
                                        input: usage["prompt_tokens"].as_u64().unwrap_or(0),
                                        output: usage["completion_tokens"].as_u64().unwrap_or(0),
                                        reasoning: None,
                                        cache_read: None,
                                        cache_write: None,
                                        cost_usd: None,
                                    });
                                }
                            }
                        }
                    },
                }
            }

            if interrupted {
                let _ = event_tx
                    .send(EngineEvent::TurnCompleted {
                        token_usage: None,
                        status: TurnCompletionStatus::Interrupted,
                    })
                    .await;
                return Ok(());
            }

            let pending_tools: Vec<AccumulatedToolCall> = tool_acc
                .into_iter()
                .filter(|t| t.name.is_some())
                .collect();
            let wants_tools = saw_tool_calls_finish || !pending_tools.is_empty();

            if !wants_tools {
                // 纯文本最终回复（已在上面流式 emit），落库后结束。
                if !assistant_text.is_empty() {
                    history.push(ClaudeChatMessage::assistant(assistant_text));
                }
                let _ = self
                    .persist_history(engine_thread_id, history)
                    .await;
                let _ = event_tx
                    .send(EngineEvent::TurnCompleted {
                        token_usage: final_token_usage,
                        status: TurnCompletionStatus::Completed,
                    })
                    .await;
                return Ok(());
            }

            // 模型请求了工具：把带 tool_calls 的 assistant 消息入历史。
            let assistant_tool_calls: Vec<ClaudeToolCall> = pending_tools
                .iter()
                .map(|t| ClaudeToolCall {
                    id: t.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    r#type: "function".to_string(),
                    function: ToolCallFunction {
                        name: t.name.clone().unwrap_or_default(),
                        arguments: t.arguments.clone(),
                    },
                })
                .collect();
            history.push(ClaudeChatMessage::assistant_with_tools(
                assistant_tool_calls.clone(),
            ));

            // 逐个执行工具，emit Action 事件，并把结果以 tool 角色入历史。
            for call in &assistant_tool_calls {
                let args: serde_json::Value =
                    serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::Value::Null);
                let summary = format!(
                    "{} {}",
                    call.function.name,
                    short_args_summary(&call.function.name, &args)
                );

                // execute_command 需先过审批门（auto_approve 时跳过）。
                if call.function.name == "execute_command" {
                    let command_preview = args["command"].as_str().unwrap_or("");
                    match self
                        .request_command_approval(
                            call,
                            command_preview,
                            &args,
                            &event_tx,
                            &cancellation,
                            auto_approve_commands,
                            engine_thread_id,
                        )
                        .await
                    {
                        CommandApproval::Interrupted => {
                            let _ = self.persist_history(engine_thread_id, history).await;
                            let _ = event_tx
                                .send(EngineEvent::TurnCompleted {
                                    token_usage: None,
                                    status: TurnCompletionStatus::Interrupted,
                                })
                                .await;
                            return Ok(());
                        }
                        CommandApproval::Denied => {
                            // 仍记录 ActionStarted/Completed，结果回喂「用户拒绝」。
                            auto_approve_commands = self
                                .threads
                                .lock()
                                .await
                                .get(engine_thread_id)
                                .map(|s| s.auto_approve_commands)
                                .unwrap_or(auto_approve_commands);
                            let _ = event_tx
                                .send(EngineEvent::ActionStarted {
                                    action_id: call.id.clone(),
                                    engine_action_id: Some(call.id.clone()),
                                    action_type: ActionType::Command,
                                    summary,
                                    details: args.clone(),
                                })
                                .await;
                            let _ = event_tx
                                .send(EngineEvent::ActionCompleted {
                                    action_id: call.id.clone(),
                                    result: ActionResult {
                                        success: false,
                                        output: Some("user denied this command".to_string()),
                                        error: Some("user denied this command".to_string()),
                                        diff: None,
                                        duration_ms: 0,
                                    },
                                })
                                .await;
                            history.push(ClaudeChatMessage::tool(
                                call.id.clone(),
                                "user denied this command".to_string(),
                            ));
                            continue;
                        }
                        CommandApproval::Approved => {
                            // 审批可能把 auto_approve 置位，回读以反映在后续迭代。
                            auto_approve_commands = self
                                .threads
                                .lock()
                                .await
                                .get(engine_thread_id)
                                .map(|s| s.auto_approve_commands)
                                .unwrap_or(auto_approve_commands);
                        }
                    }
                }

                let _ = event_tx
                    .send(EngineEvent::ActionStarted {
                        action_id: call.id.clone(),
                        engine_action_id: Some(call.id.clone()),
                        action_type: action_type_for(&call.function.name),
                        summary,
                        details: args.clone(),
                    })
                    .await;

                let started = std::time::Instant::now();
                let (success, output) = Self::execute_native_tool(
                    &call.function.name,
                    &args,
                    root_ref,
                    sandbox_mode.as_deref(),
                    tasks.as_deref(),
                )
                .await;
                let duration_ms = started.elapsed().as_millis() as u64;

                let _ = event_tx
                    .send(EngineEvent::ActionCompleted {
                        action_id: call.id.clone(),
                        result: ActionResult {
                            success,
                            output: Some(output.clone()),
                            error: if success { None } else { Some(output.clone()) },
                            diff: None,
                            duration_ms,
                        },
                    })
                    .await;

                history.push(ClaudeChatMessage::tool(call.id.clone(), output));
            }
            // 继续下一轮：模型会基于工具结果给出最终文本。
        }

        // 超出最大轮数仍未收敛：按失败结束，避免无限循环。
        let _ = self.persist_history(engine_thread_id, history).await;
        let _ = event_tx
            .send(EngineEvent::Error {
                message: format!(
                    "Claude Code Native 超过最大工具循环轮数 ({MAX_AGENT_ROUNDS})，已中止。"
                ),
                recoverable: true,
            })
            .await;
        let _ = event_tx
            .send(EngineEvent::TurnCompleted {
                token_usage: final_token_usage,
                status: TurnCompletionStatus::Failed,
            })
            .await;
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
        approval_id: &str,
        response: serde_json::Value,
        _route: Option<ApprovalRequestRoute>,
    ) -> Result<(), anyhow::Error> {
        // 唤醒在 request_command_approval 中等待的 oneshot。
        let sender = self.pending_approvals.lock().await.remove(approval_id);
        match sender {
            Some(tx) => {
                let _ = tx.send(response);
                Ok(())
            }
            None => Err(anyhow::anyhow!(
                "no pending approval with id {approval_id} (already resolved or unknown)"
            )),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::events::TurnCompletionStatus;
    use tokio::sync::mpsc;

    /// 生成一个唯一临时目录作为测试用工作目录。
    fn temp_root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "panes_native_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp root");
        dir
    }

    #[tokio::test]
    async fn tool_file_write_roundtrip() {
        let root = temp_root();
        let (ok, msg) = ClaudeCodeNativeEngine::execute_native_tool(
            "file_write",
            &serde_json::json!({ "file_path": "out.txt", "content": "hello native" }),
            Some(root.as_path()),
            Some("workspace-write"),
            None,
        )
        .await;
        assert!(ok, "file_write should succeed in workspace-write sandbox: {msg}");
        let written =
            std::fs::read_to_string(root.join("out.txt")).expect("file should exist on disk");
        assert_eq!(written, "hello native");
    }

    #[tokio::test]
    async fn readonly_sandbox_blocks_writes() {
        let root = temp_root();
        let (ok, msg) = ClaudeCodeNativeEngine::execute_native_tool(
            "file_write",
            &serde_json::json!({ "file_path": "out.txt", "content": "x" }),
            Some(root.as_path()),
            Some("read-only"),
            None,
        )
        .await;
        assert!(!ok, "file_write must be blocked in read-only sandbox");
        assert!(
            msg.contains("disabled"),
            "expected disabled message, got: {msg}"
        );
        assert!(
            !root.join("out.txt").exists(),
            "no file should be written when blocked"
        );
    }

    #[test]
    fn resolve_rejects_escape() {
        let root = temp_root();
        let result = ClaudeCodeNativeEngine::resolve_within_root(
            Some(root.as_path()),
            "../escape.txt",
        );
        assert!(result.is_err(), "escape path should be rejected");
    }

    #[test]
    fn resolve_allows_new_file() {
        let root = temp_root();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let result = ClaudeCodeNativeEngine::resolve_within_root(
            Some(root.as_path()),
            "sub/new.txt",
        );
        let resolved =
            result.expect("new file under existing subdir should resolve within root");
        // canonicalize 两端再比较（Windows 上 canonicalize 会加 \\?\ 前缀）。
        let root_canonical = root.canonicalize().unwrap();
        assert!(
            resolved.starts_with(&root_canonical),
            "resolved path must stay within root"
        );
    }

    #[tokio::test]
    async fn task_management_persists_across_calls() {
        // 同一线程的 task 实例必须跨调用保持状态：create 后 list 能看到该任务。
        let tasks = Arc::new(TaskManagementTool::new());
        let (ok_create, msg_create) = ClaudeCodeNativeEngine::execute_native_tool(
            "task_management",
            &serde_json::json!({
                "operation": "create",
                "subject": "write tests",
                "description": "cover the new tools",
                "priority": "high"
            }),
            None,
            None,
            Some(tasks.as_ref()),
        )
        .await;
        assert!(ok_create, "create should succeed: {msg_create}");

        let (ok_list, list_out) = ClaudeCodeNativeEngine::execute_native_tool(
            "task_management",
            &serde_json::json!({ "operation": "list" }),
            None,
            None,
            Some(tasks.as_ref()),
        )
        .await;
        assert!(ok_list, "list should succeed: {list_out}");
        assert!(
            list_out.contains("write tests"),
            "list output should contain the created task subject, got: {list_out}"
        );
    }

    #[tokio::test]
    async fn execute_command_runs_in_cwd() {
        let root = temp_root();
        // 在工作目录放一个 marker 文件，命令列出目录能看到它即证明 cwd 生效。
        std::fs::write(root.join("marker_native.txt"), "x").unwrap();
        let list_cmd = if cfg!(windows) { "dir /b" } else { "ls" };
        let (ok, out) = ClaudeCodeNativeEngine::execute_native_tool(
            "execute_command",
            &serde_json::json!({ "command": list_cmd, "timeout": 15 }),
            Some(root.as_path()),
            Some("workspace-write"),
            None,
        )
        .await;
        assert!(ok, "execute_command should succeed: {out}");
        assert!(
            out.contains("marker_native.txt"),
            "command output should list the marker file (cwd=root), got: {out}"
        );
    }

    #[test]
    fn approval_decision_interpretation() {
        use super::ApprovalDecision;
        let accept = |d: &str| interpret_approval(&serde_json::json!({ "decision": d }));
        let accept_action = |a: &str| interpret_approval(&serde_json::json!({ "action": a }));
        assert!(matches!(accept("accept"), ApprovalDecision::Accept));
        assert!(matches!(accept("allow"), ApprovalDecision::Accept));
        assert!(matches!(
            accept("accept_for_session"),
            ApprovalDecision::AcceptForSession
        ));
        assert!(matches!(
            accept("acceptForSession"),
            ApprovalDecision::AcceptForSession
        ));
        assert!(matches!(accept("deny"), ApprovalDecision::Deny));
        assert!(matches!(accept("decline"), ApprovalDecision::Deny));
        assert!(matches!(accept("cancel"), ApprovalDecision::Deny));
        assert!(matches!(accept_action("decline"), ApprovalDecision::Deny));
        // 空 / 未知 → 拒绝（安全默认）。
        assert!(matches!(
            interpret_approval(&serde_json::json!({})),
            ApprovalDecision::Deny
        ));
        assert!(matches!(accept("bogus"), ApprovalDecision::Deny));
    }

    /// 真实端到端测试：从 `~/.panes-agent/settings.json` 读取配置，
    /// 调用 Native 引擎发送一条消息，断言收到包含 "PONG" 的 TextDelta 和 Succeeded 完成。
    ///
    /// 需要预先在 `~/.panes-agent/settings.json` 配置真实可用的 api_key / base_url / model。
    /// 默认 `#[ignore]`，运行方式：
    ///     cargo test --manifest-path src-tauri/Cargo.toml \
    ///         claude_code_native::tests::real_chat_round_trip -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_chat_round_trip() {
        // 防御：如果环境里恰好设了这些变量，确保它们不会被读取（验证"仅从配置文件读取"）。
        // 注意：测试仍应成功，因为 Native 引擎已不再读这些变量。
        std::env::set_var("ANTHROPIC_API_KEY", "env-should-be-ignored");
        std::env::set_var("DASHSCOPE_API_KEY", "env-should-be-ignored");
        std::env::set_var("DEEPSEEK_API_KEY", "env-should-be-ignored");
        std::env::set_var("API_BASE_URL", "https://invalid.example.invalid");

        let engine = ClaudeCodeNativeEngine::new();
        let thread = engine
            .start_thread(
                ThreadScope::Workspace {
                    root_path: ".".to_string(),
                    writable_roots: vec![],
                },
                None,
                "claude-sonnet-4-6",
                SandboxPolicy {
                    writable_roots: vec![],
                    allow_network: true,
                    approval_policy: None,
                    permission_profile: None,
                    approvals_reviewer: None,
                    reasoning_effort: None,
                    sandbox_mode: None,
                    service_tier: None,
                    personality: None,
                    output_schema: None,
                    opencode_agent: None,
                },
            )
            .await
            .expect("start_thread should succeed");

        let (tx, mut rx) = mpsc::channel::<EngineEvent>(64);
        let cancellation = CancellationToken::new();

        let send_result = engine
            .send_message(
                &thread.engine_thread_id,
                TurnInput {
                    message: "List five fruits, one per line, each numbered. For example:\n1. Apple\nDo not add any explanation."
                        .to_string(),
                    attachments: vec![],
                    plan_mode: false,
                    input_items: vec![],
                },
                tx,
                cancellation,
            )
            .await;

        assert!(send_result.is_ok(), "send_message failed: {:?}", send_result.err());

        let mut text_chunks: Vec<String> = Vec::new();
        let mut final_status: Option<TurnCompletionStatus> = None;
        let mut error_message: Option<String> = None;
        while let Some(event) = rx.recv().await {
            match event {
                EngineEvent::TextDelta { content } => {
                    eprintln!("[TextDelta] {content:?}");
                    text_chunks.push(content);
                }
                EngineEvent::TurnCompleted { status, .. } => {
                    final_status = Some(status);
                    break;
                }
                EngineEvent::Error { message, .. } => {
                    error_message = Some(message);
                }
                other => {
                    eprintln!("[Event] {other:?}");
                }
            }
        }

        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("DASHSCOPE_API_KEY");
        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("API_BASE_URL");

        if let Some(msg) = error_message {
            panic!("engine emitted Error event: {msg}");
        }

        let assembled = text_chunks.concat();
        eprintln!("[assembled] {assembled}");
        eprintln!("[chunk count] {}", text_chunks.len());

        // 验证流式：send_message 应逐块 emit TextDelta（多块而非一整段）。
        assert!(
            text_chunks.len() >= 2,
            "expected >=2 incremental TextDelta events to prove streaming through send_message, got {}",
            text_chunks.len()
        );
        for n in 1..=5 {
            assert!(
                assembled.contains(&format!("{n}.")),
                "assembled response should contain numbered line '{n}.', got: {assembled}"
            );
        }
        assert_eq!(
            final_status,
            Some(TurnCompletionStatus::Completed),
            "expected TurnCompleted with status=Completed, got {:?}",
            final_status
        );
    }

    /// 真实流式测试：直接通过 `ApiClient::chat_stream()` + `bytes_stream()` 按 SSE 增量消费，
    /// 断言确实收到了多个 content chunk（而非一整段），证明上游代理支持真正的流式输出。
    ///
    /// 默认 `#[ignore]`，运行方式：
    ///     cargo test --manifest-path src-tauri/Cargo.toml \
    ///         claude_code_native::tests::real_chat_stream -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_chat_stream() {
        use futures::StreamExt;

        let settings = Settings::load().expect("settings should load from ~/.panes-agent/settings.json");
        assert!(
            settings.api.get_api_key().is_some(),
            "settings.json must contain a valid api_key"
        );

        let client = ApiClient::new(settings);
        let messages = vec![ClaudeChatMessage::user(
            "Count from 1 to 5, one number per line. Do not say anything else.",
        )];

        let response = client
            .chat_stream(messages, None)
            .await
            .expect("chat_stream should connect");

        assert!(
            response.status().is_success(),
            "expected HTTP 2xx, got {}",
            response.status()
        );

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut content_chunks: Vec<String> = Vec::new();
        let mut assembled = String::new();
        let mut saw_done = false;

        while let Some(chunk_result) = stream.next().await {
            let bytes = chunk_result.expect("stream chunk should be readable");
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buffer.find('\n') {
                let line: String = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() || !line.starts_with("data: ") {
                    continue;
                }
                let data = &line[6..];
                if data == "[DONE]" {
                    saw_done = true;
                    continue;
                }

                let value: serde_json::Value =
                    serde_json::from_str(data).expect("SSE data line should be valid JSON");
                if let Some(content) = value["choices"][0]["delta"]["content"].as_str() {
                    if !content.is_empty() {
                        eprintln!("[delta] {content:?}");
                        content_chunks.push(content.to_string());
                        assembled.push_str(content);
                    }
                }
            }
        }

        eprintln!("[assembled] {assembled}");
        eprintln!("[chunk count] {}", content_chunks.len());

        assert!(saw_done, "expected SSE stream to terminate with [DONE]");
        assert!(
            content_chunks.len() >= 2,
            "expected >=2 incremental content chunks to prove true streaming, got {}",
            content_chunks.len()
        );
        for n in 1..=5 {
            assert!(
                assembled.contains(&n.to_string()),
                "assembled content should contain '{n}', got: {assembled}"
            );
        }
    }

    /// 真实工作目录 + 文件读取测试：
    /// 以 `C:\book\book2` 为工作目录启动线程，让 LLM 用 file_read 工具读取
    /// STORY_PLAN.md 并报告标题，断言工具被调用且最终文本含真实文件内容（「逆流1983」）。
    ///
    /// 默认 `#[ignore]`，运行方式：
    ///     cargo test --manifest-path src-tauri/Cargo.toml \
    ///         claude_code_native::tests::real_file_read -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_file_read() {
        let engine = ClaudeCodeNativeEngine::new();
        let thread = engine
            .start_thread(
                ThreadScope::Workspace {
                    root_path: r"C:\book\book2".to_string(),
                    writable_roots: vec![],
                },
                None,
                "claude-sonnet-4-6",
                default_sandbox(),
            )
            .await
            .expect("start_thread should succeed");

        // 确认工作目录与 Panes 选择的模型都已关联到线程。
        {
            let guard = engine.threads.lock().await;
            let state = guard.get(&thread.engine_thread_id).expect("thread state");
            assert_eq!(
                state
                    .root_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string()),
                Some(r"C:\book\book2".to_string()),
                "root_path should be associated with the thread"
            );
            assert_eq!(
                state.model,
                Some("claude-sonnet-4-6".to_string()),
                "Panes-selected model should be stored on the thread"
            );
        }

        let (tx, mut rx) = mpsc::channel::<EngineEvent>(64);
        let cancellation = CancellationToken::new();

        let send_result = engine
            .send_message(
                &thread.engine_thread_id,
                TurnInput {
                    message: "Use the file_read tool to read STORY_PLAN.md (it's in the working directory), then tell me the document title shown in the first line. Just give me that title.".to_string(),
                    attachments: vec![],
                    plan_mode: false,
                    input_items: vec![],
                },
                tx,
                cancellation,
            )
            .await;
        assert!(send_result.is_ok(), "send_message failed: {:?}", send_result.err());

        let mut file_read_started = false;
        let mut file_read_succeeded = false;
        let mut assembled = String::new();
        let mut final_status: Option<TurnCompletionStatus> = None;
        let mut error_message: Option<String> = None;

        while let Some(event) = rx.recv().await {
            match event {
                EngineEvent::ActionStarted {
                    action_type,
                    summary,
                    ..
                } => {
                    eprintln!("[ActionStarted] {summary}");
                    if action_type.as_str() == ActionType::FileRead.as_str() {
                        file_read_started = true;
                    }
                }
                EngineEvent::ActionCompleted { result, .. } => {
                    eprintln!(
                        "[ActionCompleted] success={} output_len={}",
                        result.success,
                        result.output.as_deref().map(|s| s.len()).unwrap_or(0)
                    );
                    if result.success && file_read_started {
                        file_read_succeeded = true;
                    }
                }
                EngineEvent::TextDelta { content } => {
                    eprintln!("[TextDelta] {content:?}");
                    assembled.push_str(&content);
                }
                EngineEvent::TurnCompleted { status, .. } => {
                    final_status = Some(status);
                    break;
                }
                EngineEvent::Error { message, .. } => {
                    error_message = Some(message);
                }
                other => {
                    eprintln!("[Event] {other:?}");
                }
            }
        }

        if let Some(msg) = error_message {
            panic!("engine emitted Error event: {msg}");
        }

        assert!(
            file_read_started,
            "expected an ActionStarted event for file_read (LLM should have used the tool)"
        );
        assert!(
            file_read_succeeded,
            "expected the file_read ActionCompleted to succeed"
        );
        assert!(
            assembled.contains("逆流1983"),
            "assembled answer should contain the real title '逆流1983', got: {assembled}"
        );
        assert_eq!(
            final_status,
            Some(TurnCompletionStatus::Completed),
            "expected TurnCompleted with status=Completed, got {:?}",
            final_status
        );
    }

    /// 真实写循环测试：以一个临时目录为工作目录（workspace-write 沙箱），
    /// 让 LLM 用 file_write 创建文件、再 file_read 读回，断言写工具被调用、成功，
    /// 且文件真实落盘内容正确。
    ///
    /// 默认 `#[ignore]`，运行方式：
    ///     cargo test --manifest-path src-tauri/Cargo.toml \
    ///         claude_code_native::tests::real_file_write_edit -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_file_write_edit() {
        let root = temp_root();

        let engine = ClaudeCodeNativeEngine::new();
        let thread = engine
            .start_thread(
                ThreadScope::Workspace {
                    root_path: root.to_string_lossy().to_string(),
                    writable_roots: vec![],
                },
                None,
                "claude-sonnet-4-6",
                SandboxPolicy {
                    sandbox_mode: Some("workspace-write".to_string()),
                    ..default_sandbox()
                },
            )
            .await
            .expect("start_thread should succeed");

        let (tx, mut rx) = mpsc::channel::<EngineEvent>(64);
        let cancellation = CancellationToken::new();

        let target_file = root.join("hello.txt");
        // 确保测试前文件不存在，写完后才存在。
        assert!(!target_file.exists(), "precondition: target file absent");

        let send_result = engine
            .send_message(
                &thread.engine_thread_id,
                TurnInput {
                    message: "Create a file named hello.txt in the working directory with exactly this content (no extra whitespace or lines): Native was here. Then use file_read to read it back and confirm the contents.".to_string(),
                    attachments: vec![],
                    plan_mode: false,
                    input_items: vec![],
                },
                tx,
                cancellation,
            )
            .await;
        assert!(send_result.is_ok(), "send_message failed: {:?}", send_result.err());

        let mut file_write_started = false;
        let mut file_write_succeeded = false;
        let mut assembled = String::new();
        let mut final_status: Option<TurnCompletionStatus> = None;
        let mut error_message: Option<String> = None;

        while let Some(event) = rx.recv().await {
            match event {
                EngineEvent::ActionStarted {
                    action_type,
                    summary,
                    ..
                } => {
                    eprintln!("[ActionStarted] {summary}");
                    if action_type.as_str() == ActionType::FileWrite.as_str() {
                        file_write_started = true;
                    }
                }
                EngineEvent::ActionCompleted { result, .. } => {
                    eprintln!(
                        "[ActionCompleted] success={} output={:?}",
                        result.success,
                        result.output.as_deref()
                    );
                    if result.success && file_write_started {
                        file_write_succeeded = true;
                    }
                }
                EngineEvent::TextDelta { content } => {
                    eprintln!("[TextDelta] {content:?}");
                    assembled.push_str(&content);
                }
                EngineEvent::TurnCompleted { status, .. } => {
                    final_status = Some(status);
                    break;
                }
                EngineEvent::Error { message, .. } => {
                    error_message = Some(message);
                }
                other => {
                    eprintln!("[Event] {other:?}");
                }
            }
        }

        if let Some(msg) = error_message {
            panic!("engine emitted Error event: {msg}");
        }

        assert!(
            file_write_started,
            "expected an ActionStarted event for file_write"
        );
        assert!(
            file_write_succeeded,
            "expected the file_write ActionCompleted to succeed"
        );
        // 真实落盘校验：文件存在且内容正确。
        assert!(
            target_file.exists(),
            "hello.txt should exist on disk after file_write"
        );
        let disk_content =
            std::fs::read_to_string(&target_file).expect("read back written file");
        assert!(
            disk_content.contains("Native was here"),
            "disk content should contain the written text, got: {disk_content}"
        );
        assert_eq!(
            final_status,
            Some(TurnCompletionStatus::Completed),
            "expected TurnCompleted with status=Completed, got {:?}",
            final_status
        );
    }

    /// 真实 execute_command 测试：以临时目录为 root（workspace-write），
    /// 预先置 auto_approve_commands=true 跳过审批 UI，让 LLM 跑 `echo` 并报告输出。
    ///
    /// 默认 `#[ignore]`，运行方式：
    ///     cargo test --manifest-path src-tauri/Cargo.toml \
    ///         claude_code_native::tests::real_command_via_agent -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_command_via_agent() {
        let root = temp_root();

        let engine = ClaudeCodeNativeEngine::new();
        let thread = engine
            .start_thread(
                ThreadScope::Workspace {
                    root_path: root.to_string_lossy().to_string(),
                    writable_roots: vec![],
                },
                None,
                "claude-sonnet-4-6",
                SandboxPolicy {
                    sandbox_mode: Some("workspace-write".to_string()),
                    ..default_sandbox()
                },
            )
            .await
            .expect("start_thread should succeed");

        // 跳过审批 UI（单测环境无人点击批准）。
        {
            let mut guard = engine.threads.lock().await;
            if let Some(state) = guard.get_mut(&thread.engine_thread_id) {
                state.auto_approve_commands = true;
            }
        }

        let (tx, mut rx) = mpsc::channel::<EngineEvent>(64);
        let cancellation = CancellationToken::new();

        let echo_payload = "panes_native_ok";
        let send_result = engine
            .send_message(
                &thread.engine_thread_id,
                TurnInput {
                    message: format!(
                        "Use execute_command to run this exact command and report its stdout: echo {echo_payload}"
                    ),
                    attachments: vec![],
                    plan_mode: false,
                    input_items: vec![],
                },
                tx,
                cancellation,
            )
            .await;
        assert!(send_result.is_ok(), "send_message failed: {:?}", send_result.err());

        let mut command_started = false;
        let mut command_output: Option<String> = None;
        let mut assembled = String::new();
        let mut final_status: Option<TurnCompletionStatus> = None;
        let mut error_message: Option<String> = None;

        while let Some(event) = rx.recv().await {
            match event {
                EngineEvent::ActionStarted {
                    action_type,
                    summary,
                    ..
                } => {
                    eprintln!("[ActionStarted] {summary}");
                    if action_type.as_str() == ActionType::Command.as_str() {
                        command_started = true;
                    }
                }
                EngineEvent::ActionCompleted { result, .. } => {
                    eprintln!(
                        "[ActionCompleted] success={} output={:?}",
                        result.success,
                        result.output.as_deref()
                    );
                    if command_started && command_output.is_none() {
                        command_output = result.output.clone();
                    }
                }
                EngineEvent::TextDelta { content } => {
                    eprintln!("[TextDelta] {content:?}");
                    assembled.push_str(&content);
                }
                EngineEvent::TurnCompleted { status, .. } => {
                    final_status = Some(status);
                    break;
                }
                EngineEvent::Error { message, .. } => {
                    error_message = Some(message);
                }
                other => {
                    eprintln!("[Event] {other:?}");
                }
            }
        }

        if let Some(msg) = error_message {
            panic!("engine emitted Error event: {msg}");
        }
        assert!(
            command_started,
            "expected an ActionStarted event for execute_command"
        );
        let out = command_output.expect("expected command output");
        assert!(
            out.contains(echo_payload),
            "command stdout should contain '{echo_payload}', got: {out}"
        );
        assert_eq!(
            final_status,
            Some(TurnCompletionStatus::Completed),
            "expected TurnCompleted with status=Completed, got {:?}",
            final_status
        );
    }

    fn default_sandbox() -> SandboxPolicy {
        SandboxPolicy {
            writable_roots: vec![],
            allow_network: true,
            approval_policy: None,
            permission_profile: None,
            approvals_reviewer: None,
            reasoning_effort: None,
            sandbox_mode: None,
            service_tier: None,
            personality: None,
            output_schema: None,
            opencode_agent: None,
        }
    }
}
