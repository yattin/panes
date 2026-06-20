# Vendor claurst 作为 Rust 库集成进 panes(方案)

> 与 `claurst-engine-integration.md`(ACP/引擎接入视角)并列。本文聚焦**以 Rust crate 库集成(同 `vendor/claude-code-rust` 模式)取代 ACP sidecar** 的方案。
> 状态:**技术方案已定,license 待定**。未进入编码实现。

## 1. 背景与动机

panes 已有成熟的 in-process 集成范式:`vendor/claude-code-rust`(`claude_code_rs`,MIT)作为 path 依赖,`claude_code_native` 引擎直接 `use claude_code_rs::api::{ApiClient,...}` / `::tools::{FileReadTool,...}`,in-process 跑 agent 并产出 panes 的 `EngineEvent`。

相比 ACP sidecar(stdio 子进程 + WS 桥),in-process vendor 更原生:

- 无 IPC 开销;
- 直接复用 panes 的 `Engine` 抽象 / 权限审批 UI / `SandboxPolicy` / `writable_roots` / 终端 / 事件流;
- claurst 已具备等价库入口(`run_query_loop` + `AnthropicClient` + `all_tools`),可照搬该模式新增 `ClaurstEngine`。

**已确认的决策**:

- 技术方案先行,**license 待定**(见 §2);
- agent 循环**直接用 `run_query_loop`**(复用 claurst 成熟循环:compact / tool-budget / cost),而非像 `claude_code_native` 自写循环。

## 2. ⚠️ 阻断点:License 待定(分发前必须解决)

这是和"像 claude-code-rust 那样"做不到等价的根本原因:

| 项 | claude-code-rust | claurst |
|---|---|---|
| License | **MIT** | **GPL-3.0** |
| 能否被 panes 自由 vendor | ✅ | ❌ |

vendor(静态链接进 panes 二进制)→ **GPL 传染,panes 须以 GPL-3.0 开源**;公开 C 端分发还附 *Installation Information* 义务。

落地前必选其一:

1. 接受 panes GPL 化;
2. 联系 claurst 作者(kuberwastaken)商业 / dual 授权;
3. 放弃 claurst,改用 MIT 许可的 agent。

## 3. 技术风险(实现前需逐一验证)

### 3.1 wreq / BoringSSL 符号冲突(首要验证项)

`claurst-api` **硬依赖** `wreq`(`crates/api/Cargo.toml:13`,BoringSSL / boring-sys,非 optional);panes 用 `git2` 的 `vendored-openssl`(openssl-sys)。`boring-sys` + `openssl-sys` 是 claurst 自己 workspace `Cargo.toml` 注释点名的冲突对。

**解法**:fork claurst,把 `wreq` / `bun_tls`(`api/src/lib.rs:26`)设为 **optional feature**(条件编译 Anthropic TLS impersonation 路径),panes 不启用 → 走 reqwest + rustls。代价:产生需同步上游的 fork。

### 3.2 权限桥接

`run_query_loop` 内部执行工具时,经 `ToolContext.permission_handler`(`tools/lib.rs:281`)做**同步** `request_permission`;panes 审批是异步 oneshot。需实现自定义 `PermissionHandler`,在同步 fn 内阻塞等待 panes 决策(阻塞 channel)。

先研究 claurst 交互式权限流:`PendingPermissionStore`(`tools/lib.rs:177`)+ `AskPermissionHandler`(`:594`)+ `request_permission_inner`(`:343`)如何挂起 / 恢复。

### 3.3 workspace vendor 复杂度

claurst 是**多 crate workspace**(不像 claude-code-rust 是单 crate)。要 vendor 整个 `src-rust/`(保持内部 path 结构),panes 引 `query/api/core/tools` 子集,并协调 workspace 级依赖(reqwest 0.13 vs panes 版本等)。

## 4. 方案

### 4.1 vendor 集成(参照 claude-code-rust)

- `claurst/src-rust/` → `panes/vendor/claurst/`(保留 `crates/` 结构)
- `panes/src-tauri/Cargo.toml` 加:

  ```toml
  claurst-query = { path = "../vendor/claurst/crates/query" }
  claurst-api   = { path = "../vendor/claurst/crates/api" }   # 视 wreq 改造加 default-features = false
  claurst-tools = { path = "../vendor/claurst/crates/tools" }
  claurst-core  = { path = "../vendor/claurst/crates/core" }
  ```

- 协调 reqwest / tokio 等 workspace 依赖版本对齐

### 4.2 ClaurstEngine(照抄 `claude_code_native.rs` 结构)

新增 `panes/src-tauri/src/engines/claurst_engine.rs`,实现 `Engine` trait(`engines/mod.rs:425`):

- `start_thread`:建 `claurst_api::AnthropicClient`(从 claurst Settings/Config)、配 `ToolContext`(cwd = `scope.root`、writable_roots、permission_mode、**自定义 PermissionHandler**),存 `ThreadState`(history: `Vec<claurst_core::types::Message>` + client + tool_ctx + cancel + pending_approvals oneshot 表)
- `send_message`:spawn 调

  ```rust
  claurst_query::run_query_loop(
      &client, &mut messages, &claurst_tools::all_tools(),
      &tool_ctx, &query_config, cost_tracker,
      Some(bridge_tx), cancel, None,
  )   // query/lib.rs:703
  ```

  bridge task 把 `QueryEvent` → `EngineEvent` 推给 panes 的 `event_tx`
- `respond_to_approval`:唤醒对应权限 oneshot
- `interrupt`:触发 `CancellationToken`

### 4.3 QueryEvent → EngineEvent 映射(`query/lib.rs:450` → `engines/events.rs:113`)

