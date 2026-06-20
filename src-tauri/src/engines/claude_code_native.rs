use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::{oneshot, Mutex};
use tokio_util::sync::CancellationToken;

use claude_code_rs::api::{
    ApiClient, ToolCall as ClaudeToolCall, ToolCallFunction, ToolDefinition,
};
use claude_code_rs::config::Settings;
use claude_code_rs::tools::{
    ExecuteCommandTool, FileEditTool, FileReadTool, FileWriteTool, ListFilesTool, SearchTool,
    TaskManagementTool, Tool,
};
use claude_code_rs::ChatMessage as ClaudeChatMessage;

use crate::db::workspaces::get_cuelight_binding_by_root;
use crate::engines::cuelight_tools::{
    build_cuelight_system_prompt, build_cuelight_tool_definitions, execute_cuelight_tool,
    CueLightThreadContext,
};
use crate::engines::events::{ActionResult, ActionType, TokenUsage, TurnCompletionStatus};
use crate::engines::{
    ApprovalRequestRoute, Engine, EngineEvent, EngineThread, ModelInfo, SandboxPolicy, ThreadScope,
    TurnInput,
};

/// 工具输出回喂给 LLM 时的最大长度（字节），避免单次读取撑爆上下文。
const MAX_TOOL_OUTPUT_BYTES: usize = 64 * 1024;
/// 单轮 send_message 内允许的最多 agent 循环轮数，防止失控。
const MAX_AGENT_ROUNDS: usize = 32;
/// execute_command 默认超时（秒）。
const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 60;
/// 上下文最大限制（token 数），用于计算使用百分比。
const CONTEXT_MAX_TOKENS: usize = 1_000_000;
/// 压缩时保留的最近消息数量。
const COMPRESS_PRESERVE_RECENT: usize = 10;

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
    /// CueLight 影视模式上下文（从 workspace 绑定加载）
    cuelight_context: Option<CueLightThreadContext>,
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
            cuelight_context: None,
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
    /// 数据库引用（用于加载 CueLight 绑定）
    db: Option<crate::db::Database>,
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
            db: None,
        }
    }

    /// 设置数据库引用（用于加载 CueLight 绑定）
    pub fn set_db(&mut self, db: crate::db::Database) {
        self.db = Some(db);
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
        cuelight_context: Option<&CueLightThreadContext>,
    ) -> String {
        // CueLight 影视模式 — 完整替换
        if let Some(ctx) = cuelight_context {
            let mut prompt = build_cuelight_system_prompt(ctx);
            if let Some(root) = root_path {
                prompt.push_str(&format!(
                    "\n\n## 本地原文分析工具\n当前工作目录：{}\n你也可以使用本地只读工具 `file_read`、`list_files`、`search` 和 `task_management`。当需要分析剧本原文时，先调用 `cuelight_download_original_script` 将原文下载到 `.cuelight/original-script/original-script.txt`，然后用本地文件工具读取和检索。",
                    root.display()
                ));
            }
            if !allow_writes {
                prompt.push_str("\n当前会话是 read-only sandbox，`cuelight_download_original_script` 无法写入本地文件；需要 workspace-write 或更高权限。");
            }
            if plan_mode {
                prompt.push_str("\n\nPlan mode is ON: only produce a plan, do not attempt to execute edits or commands.");
            }
            return prompt;
        }

        // 原有编程模式逻辑
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
    /// 如果存在 CueLight 绑定，则只暴露影视工具。
    fn build_tool_definitions(
        allow_writes: bool,
        cuelight_context: Option<&CueLightThreadContext>,
    ) -> Vec<ToolDefinition> {
        let mut raw = Vec::new();

        // CueLight 影视模式保留本地只读工具，方便下载原文后进行文件级分析。
        if cuelight_context.is_some() {
            raw.extend(build_cuelight_tool_definitions());
            raw.push(FileReadTool::new().tool_definition());
            raw.push(ListFilesTool::new().tool_definition());
            raw.push(SearchTool::new().tool_definition());
            raw.push(TaskManagementTool::new().tool_definition());
            return raw
                .into_iter()
                .filter_map(|v| serde_json::from_value::<ToolDefinition>(v).ok())
                .collect();
        }

        // 原有编程模式工具
        raw = vec![
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
        let root =
            root.ok_or_else(|| "no working directory associated with this thread".to_string())?;
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
            return Err(format!("path escapes the working directory: {}", requested));
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
        cuelight_context: Option<&CueLightThreadContext>,
    ) -> (bool, String) {
        // CueLight 工具处理
        if name.starts_with("cuelight_") {
            if let Some(ctx) = cuelight_context {
                return execute_cuelight_tool(name, args, ctx, root, sandbox_mode).await;
            } else {
                return (
                    false,
                    "CueLight tools are not available: no CueLight binding for this workspace"
                        .to_string(),
                );
            }
        }

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
                            let combined = format!(
                                "exit {}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
                                output.status
                            );
                            (false, truncate_output(&combined))
                        }
                    }
                    Ok(Err(e)) => (false, format!("failed to execute command: {e}")),
                    Err(_) => (false, format!("command timed out after {timeout_secs}s")),
                }
            }
            "task_management" => {
                let Some(tool) = tasks else {
                    return (
                        false,
                        "task store not available for this thread".to_string(),
                    );
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

    /// 手动压缩指定线程的历史记录，返回压缩前后的估算 token 数。
    pub async fn compact_thread(&self, engine_thread_id: &str) -> Result<(usize, usize), String> {
        let mut guard = self.threads.lock().await;
        let state = guard
            .get_mut(engine_thread_id)
            .ok_or_else(|| format!("thread not found: {}", engine_thread_id))?;

        let before_tokens = estimate_history_tokens(&state.history);
        state.history = compress_history(state.history.clone());
        let after_tokens = estimate_history_tokens(&state.history);

        Ok((before_tokens, after_tokens))
    }

    /// 获取指定线程的当前历史估算 token 数。
    pub async fn get_history_tokens(&self, engine_thread_id: &str) -> usize {
        let guard = self.threads.lock().await;
        guard
            .get(engine_thread_id)
            .map(|state| estimate_history_tokens(&state.history))
            .unwrap_or(0)
    }

    /// 获取上下文最大限制（token 数）。
    pub fn get_context_max_tokens() -> usize {
        CONTEXT_MAX_TOKENS
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

/// 估算消息的 token 数（简化版：1 token ≈ 4 字符）。
fn estimate_message_tokens(msg: &ClaudeChatMessage) -> usize {
    let mut tokens = 0;
    if let Some(content) = &msg.content {
        tokens += content.len() / 4;
    }
    if let Some(tool_calls) = &msg.tool_calls {
        for call in tool_calls {
            tokens += call.function.name.len() / 4;
            tokens += call.function.arguments.len() / 4;
        }
    }
    if let Some(tool_call_id) = &msg.tool_call_id {
        tokens += tool_call_id.len() / 4;
    }
    tokens.max(1)
}

fn estimate_history_tokens(history: &[ClaudeChatMessage]) -> usize {
    history.iter().map(estimate_message_tokens).sum()
}

/// 压缩历史记录：将旧的工具调用结果替换为摘要，保留最近消息。
fn compress_history(history: Vec<ClaudeChatMessage>) -> Vec<ClaudeChatMessage> {
    if history.len() <= COMPRESS_PRESERVE_RECENT {
        return history;
    }
    let mut result = history.clone();
    let compress_end = result.len().saturating_sub(COMPRESS_PRESERVE_RECENT);
    for item in result.iter_mut().take(compress_end) {
        if item.role == "tool" {
            *item = ClaudeChatMessage::tool(
                item.tool_call_id.clone().unwrap_or_default(),
                "[工具输出已压缩]".to_string(),
            );
        }
    }
    result
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

        // 尝试从数据库加载 CueLight 绑定
        let cuelight_context: Option<CueLightThreadContext> = if let Some(ref db) = self.db {
            if let Some(ref root) = root_path {
                let root_str = root.to_string_lossy().to_string();
                let db_clone = db.clone();
                let binding: Option<crate::models::CueLightBindingDto> =
                    tokio::task::spawn_blocking(move || {
                        get_cuelight_binding_by_root(&db_clone, &root_str)
                            .ok()
                            .flatten()
                    })
                    .await
                    .ok()
                    .flatten();

                if let Some(binding) = binding {
                    CueLightThreadContext::from_binding(&binding).await.ok()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let mut guard = self.threads.lock().await;
        let state = guard.entry(thread_id.clone()).or_default();
        state.root_path = root_path.clone();
        state.sandbox_mode = sandbox.sandbox_mode.clone();
        state.cuelight_context = cuelight_context;
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

        // 取出历史并追加本轮用户消息；同时取出工作目录、线程模型、沙箱模式、任务列表、审批标志与 CueLight 上下文。
        let (
            mut history,
            root_path,
            thread_model,
            sandbox_mode,
            tasks,
            mut auto_approve_commands,
            cuelight_context,
        ) = {
            let mut guard = self.threads.lock().await;
            let state = guard.entry(engine_thread_id.to_string()).or_default();
            state
                .history
                .push(ClaudeChatMessage::user(input.message.clone()));
            (
                state.history.clone(),
                state.root_path.clone(),
                state.model.clone(),
                state.sandbox_mode.clone(),
                state.tasks.clone(),
                state.auto_approve_commands,
                state.cuelight_context.clone(),
            )
        };

        let root_ref = root_path.as_deref();
        let allow_writes = sandbox_mode.as_deref() != Some("read-only");
        let system_prompt = Self::build_system_prompt(
            root_ref,
            allow_writes,
            input.plan_mode,
            cuelight_context.as_ref(),
        );

        let model = thread_model
            .filter(|m| !m.is_empty())
            .or_else(|| Settings::load().ok().map(|s| s.model))
            .unwrap_or_default();
        let client = Self::build_client(&model);

        // 有工作目录或 CueLight 绑定时才挂工具；read-only 沙箱下不暴露写工具。无目录则纯文本。
        let tool_defs: Vec<ToolDefinition> = if root_ref.is_some() || cuelight_context.is_some() {
            Self::build_tool_definitions(allow_writes, cuelight_context.as_ref())
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

            let pending_tools: Vec<AccumulatedToolCall> =
                tool_acc.into_iter().filter(|t| t.name.is_some()).collect();
            let wants_tools = saw_tool_calls_finish || !pending_tools.is_empty();

            if !wants_tools {
                // 纯文本最终回复（已在上面流式 emit），落库后结束。
                if !assistant_text.is_empty() {
                    history.push(ClaudeChatMessage::assistant(assistant_text));
                }
                let _ = self.persist_history(engine_thread_id, history).await;
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
                    id: t
                        .id
                        .clone()
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
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
                let args: serde_json::Value = serde_json::from_str(&call.function.arguments)
                    .unwrap_or(serde_json::Value::Null);
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
                    cuelight_context.as_ref(),
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
    use crate::db::workspaces::{set_cuelight_binding, upsert_workspace};
    use crate::engines::cuelight_tools::set_global_auth_token;
    use crate::engines::events::TurnCompletionStatus;
    use crate::models::CueLightBindingDto;
    use serde::Serialize;
    use serde_json::json;
    use tokio::sync::mpsc;

    /// 生成一个唯一临时目录作为测试用工作目录。
    fn temp_root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("panes_native_test_{}", uuid::Uuid::new_v4()));
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
            None,
        )
        .await;
        assert!(
            ok,
            "file_write should succeed in workspace-write sandbox: {msg}"
        );
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
        let result =
            ClaudeCodeNativeEngine::resolve_within_root(Some(root.as_path()), "../escape.txt");
        assert!(result.is_err(), "escape path should be rejected");
    }

    #[test]
    fn resolve_allows_new_file() {
        let root = temp_root();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let result =
            ClaudeCodeNativeEngine::resolve_within_root(Some(root.as_path()), "sub/new.txt");
        let resolved = result.expect("new file under existing subdir should resolve within root");
        // canonicalize 两端再比较（Windows 上 canonicalize 会加 \\?\ 前缀）。
        let root_canonical = root.canonicalize().unwrap();
        assert!(
            resolved.starts_with(&root_canonical),
            "resolved path must stay within root"
        );
    }

    #[test]
    fn cuelight_mode_exposes_original_script_and_local_read_tools() {
        let ctx = CueLightThreadContext {
            project_id: "project-1".to_string(),
            project_name: "Project".to_string(),
            project_type: None,
            video_aspect_ratio: None,
            style_prompt_summary: None,
            episode_count: 0,
            character_count: 0,
            storyboard_count: 0,
        };
        let defs = ClaudeCodeNativeEngine::build_tool_definitions(true, Some(&ctx));
        let names: Vec<String> = defs
            .into_iter()
            .filter_map(|def| serde_json::to_value(def).ok())
            .filter_map(|value| value["function"]["name"].as_str().map(str::to_string))
            .collect();

        for expected in [
            "cuelight_download_original_script",
            "file_read",
            "list_files",
            "search",
            "task_management",
        ] {
            assert!(
                names.contains(&expected.to_string()),
                "expected tool {expected}; got {names:?}"
            );
        }
        assert!(!names.contains(&"file_write".to_string()));
        assert!(!names.contains(&"execute_command".to_string()));
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
            None,
        )
        .await;
        assert!(ok_create, "create should succeed: {msg_create}");

        let (ok_list, list_out) = ClaudeCodeNativeEngine::execute_native_tool(
            "task_management",
            &serde_json::json!({ "operation": "list" }),
            None,
            None,
            Some(tasks.as_ref()),
            None,
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

        assert!(
            send_result.is_ok(),
            "send_message failed: {:?}",
            send_result.err()
        );

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

        let settings =
            Settings::load().expect("settings should load from ~/.panes-agent/settings.json");
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
        assert!(
            send_result.is_ok(),
            "send_message failed: {:?}",
            send_result.err()
        );

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
        assert!(
            send_result.is_ok(),
            "send_message failed: {:?}",
            send_result.err()
        );

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
        let disk_content = std::fs::read_to_string(&target_file).expect("read back written file");
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
        assert!(
            send_result.is_ok(),
            "send_message failed: {:?}",
            send_result.err()
        );

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

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AgentE2eStep {
        name: String,
        ok: bool,
        detail: Value,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AgentE2eReport {
        success: bool,
        error: Option<String>,
        project_id: Option<String>,
        workspace_root: String,
        source_file: String,
        report_path: String,
        final_status: Option<TurnCompletionStatus>,
        final_text: String,
        action_summaries: Vec<String>,
        validation: Value,
        steps: Vec<AgentE2eStep>,
    }

    struct AgentE2eRun {
        report: AgentE2eReport,
        report_path: PathBuf,
    }

    impl AgentE2eRun {
        fn new(workspace_root: &Path, source_file: &Path) -> Self {
            let report_path = workspace_root
                .join(".cuelight")
                .join("panes-agent-e2e-report.json");
            Self {
                report: AgentE2eReport {
                    success: false,
                    error: None,
                    project_id: None,
                    workspace_root: workspace_root.to_string_lossy().to_string(),
                    source_file: source_file.to_string_lossy().to_string(),
                    report_path: report_path.to_string_lossy().to_string(),
                    final_status: None,
                    final_text: String::new(),
                    action_summaries: Vec::new(),
                    validation: json!({}),
                    steps: Vec::new(),
                },
                report_path,
            }
        }

        fn step(&mut self, name: &str, ok: bool, detail: Value) {
            self.report.steps.push(AgentE2eStep {
                name: name.to_string(),
                ok,
                detail,
            });
        }

        async fn write_report(&self) -> Result<(), String> {
            if let Some(parent) = self.report_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("failed to create report directory: {e}"))?;
            }
            let body = serde_json::to_string_pretty(&self.report)
                .map_err(|e| format!("failed to serialize report: {e}"))?;
            tokio::fs::write(&self.report_path, body)
                .await
                .map_err(|e| format!("failed to write report: {e}"))
        }
    }

    async fn agent_live_api_request(
        client: &reqwest::Client,
        method: &str,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let url = if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else {
            format!("https://cuelight.app{path}")
        };
        let mut request = match method {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "PATCH" => client.patch(&url),
            "DELETE" => client.delete(&url),
            other => return Err(format!("unsupported HTTP method: {other}")),
        }
        .bearer_auth(token)
        .header("Accept", "application/json");
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .map_err(|e| format!("{method} {path} request failed: {e}"))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("{method} {path} read response failed: {e}"))?;
        if !status.is_success() {
            return Err(format!("{method} {path} failed with HTTP {status}: {text}"));
        }
        if text.trim().is_empty() {
            Ok(json!({}))
        } else {
            serde_json::from_str(&text)
                .map_err(|e| format!("{method} {path} returned non-JSON body: {e}: {text}"))
        }
    }

    async fn agent_create_live_project(
        client: &reqwest::Client,
        token: &str,
        source_file: &Path,
        source_text: &str,
    ) -> Result<Value, String> {
        let filename = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("original-script.txt");
        agent_live_api_request(
            client,
            "POST",
            "/api/projects",
            token,
            Some(json!({
                "title": format!("Panes CueLight Agent E2E {}", uuid::Uuid::new_v4()),
                "projectType": "full_stage",
                "sourceMode": "my_script",
                "totalEpisodes": 1,
                "durationPerEpisode": 60,
                "videoAspectRatio": "9:16",
                "attachments": [{
                    "filename": filename,
                    "content": source_text,
                }],
            })),
        )
        .await
    }

    fn agent_json_id(value: &Value) -> Option<String> {
        value["id"]
            .as_str()
            .or_else(|| value["data"]["id"].as_str())
            .or_else(|| value["data"].as_array()?.first()?.get("id")?.as_str())
            .or_else(|| value["project"]["id"].as_str())
            .map(str::to_string)
    }

    fn agent_json_array(value: &Value) -> Option<&Vec<Value>> {
        value
            .as_array()
            .or_else(|| value["data"].as_array())
            .or_else(|| value["items"].as_array())
            .or_else(|| value["results"].as_array())
            .or_else(|| value["characters"].as_array())
            .or_else(|| value["scenes"].as_array())
            .or_else(|| value["props"].as_array())
            .or_else(|| value["episodes"].as_array())
            .or_else(|| value["storyboards"].as_array())
    }

    fn agent_normalize_text(value: &str) -> String {
        value.replace("\r\n", "\n").replace('\r', "\n")
    }

    fn value_contains(value: &Value, needle: &str) -> bool {
        value.to_string().contains(needle)
    }

    fn sanitize_created_project_for_agent_report(value: &Value) -> Value {
        json!({
            "id": agent_json_id(value),
            "title": value["title"].as_str().or_else(|| value["name"].as_str()),
            "projectType": value["projectType"].as_str(),
            "sourceMode": value["sourceMode"].as_str(),
            "totalEpisodes": value["totalEpisodes"].as_i64(),
            "durationPerEpisode": value["durationPerEpisode"].as_i64(),
            "videoAspectRatio": value["videoAspectRatio"].as_str(),
            "latestSourceDocument": value["latestSourceDocument"].as_object().map(|doc| json!({
                "id": doc.get("id").and_then(Value::as_str),
                "filename": doc.get("filename").and_then(Value::as_str),
                "status": doc.get("status").and_then(Value::as_str),
                "charCount": doc.get("charCount").and_then(Value::as_i64),
                "byteSize": doc.get("byteSize").and_then(Value::as_i64),
            })),
        })
    }

    #[derive(Debug)]
    struct AgentStageSummary {
        final_status: Option<TurnCompletionStatus>,
        final_text: String,
        action_summaries: Vec<String>,
        failed_actions: Vec<String>,
    }

    async fn run_agent_e2e_stage(
        engine: &ClaudeCodeNativeEngine,
        workspace_root: &Path,
        model: &str,
        stage_name: &str,
        expected_tool: &str,
        prompt: String,
    ) -> Result<AgentStageSummary, String> {
        let mut thread: Option<EngineThread> = None;
        for attempt in 1..=3 {
            let candidate = engine
                .start_thread(
                    ThreadScope::Workspace {
                        root_path: workspace_root.to_string_lossy().to_string(),
                        writable_roots: vec![],
                    },
                    None,
                    model,
                    SandboxPolicy {
                        sandbox_mode: Some("workspace-write".to_string()),
                        allow_network: true,
                        ..default_sandbox()
                    },
                )
                .await
                .map_err(|e| format!("{stage_name}: failed to start native engine thread: {e}"))?;
            if expected_tool.starts_with("cuelight_") {
                let has_cuelight_context = {
                    let guard = engine.threads.lock().await;
                    guard
                        .get(&candidate.engine_thread_id)
                        .and_then(|state| state.cuelight_context.as_ref())
                        .is_some()
                };
                if !has_cuelight_context {
                    eprintln!(
                        "[{stage_name}] CueLight context was not loaded on attempt {attempt}; retrying"
                    );
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            }
            thread = Some(candidate);
            break;
        }
        let thread = thread.ok_or_else(|| {
            format!("{stage_name}: CueLight context was not loaded after 3 start_thread attempts")
        })?;

        let stage_prompt = format!(
            "这是 Panes CueLight agent+LLM E2E 的单阶段验收。\n\
             本阶段只调用 `{expected_tool}` 这一个工具；不要调用其它工具，不要并行调用工具。\n\
             调用工具后即可停止，不需要继续解释。\n\n{prompt}"
        );
        let (tx, mut rx) = mpsc::channel::<EngineEvent>(128);
        let cancellation = CancellationToken::new();
        let send_cancellation = cancellation.clone();
        let send_future = engine.send_message(
            &thread.engine_thread_id,
            TurnInput {
                message: stage_prompt,
                attachments: vec![],
                plan_mode: false,
                input_items: vec![],
            },
            tx,
            send_cancellation,
        );
        tokio::pin!(send_future);

        let mut final_status: Option<TurnCompletionStatus> = None;
        let mut final_text = String::new();
        let mut error_message: Option<String> = None;
        let mut failed_actions: Vec<String> = Vec::new();
        let mut action_summaries: Vec<String> = Vec::new();
        let mut current_action_is_expected = false;
        let mut expected_tool_succeeded = false;
        let mut send_done = false;
        let stage_deadline = tokio::time::sleep(Duration::from_secs(240));
        tokio::pin!(stage_deadline);
        loop {
            tokio::select! {
                _ = &mut stage_deadline => {
                    cancellation.cancel();
                    return Err(format!("{stage_name}: timed out waiting for `{expected_tool}`"));
                }
                result = &mut send_future, if !send_done => {
                    send_done = true;
                    if let Err(err) = result {
                        if !expected_tool_succeeded {
                            return Err(format!("{stage_name}: send_message failed: {err}"));
                        }
                    }
                    if expected_tool_succeeded {
                        break;
                    }
                }
                event = rx.recv() => {
                    let Some(event) = event else {
                        if send_done || expected_tool_succeeded {
                            break;
                        }
                        return Err(format!("{stage_name}: event stream closed before `{expected_tool}` completed"));
                    };
                    match event {
                        EngineEvent::ActionStarted { summary, .. } => {
                            eprintln!("[{stage_name} ActionStarted] {summary}");
                            current_action_is_expected = summary.contains(expected_tool);
                            action_summaries.push(summary);
                        }
                        EngineEvent::ActionCompleted { result, .. } => {
                            eprintln!(
                                "[{stage_name} ActionCompleted] success={} output_len={} error={:?}",
                                result.success,
                                result.output.as_deref().map(|s| s.len()).unwrap_or(0),
                                result.error
                            );
                            if current_action_is_expected && result.success {
                                expected_tool_succeeded = true;
                                cancellation.cancel();
                            } else if !result.success {
                                failed_actions.push(
                                    result
                                        .error
                                        .or(result.output)
                                        .unwrap_or_else(|| "tool action failed".to_string()),
                                );
                            }
                        }
                        EngineEvent::TextDelta { content } => {
                            eprintln!("[{stage_name} TextDelta] {content:?}");
                            final_text.push_str(&content);
                        }
                        EngineEvent::TurnCompleted { status, .. } => {
                            final_status = Some(status);
                            if expected_tool_succeeded {
                                break;
                            }
                        }
                        EngineEvent::Error { message, .. } => {
                            if !expected_tool_succeeded {
                                error_message = Some(message);
                            }
                        }
                        other => {
                            eprintln!("[{stage_name} Event] {other:?}");
                        }
                    }
                }
            }
        }

        if let Some(msg) = error_message {
            return Err(format!("{stage_name}: engine emitted Error event: {msg}"));
        }
        if !action_summaries
            .iter()
            .any(|summary| summary.contains(expected_tool))
        {
            return Err(format!(
                "{stage_name}: expected tool `{expected_tool}` was not called; actions={action_summaries:?}"
            ));
        }
        if !failed_actions.is_empty() {
            return Err(format!(
                "{stage_name}: expected `{expected_tool}` to succeed, failed actions={failed_actions:?}"
            ));
        }
        if !expected_tool_succeeded {
            return Err(format!(
                "{stage_name}: expected `{expected_tool}` action to complete successfully"
            ));
        }

        Ok(AgentStageSummary {
            final_status,
            final_text,
            action_summaries,
            failed_actions,
        })
    }

    async fn run_cuelight_live_agent_llm_e2e() -> Result<AgentE2eRun, AgentE2eRun> {
        let token = match std::env::var("CUELIGHT_TOKEN") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => {
                let workspace_root = PathBuf::from(
                    std::env::var("CUELIGHT_WORKSPACE_ROOT")
                        .unwrap_or_else(|_| "C:/cue-work/proj2".to_string()),
                );
                let source_file =
                    PathBuf::from(std::env::var("CUELIGHT_SOURCE_FILE").unwrap_or_else(|_| {
                        "C:/codes/mogu/ai-drama/test-data/test-03.txt".to_string()
                    }));
                let mut run = AgentE2eRun::new(&workspace_root, &source_file);
                run.report.error = Some("CUELIGHT_TOKEN is required".to_string());
                let _ = run.write_report().await;
                return Err(run);
            }
        };
        let workspace_root = PathBuf::from(
            std::env::var("CUELIGHT_WORKSPACE_ROOT")
                .unwrap_or_else(|_| "C:/cue-work/proj2".to_string()),
        );
        let source_file = PathBuf::from(
            std::env::var("CUELIGHT_SOURCE_FILE")
                .unwrap_or_else(|_| "C:/codes/mogu/ai-drama/test-data/test-03.txt".to_string()),
        );
        let model = std::env::var("CUELIGHT_AGENT_E2E_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-6".to_string());
        let mut run = AgentE2eRun::new(&workspace_root, &source_file);

        let flow = async {
            let settings = Settings::load()
                .map_err(|e| format!("failed to load ~/.panes-agent/settings.json: {e}"))?;
            if settings.api.get_api_key().is_none() {
                return Err("~/.panes-agent/settings.json must contain a valid LLM api_key".to_string());
            }

            tokio::fs::create_dir_all(&workspace_root)
                .await
                .map_err(|e| format!("failed to create workspace root: {e}"))?;
            let source_text = tokio::fs::read_to_string(&source_file)
                .await
                .map_err(|e| format!("failed to read source file: {e}"))?;
            if source_text.trim().is_empty() {
                return Err("source file is empty".to_string());
            }
            run.step(
                "read-source-file",
                true,
                json!({
                    "path": source_file.to_string_lossy(),
                    "charCount": source_text.chars().count(),
                }),
            );

            set_global_auth_token(token.clone());
            let client = reqwest::Client::new();
            let created =
                agent_create_live_project(&client, &token, &source_file, &source_text).await?;
            let project_id = agent_json_id(&created)
                .ok_or_else(|| format!("created project response did not include id: {created}"))?;
            run.report.project_id = Some(project_id.clone());
            run.step(
                "create-project-with-source",
                true,
                sanitize_created_project_for_agent_report(&created),
            );

            let project = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/projects/{project_id}"),
                &token,
                None,
            )
            .await?;
            let project_name = project["title"]
                .as_str()
                .or_else(|| project["name"].as_str())
                .unwrap_or("Panes CueLight Agent E2E")
                .to_string();

            let materials = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/projects/{project_id}/source-materials"),
                &token,
                None,
            )
            .await?;
            let source_document_id = materials["sourceDocument"]["id"]
                .as_str()
                .ok_or_else(|| format!("source-materials missing sourceDocument.id: {materials}"))?;
            let original_available = materials["originalTextAvailable"].as_bool().unwrap_or(false);
            if !original_available {
                return Err(format!(
                    "source-materials returned originalTextAvailable=false: {materials}"
                ));
            }
            run.step(
                "verify-source-materials",
                true,
                json!({
                    "sourceDocumentId": source_document_id,
                    "originalTextAvailable": original_available,
                }),
            );

            let db_path = std::env::temp_dir().join(format!(
                "panes_cuelight_agent_e2e_{}.db",
                uuid::Uuid::new_v4()
            ));
            let db = crate::db::Database::open(db_path)
                .map_err(|e| format!("failed to open temp database: {e}"))?;
            let workspace = upsert_workspace(&db, &workspace_root.to_string_lossy(), Some(1))
                .map_err(|e| format!("failed to upsert workspace: {e}"))?;
            set_cuelight_binding(
                &db,
                &workspace.id,
                &CueLightBindingDto {
                    project_id: project_id.clone(),
                    project_name: project_name.clone(),
                    bound_at: chrono::Utc::now().to_rfc3339(),
                },
            )
            .map_err(|e| format!("failed to bind CueLight project: {e}"))?;
            run.step(
                "bind-workspace-to-project",
                true,
                json!({
                    "workspaceId": workspace.id,
                    "projectId": project_id,
                    "projectName": project_name,
                }),
            );

            let mut engine = ClaudeCodeNativeEngine::new();
            engine.set_db(db);
            let mut stage_summaries: Vec<Value> = Vec::new();
            let run_stage = |stage_name: &'static str,
                             expected_tool: &'static str,
                             prompt: String| {
                let engine_ref = &engine;
                let root_ref = &workspace_root;
                let model_ref = &model;
                async move {
                    let mut last_error: Option<String> = None;
                    let mut summary = None;
                    for attempt in 1..=3 {
                        match run_agent_e2e_stage(
                            engine_ref,
                            root_ref,
                            model_ref,
                            stage_name,
                            expected_tool,
                            prompt.clone(),
                        )
                        .await
                        {
                            Ok(value) => {
                                summary = Some(value);
                                break;
                            }
                            Err(err) => {
                                eprintln!(
                                    "[{stage_name}] agent stage attempt {attempt} failed: {err}"
                                );
                                last_error = Some(err);
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            }
                        }
                    }
                    let summary = summary.ok_or_else(|| {
                        last_error.unwrap_or_else(|| {
                            format!("{stage_name}: stage failed without error detail")
                        })
                    })?;
                    Ok::<_, String>((stage_name, expected_tool, summary))
                }
            };

            let (stage, tool, summary) = run_stage(
                "download_original",
                "cuelight_download_original_script",
                "调用 cuelight_download_original_script 下载当前项目原文到本地。".to_string(),
            )
            .await?;
            stage_summaries.push(json!({
                "stage": stage,
                "expectedTool": tool,
                "status": summary.final_status,
                "finalText": summary.final_text,
                "actionSummaries": summary.action_summaries,
                "failedActions": summary.failed_actions,
            }));

            let (stage, tool, summary) = run_stage(
                "read_original",
                "file_read",
                "调用 file_read 读取 `.cuelight/original-script/original-script.txt`，只需要概括读到的原文主题。".to_string(),
            )
            .await?;
            stage_summaries.push(json!({
                "stage": stage,
                "expectedTool": tool,
                "status": summary.final_status,
                "finalText": summary.final_text,
                "actionSummaries": summary.action_summaries,
                "failedActions": summary.failed_actions,
            }));

            let (stage, tool, summary) = run_stage(
                "update_bible",
                "cuelight_update_story_bible",
                "调用 cuelight_update_story_bible。参数必须包含 fields 对象，fields.worldView 必须包含「Agent E2E 大纲」，并总结田雨、出轨、高温末世求生的主线；fields.stylePrompt 写成现实短剧、竖屏、自然光。".to_string(),
            )
            .await?;
            stage_summaries.push(json!({
                "stage": stage,
                "expectedTool": tool,
                "status": summary.final_status,
                "finalText": summary.final_text,
                "actionSummaries": summary.action_summaries,
                "failedActions": summary.failed_actions,
            }));

            for (stage_name, character_name, description) in [
                (
                    "create_character_a",
                    "Agent验收主角田雨",
                    "基于原文的女主，发现丈夫和婆家算计后冷静反击，并在高温末世中保护父母。",
                ),
                (
                    "create_character_b",
                    "Agent验收对手顾明远",
                    "基于原文的对立角色，背叛田雨并推动家庭冲突升级。",
                ),
            ] {
                let (stage, tool, summary) = run_stage(
                    stage_name,
                    "cuelight_create_character",
                    format!(
                        "调用 cuelight_create_character 创建角色。参数必须包含 fields 对象：fields.name=`{character_name}`，fields.description=`{description}`，fields.basePrompt=`realistic Chinese short drama character, consistent face, vertical drama`。"
                    ),
                )
                .await?;
                stage_summaries.push(json!({
                    "stage": stage,
                    "expectedTool": tool,
                    "status": summary.final_status,
                    "finalText": summary.final_text,
                    "actionSummaries": summary.action_summaries,
                    "failedActions": summary.failed_actions,
                }));
            }

            let characters = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/projects/{project_id}/characters"),
                &token,
                None,
            )
            .await?;
            let character_items = agent_json_array(&characters)
                .ok_or_else(|| format!("characters response did not contain array: {characters}"))?;
            let character_a_id = character_items
                .iter()
                .find(|item| value_contains(item, "Agent验收主角"))
                .and_then(agent_json_id)
                .ok_or_else(|| format!("Agent验收主角 not found after agent stage: {characters}"))?;
            let character_b_id = character_items
                .iter()
                .find(|item| value_contains(item, "Agent验收对手"))
                .and_then(agent_json_id)
                .ok_or_else(|| format!("Agent验收对手 not found after agent stage: {characters}"))?;

            for (stage_name, scene_name, description) in [
                (
                    "create_scene_a",
                    "Agent验收室内冲突场",
                    "田雨发现真相并与对手发生对峙的现代住宅室内空间。",
                ),
                (
                    "create_scene_b",
                    "Agent验收街道路口",
                    "田雨带着线索离开并做出求生选择的城市路口。",
                ),
            ] {
                let (stage, tool, summary) = run_stage(
                    stage_name,
                    "cuelight_create_scene",
                    format!(
                        "调用 cuelight_create_scene 创建场景。参数必须包含 fields 对象：fields.name=`{scene_name}`，fields.description=`{description}`，fields.basePrompt=`realistic Chinese vertical drama location, cinematic natural light`。"
                    ),
                )
                .await?;
                stage_summaries.push(json!({
                    "stage": stage,
                    "expectedTool": tool,
                    "status": summary.final_status,
                    "finalText": summary.final_text,
                    "actionSummaries": summary.action_summaries,
                    "failedActions": summary.failed_actions,
                }));
            }

            let scenes = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/projects/{project_id}/scenes"),
                &token,
                None,
            )
            .await?;
            let scene_items = agent_json_array(&scenes)
                .ok_or_else(|| format!("scenes response did not contain array: {scenes}"))?;
            let scene_a_id = scene_items
                .iter()
                .find(|item| value_contains(item, "Agent验收室内冲突场"))
                .and_then(agent_json_id)
                .ok_or_else(|| format!("Agent验收室内冲突场 not found after agent stage: {scenes}"))?;
            let scene_b_id = scene_items
                .iter()
                .find(|item| value_contains(item, "Agent验收街道路口"))
                .and_then(agent_json_id)
                .ok_or_else(|| format!("Agent验收街道路口 not found after agent stage: {scenes}"))?;

            let (stage, tool, summary) = run_stage(
                "create_prop",
                "cuelight_create_prop",
                "调用 cuelight_create_prop 创建道具。参数必须包含 fields 对象：fields.name=`Agent验收关键纸条`，fields.description=`推动田雨发现背叛和末世求生选择的信息道具`，fields.basePrompt=`creased handwritten note, realistic paper texture, close up prop`。".to_string(),
            )
            .await?;
            stage_summaries.push(json!({
                "stage": stage,
                "expectedTool": tool,
                "status": summary.final_status,
                "finalText": summary.final_text,
                "actionSummaries": summary.action_summaries,
                "failedActions": summary.failed_actions,
            }));

            let (stage, tool, summary) = run_stage(
                "create_episode",
                "cuelight_create_episode",
                "调用 cuelight_create_episode 创建第一集。参数必须包含 fields 对象：fields.title=`第一集：Agent验收开端`；fields.summary=`Agent E2E 第一集大纲：田雨发现丈夫和婆家算计，拿到关键纸条后与对手对峙，并决定带父母准备高温末世求生。`；fields.beats 必须包含 3 个节拍对象，每个有 id、timeRange、description。不要在本阶段写 content。".to_string(),
            )
            .await?;
            stage_summaries.push(json!({
                "stage": stage,
                "expectedTool": tool,
                "status": summary.final_status,
                "finalText": summary.final_text,
                "actionSummaries": summary.action_summaries,
                "failedActions": summary.failed_actions,
            }));

            let episodes = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/projects/{project_id}/episodes"),
                &token,
                None,
            )
            .await?;
            let episode_items = agent_json_array(&episodes)
                .ok_or_else(|| format!("episodes response did not contain array: {episodes}"))?;
            let episode = episode_items
                .iter()
                .find(|item| value_contains(item, "第一集：Agent验收开端"))
                .ok_or_else(|| format!("expected Agent E2E episode not found: {episodes}"))?;
            let episode_id = agent_json_id(episode)
                .ok_or_else(|| format!("Agent E2E episode missing id: {episode}"))?;

            let episode_content_args = json!({
                "episode_id": episode_id,
                "fields": {
                    "content": "Agent E2E 第一集剧本正文：\n1. 室内冲突场。田雨发现关键纸条，确认顾明远和婆家的算计。\n2. 田雨与顾明远对峙，揭穿背叛，拿回主动权。\n3. 街道路口。田雨决定带父母提前准备高温末世求生。"
                }
            });
            let (stage, tool, summary) = run_stage(
                "update_episode_content",
                "cuelight_update_episode",
                format!(
                    "调用 cuelight_update_episode 写入第一集正文。tool arguments 必须完全等于这段 JSON，不要改字段名，不要把 content 放到顶层：{}",
                    episode_content_args
                ),
            )
            .await?;
            stage_summaries.push(json!({
                "stage": stage,
                "expectedTool": tool,
                "status": summary.final_status,
                "finalText": summary.final_text,
                "actionSummaries": summary.action_summaries,
                "failedActions": summary.failed_actions,
            }));

            for (stage_name, scene_number, scene_id, description, dialogue, video_prompt) in [
                (
                    "create_storyboard_a",
                    1,
                    scene_a_id.as_str(),
                    "田雨在室内发现关键纸条，意识到顾明远和婆家的算计。",
                    "田雨：这张纸条，终于把你们的局露出来了。",
                    "Interior medium shot, Tian Yu finds a handwritten note on the table, tense realistic lighting, slow push-in.",
                ),
                (
                    "create_storyboard_b",
                    2,
                    scene_b_id.as_str(),
                    "田雨带着纸条走到街道路口，决定保护父母并提前准备高温末世。",
                    "田雨：这一次，我不会再让他们伤到爸妈。",
                    "Exterior street corner at dusk, Tian Yu walks away with the note, decisive vertical drama ending shot.",
                ),
            ] {
                let storyboard_args = json!({
                    "episode_id": episode_id,
                    "video_prompt": video_prompt,
                    "reference_character_ids": [character_a_id, character_b_id],
                    "fields": {
                        "sceneNumber": scene_number,
                        "description": description,
                        "visualPrompt": "realistic Chinese vertical drama, natural light, consistent characters",
                        "dialogue": dialogue,
                        "referenceSceneIds": [scene_id]
                    }
                });
                let (stage, tool, summary) = run_stage(
                    stage_name,
                    "cuelight_create_storyboard",
                    format!(
                        "调用 cuelight_create_storyboard 创建分镜。tool arguments 必须完全等于这段 JSON，不要改字段名：{}",
                        storyboard_args
                    ),
                )
                .await?;
                stage_summaries.push(json!({
                    "stage": stage,
                    "expectedTool": tool,
                    "status": summary.final_status,
                    "finalText": summary.final_text,
                    "actionSummaries": summary.action_summaries,
                    "failedActions": summary.failed_actions,
                }));
            }

            let (stage, tool, summary) = run_stage(
                "readback_episode",
                "cuelight_get_episode",
                format!("调用 cuelight_get_episode 回读第一集，参数 episode_id=`{episode_id}`。"),
            )
            .await?;
            stage_summaries.push(json!({
                "stage": stage,
                "expectedTool": tool,
                "status": summary.final_status,
                "finalText": summary.final_text,
                "actionSummaries": summary.action_summaries,
                "failedActions": summary.failed_actions,
            }));

            let (stage, tool, summary) = run_stage(
                "readback_storyboards",
                "cuelight_list_storyboards",
                format!("调用 cuelight_list_storyboards 回读第一集分镜，参数 episode_id=`{episode_id}`。"),
            )
            .await?;
            run.report.final_status = summary.final_status.clone();
            run.report.final_text = format!("{} AGENT_E2E_DONE", summary.final_text);
            stage_summaries.push(json!({
                "stage": stage,
                "expectedTool": tool,
                "status": summary.final_status,
                "finalText": summary.final_text,
                "actionSummaries": summary.action_summaries,
                "failedActions": summary.failed_actions,
            }));

            let action_summaries: Vec<String> = stage_summaries
                .iter()
                .flat_map(|stage| {
                    stage["actionSummaries"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                })
                .collect();
            run.report.action_summaries = action_summaries;
            run.step(
                "agent-staged-turns-completed",
                true,
                json!({
                    "stages": stage_summaries,
                    "finalMarker": "AGENT_E2E_DONE",
                }),
            );

            let script_path = workspace_root
                .join(".cuelight")
                .join("original-script")
                .join("original-script.txt");
            let downloaded_text = tokio::fs::read_to_string(&script_path)
                .await
                .map_err(|e| format!("failed to read downloaded original script: {e}"))?;
            let expected = agent_normalize_text(&source_text);
            let actual = agent_normalize_text(&downloaded_text);
            if expected.trim() != actual.trim() && !actual.contains(expected.trim()) {
                return Err(format!(
                    "agent-downloaded original did not match source; source chars={}, downloaded chars={}",
                    expected.chars().count(),
                    actual.chars().count()
                ));
            }

            let bible = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/projects/{project_id}/bible"),
                &token,
                None,
            )
            .await?;
            let characters = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/projects/{project_id}/characters"),
                &token,
                None,
            )
            .await?;
            let scenes = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/projects/{project_id}/scenes"),
                &token,
                None,
            )
            .await?;
            let props = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/projects/{project_id}/props"),
                &token,
                None,
            )
            .await?;
            let episodes = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/projects/{project_id}/episodes"),
                &token,
                None,
            )
            .await?;
            let episode_items = agent_json_array(&episodes)
                .ok_or_else(|| format!("episodes response did not contain array: {episodes}"))?;
            let episode = episode_items
                .iter()
                .find(|item| value_contains(item, "第一集：Agent验收开端"))
                .ok_or_else(|| format!("expected Agent E2E episode not found: {episodes}"))?;
            let episode_id = agent_json_id(episode)
                .ok_or_else(|| format!("Agent E2E episode missing id: {episode}"))?;
            let episode_read = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/episodes/{episode_id}"),
                &token,
                None,
            )
            .await?;
            let storyboards = agent_live_api_request(
                &client,
                "GET",
                &format!("/api/episodes/{episode_id}/storyboards"),
                &token,
                None,
            )
            .await?;
            let storyboard_count = agent_json_array(&storyboards)
                .map(|items| items.len())
                .unwrap_or_default();

            let validation = json!({
                "downloadedScriptPath": script_path.to_string_lossy(),
                "downloadedCharCount": downloaded_text.chars().count(),
                "bibleHasMarker": value_contains(&bible, "Agent E2E 大纲"),
                "charactersHaveMarkers": value_contains(&characters, "Agent验收主角") && value_contains(&characters, "Agent验收对手"),
                "scenesHaveMarkers": value_contains(&scenes, "Agent验收室内冲突场") && value_contains(&scenes, "Agent验收街道路口"),
                "propsHaveMarker": value_contains(&props, "Agent验收关键纸条"),
                "episodeHasMarker": value_contains(&episode_read, "Agent E2E 第一集剧本正文"),
                "storyboardCount": storyboard_count,
                "storyboardsHavePromptFields": value_contains(&storyboards, "visualPrompt") || value_contains(&storyboards, "videoPrompt"),
            });
            run.report.validation = validation.clone();
            run.step("validate-local-and-remote-state", true, validation.clone());

            for (field, ok) in [
                ("bibleHasMarker", validation["bibleHasMarker"].as_bool().unwrap_or(false)),
                (
                    "charactersHaveMarkers",
                    validation["charactersHaveMarkers"].as_bool().unwrap_or(false),
                ),
                ("scenesHaveMarkers", validation["scenesHaveMarkers"].as_bool().unwrap_or(false)),
                ("propsHaveMarker", validation["propsHaveMarker"].as_bool().unwrap_or(false)),
                ("episodeHasMarker", validation["episodeHasMarker"].as_bool().unwrap_or(false)),
                (
                    "storyboardsHavePromptFields",
                    validation["storyboardsHavePromptFields"].as_bool().unwrap_or(false),
                ),
            ] {
                if !ok {
                    return Err(format!("validation field failed: {field}; validation={validation}"));
                }
            }
            if storyboard_count < 2 {
                return Err(format!(
                    "expected at least 2 storyboards from agent, got {storyboard_count}"
                ));
            }

            for forbidden_path in [
                workspace_root.join(".cuelight").join(&project_id).join("source").join("chunks"),
                workspace_root.join(".cuelight").join("source").join("chunks"),
                workspace_root.join(".cuelight").join("keyframes"),
                workspace_root.join(".cuelight").join("video-assets"),
            ] {
                if forbidden_path.exists() {
                    return Err(format!(
                        "out-of-scope local artifact exists: {}",
                        forbidden_path.display()
                    ));
                }
            }
            run.step(
                "verify-boundaries",
                true,
                json!({
                    "notCalled": [
                        "source chunks",
                        "seasons",
                        "keyframes",
                        "video assets",
                        "screenwriter/source workflow",
                        "paid image/video generation",
                        "upload_file"
                    ]
                }),
            );

            Ok(())
        }
        .await;

        match flow {
            Ok(()) => {
                run.report.success = true;
                run.report.error = None;
                if let Err(err) = run.write_report().await {
                    run.report.success = false;
                    run.report.error = Some(err);
                    return Err(run);
                }
                Ok(run)
            }
            Err(err) => {
                run.report.success = false;
                run.report.error = Some(err);
                let _ = run.write_report().await;
                Err(run)
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn cuelight_live_agent_llm_original_to_assets_e2e() {
        match run_cuelight_live_agent_llm_e2e().await {
            Ok(run) => {
                eprintln!(
                    "CueLight live agent+LLM E2E passed. projectId={:?} report={}",
                    run.report.project_id, run.report.report_path
                );
            }
            Err(run) => {
                panic!(
                    "CueLight live agent+LLM E2E failed. projectId={:?} report={} error={:?}",
                    run.report.project_id, run.report.report_path, run.report.error
                );
            }
        }
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