| claurst `QueryEvent` | panes `EngineEvent` |
|---|---|
| `Stream(ContentBlockDelta{TextDelta})` | `TextDelta` |
| `Stream(...ThinkingDelta)` | `ThinkingDelta` |
| `ToolStart{tool_name,input_json}` | `ActionStarted{action_type: 按名映射}` |
| `ToolEnd{result,is_error}` | `ActionCompleted{success=!is_error, output=result}` |
| `TurnComplete{usage,stop_reason}` | `TurnCompleted{token_usage, status}` |
| `Error` | `Error{recoverable}` |
| `TokenWarning` | `UsageLimitsUpdated`(部分字段) |
| `Status` | `Notice` 或忽略 |

- 工具名 → `ActionType`(`events.rs:191`):`Read`→FileRead、`Write`→FileWrite、`Edit`→FileEdit、`Bash`/`PtyBash`/`PowerShell`→Command、`Glob`/`Grep`/`Web*`→Search
- 注:claurst 工具输出在 `ToolEnd` 一次性返回(非流式),panes 的 `ActionOutputDelta` / `DiffUpdated` 无直接对应 —— `ActionCompleted.output` 已足够

### 4.4 权限桥接(关键实现点)

```rust
impl PermissionHandler for PanesPermissionHandler {
    fn request_permission(&self, request: &PermissionRequest) -> PermissionDecision {
        // 同步 fn 内:
        // 1) 经阻塞 channel 把请求送出
        // 2) 上游经 event_tx emit ApprovalRequested 给 panes UI
        // 3) panes respond_to_approval 决策回灌
        // 4) 阻塞 recv 返回 PermissionDecision
    }
}
```

可借助 claurst 的 `PendingPermissionStore` 模式而非全自造。**死锁注意**:同步阻塞不能卡死 `run_query_loop` 所在的运行时线程,可能需 `tokio::task::block_in_place` 或独立运行时。

### 4.5 能力注册(`engines/mod.rs`)

- `capabilities_for_engine`(`:151`)加:

  ```rust
  const CLAURST_CAPABILITIES: EngineCapabilities = EngineCapabilities {
      permission_modes: &["restricted", "standard", "trusted"],
      sandbox_modes: &["read-only", "workspace-write"],
      approval_decisions: &["accept", "decline", "accept_for_session"],
  };
  // match 加: "claurst" => CLAURST_CAPABILITIES,
  ```

- `EngineManager`(`:469`)加 `claurst: Arc<ClaurstEngine>` + `new()` / `with_db()` 初始化 + 各 `match engine_id` 分支(start / send / steer / approval / interrupt / archive,`:786+`)
- 前端引擎选择 UI 自动从 `list_engines` 获取,无需硬编码

## 5. 关键文件

- 新增:`panes/src-tauri/src/engines/claurst_engine.rs`
- 改:`panes/src-tauri/src/engines/mod.rs`(EngineManager 字段 + capabilities + 各路由分支)
- 改:`panes/src-tauri/Cargo.toml`(path 依赖 + 依赖协调)
- 新增:`panes/vendor/claurst/`(claurst workspace 拷贝)
- 可能 fork claurst:wreq 设 optional(`crates/api/Cargo.toml` + workspace `Cargo.toml` + `bun_tls` 模块条件编译)

## 6. 复用的现成实现(避免重造)

- **模板**:`contexts/agent_runtime/claude_code_native/mod.rs`(ThreadState / build_client / build_system_prompt / execute_native_tool / 审批 oneshot / cancel 全套结构)
- **panes 基建**:`EngineEvent`、`SandboxPolicy`、`ThreadScope`、`respond_to_approval`、前端审批 / 沙箱 UI
- **claurst 库**:`run_query_loop`、`AnthropicClient`、`all_tools()`、`ToolContext`、`PermissionHandler`

## 7. 验证

1. `cd panes/src-tauri && cargo build` —— **首要**:验证 wreq/BoringSSL 与 git2/openssl 能否共存编译(失败 → 落地 wreq optional 改造)
2. 单测:`ClaurstEngine::start_thread` + `send_message`,断言 `EngineEvent` 序列(`TurnStarted → (TextDelta|ActionStarted|ActionCompleted)* → TurnCompleted`)
3. 权限:触发写工具,验证 `ApprovalRequested` ↔ `respond_to_approval` 闭环
4. 端到端:`pnpm tauri:dev`,选 Claurst 引擎发 prompt,观察前端事件流 / 审批 UI / 终端
5. `cargo test -p agent_workspace_lib`

## 8. 待解决问题

1. **License**(阻断点,见 §2)
2. wreq optional 改造是否必须 + fork 同步成本
3. 权限桥接:同步 `PermissionHandler` ↔ 异步 oneshot 的阻塞 channel 细节(死锁规避)
4. claurst Settings/Config 加载路径硬编码 `$HOME/.claurst`(`core/lib.rs:1475`)在 panes 进程内如何重定向 / 内存构造
5. claurst compact / tool-budget / cost 与 panes token 统计语义对齐

## 9. 与其他文档的关系

| 文档 | 视角 | 状态 |
|---|---|---|
| `claurst-engine-integration.md` | ACP/引擎接入(含本地/远程、文件桥接) | 方案 |
| `bridge-workspace-design.md`(在 claurst 目录) | 远程模式文件桥接子协议 | 方案(仅远程模式需要) |
| **本文** | **Rust crate 库集成(取代 ACP)** | **方案,license 待定** |

> 若最终走"库集成"(本文),则 ACP sidecar 与文件桥接均不再需要 —— in-process 直接操作本地 FS,与 `claude_code_native` 一致。
