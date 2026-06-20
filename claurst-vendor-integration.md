# 基于 claurst 的干净重构集成方案(决策版)

> 本文取代早先"把 claurst 整个 workspace 作为 path 依赖 vendor 进 panes"的设想。
> **主线决策:不走 vendor(会触发 GPL 传染),而是基于 claurst 的思想/行为做 clean-room 干净重构**,产出 panes 自有的 MIT 代码,**丢弃全部 TUI 与冗余 crate**。
> 状态:**方向与决策已定,进入设计/实现阶段**。License 阻断点已由"干净重构"绕过(见 §3 合规纪律)。
> 配套视角:`claurst-engine-integration.md`(ACP sidecar 接入)——若走本文的 in-process 重构,ACP sidecar 与文件桥接均不再需要。

---

## 0. 决策摘要(TL;DR)

| 议题 | 决策 | 理由 |
|---|---|---|
| 集成方式 | **clean-room 干净重构**,不 vendor claurst 任何 crate 进编译 | vendor(静态链接 GPL)→ GPL-3.0 传染,panes 须转 GPL;clean-room 产出独立版权作品,可保 MIT |
| License | 新代码 **MIT**(随 panes) | 思想/表达二分(Baker v. Selden)+ clean-room 先例(Phoenix v. IBM,claurst 自身亦据此) |
| TUI | **全部丢弃**(tui/cli/commands/buddy/bridge/plugins/acp) | panes 有自己的 Tauri 前端;这些 crate 与核心 agent 循环解耦 |
| HTTP 栈 | **reqwest 0.12 + rustls**(panes 现有),重写 SSE Messages 客户端 | 彻底回避 `wreq`/BoringSSL ↔ `git2`/openssl-sys 符号冲突;统一 reqwest 版本(0.13→0.12) |
| Agent 循环 | **自实现等价 `run_query_loop`**(compact/tool-budget/cost),骨架借鉴 `claude_code_native::send_message` | 复用 claurst 成熟循环的*行为*;复用 panes 现成的异步审批/事件/CancellationToken |
| 权限 | **异步 oneshot**(照 `claude_code_native` 模板),不照搬 claurst 同步 `PermissionHandler` | 规避同步阻塞 channel + `block_in_place` 死锁(原 vendor 方案 §3.2 难点消失) |
| MCP | **可选,后续单独引入**(`rmcp`) | 非首期必需;panes 已有 cuelight_tools 等扩展机制 |
| 代码组织 | 新增 workspace crate **`panes-agent`**(MIT),引擎适配层 `engines/panes_native.rs` | 与 `vendor/claude-code-rust`(单 crate)模式对齐;解耦 src-tauri |
| `vendor/claurst/` | **降级为只读参考**,不参与编译、不入分发包二进制 | 保留作行为参考;避免 GPL 源码随分发传播 |

---

## 1. 背景与动机

panes 已有成熟的 in-process 集成范式:`vendor/claude-code-rust`(`claude_code_rs`,**MIT**)作为 path 依赖(`src-tauri/Cargo.toml:17`,`default-features = false`),`claude_code_native` 引擎(`contexts/agent_runtime/claude_code_native/mod.rs`)直接 `use claude_code_rs::api::{ApiClient, ModelStreamEvent, ...}` / `::tools::{FileReadTool, ...}`,in-process 跑 agent 并产出 panes 的 `EngineEvent`。

claurst(`vendor/claurst/`,GPL-3.0)是 Claude Code 行为的 Rust clean-room 重实现,提供等价能力且循环更成熟(`run_query_loop`:auto-compact / tool-budget / cost tracker / 多轮)。panes 希望获得这层能力,但 claurst 的 license 与 TUI 负担使其**不能**像 claude-code-rust 那样直接 vendor。

由此引出本文的核心判断:**参照 claurst 的行为,在 panes 内干净重写一份,而非搬运其代码。**

---

## 2. 为什么放弃 vendor、转向干净重构

### 2.1 原 vendor 方案的致命阻断

| 项 | claude-code-rust(已 vendor) | claurst(原计划 vendor) |
|---|---|---|
| License | **MIT** | **GPL-3.0** |
| 能否被 MIT 的 panes 自由 vendor | ✅ | ❌ |
| crate 形态 | 单 crate | 12-crate workspace |
| HTTP 栈 | reqwest | **wreq(BoringSSL)** 硬依赖 |

vendor(静态链接进 panes 二进制)→ **GPL 传染,panes 须以 GPL-3.0 开源**;公开 C 端分发还附 *Installation Information* 义务。这是和"像 claude-code-rust 那样"做不到等价的根本原因。

### 2.2 干净重构如何同时解掉所有阻断

不仅解 license,还顺带消解 vendor 方案的全部技术风险(原方案 §3):

| 原 vendor 风险 | 干净重构后 |
|---|---|
| §3.1 `wreq`/BoringSSL ↔ `git2`/openssl-sys 符号冲突(首要阻断) | **消失**——重写 HTTP 层直接用 panes 的 reqwest 0.12 + rustls,无 wreq |
| §3.1 `AnthropicClient.http: wreq::Client`(`api/src/lib.rs:457`)是主体非旁路,feature-gate 不可行 | 不复用该类型,自写客户端 |
| reqwest **0.12**(panes `Cargo.toml:14`) ↔ **0.13**(claurst workspace `:35`)大版本分叉 | 重写只依赖 panes 的 0.12,无双版本 |
| §3.2 同步 `PermissionHandler`(`tools/src/lib.rs:604`)↔ 异步审批,需 `block_in_place` 防死锁 | **改为异步 oneshot**(照 `claude_code_native` 模板 `mod.rs:461`),难点消失 |
| §3.3 12-crate workspace vendor + workspace 级依赖协调 | 不引入 claurst workspace,只写 panes 需要的子集 |
| TUI/插件/companion/远程桥等冗余代码随 vendor 进来 | 整体丢弃(见 §4) |

**结论:干净重构在 license、TLS 冲突、版本协调、权限死锁、冗余五个维度上全面优于 vendor。** 代价是要重写 agent 循环 + SSE 客户端 + 工具集,但 `claude_code_native` 是现成骨架,claurst `run_query_loop` 提供行为参照,工作量可控(见 §9 权衡)。

---

## 3. License / 合规决策(关键章节)

### 3.1 决策

> **基于 claurst 的思想与行为,在 panes 内 clean-room 重写;新代码以 MIT 发布;不静态链接、不分发任何 claurst GPL 源码。**

### 3.2 法律依据

- **思想/表达二分**(*Baker v. Selden*, 1879):版权保护*表达*(expression),不保护*思想、方法、行为*。agent 循环算法、工具契约、事件模型属于行为/方法层面,不受版权保护。
- **clean-room 先例**(*Phoenix Technologies v. IBM*, 1984,BIOS clean-room):通过"规格→独立实现"两阶段隔离,产出独立版权作品,不构成衍生作品。claurst 自身即据此从 Claude Code TypeScript 规格重写为 Rust(见 `vendor/claurst/README.md` "Important Notice")。
- 因此:**参照 claurst 的行为独立重写,产出的是 panes 的独立版权作品,不触发 GPL copyleft,可保持 MIT。**

### 3.3 必须遵守的 clean-room 纪律(否则结论不成立)

clean-room 的安全性**完全取决于执行纪律**,不是"看了 GPL 代码就能随便重写":

1. **两阶段隔离**:
   - **规格阶段**:从 claurst 的*行为*提炼接口契约与算法描述(可参考 `vendor/claurst/spec/`——这是行为规格层,属思想;以及 `run_query_loop` 的可观察行为)。产出一份 panes 自己的规格文档。
   - **实现阶段**:实现者按规格**独立写 Rust**,目标是"按行为规格实现",而非"翻译 claurst 源码"。
2. **避免实质性相似**(*substantial similarity* 是衍生作品的判定核心):不得逐行/逐函数翻译 claurst 的 `src-rust/`。命名、结构、注释应体现 panes 自身设计,而非照搬。
3. **表达层只读、不复制**:`vendor/claurst/src-rust/` 仅作"理解行为"的只读参考;任何代码片段都不进入 panes 仓库(连"改改变量名"也不行)。
4. **spec 文本不可复制**:`vendor/claurst/spec/*.md` 文本本身是文字作品受版权保护;只能"按其描述的行为实现",不能复制其文字。
5. **声明与致谢**:新 crate 头部加 clean-room 声明——"Behavior inspired by claurst (GPL-3.0 by kuberwastaken); clean-room reimplementation, no source code derived."

### 3.4 合规清单(分发前逐项确认)

- [ ] `panes-agent` crate 与引擎层 license = MIT,头含 clean-room 声明 + claurst 致谢
- [ ] panes `Cargo.toml` **不**出现任何 `path = "../vendor/claurst/..."` 依赖(不编译、不链接)
- [ ] 分发包二进制中不含 claurst GPL 目标码
- [ ] `vendor/claurst/` 在公开分发中明确标注"只读参考,非 panes 组成部分";(可选)将其移出主仓库为独立参考仓,或仅本地保留
- [ ] `vendor/README.md` 补充 claurst 条目:定位=行为参考、license=GPL-3.0、不参与编译

### 3.5 诚实声明(不确定性)

- clean-room 是**法律上可行的路径**,但"是否构成衍生作品"最终是**事实问题**(看实质相似性 + 接触),需个案判断。本方案给出的是工程上最稳妥的姿势,**不等同于法律意见**;面向公开/商业分发前建议由法务复核重写产出与 claurst 的相似度。
- 若团队无法保证 §3.3 纪律(例如实现者不可避免地逐行参照),则应退回备选:① 接受 panes GPL 化;② 向 claurst 作者(kuberwastaken)寻求商业/dual 授权;③ 放弃 claurst、改用其他 MIT agent。三者均非本文主线。

---

## 4. 范围:保留重写 vs 丢弃

claurst workspace 共 12 crate(`vendor/claurst/src-rust/Cargo.toml:3-16`)。按下表处置:

### 4.1 保留并重写(panes 自有 `panes-agent`)

| claurst 来源(行为参照) | 重写目标 | 说明 |
|---|---|---|
| `query`(`run_query_loop` `query/src/lib.rs:703`,`QueryEvent` `:450`,`QueryConfig` `:76`) | agent loop | compact / tool-budget / cost / 多轮;骨架借鉴 `claude_code_native::send_message` |
| `api`(`AnthropicClient` `api/src/lib.rs:457`) | api client | reqwest 0.12 + rustls + SSE Messages 流;**不带 wreq/BoringSSL impersonation** |
| `tools`(`all_tools()` `tools/src/lib.rs:525`,`ToolContext` `:278`,`Tool` trait) | 工具集 | 优先复用 panes 已有实现(`execute_native_tool` `claude_code_native/mod.rs:270`、`cuelight_tools`) |
| `tools`/`core`(权限:`PermissionHandler`/`PermissionRequest`/`PermissionDecision`) | 异步权限模型 | 改异步 oneshot(不照搬同步 handler) |
| `core`(`Message`/`ContentBlock`/`UsageInfo`/`CostTracker`) | 类型层 | 对齐 Anthropic Messages API + panes `EngineEvent` |

### 4.2 丢弃(不重写)

| crate | 职责 | 丢弃理由 |
|---|---|---|
| `tui` | ratatui 终端 UI | panes 有 Tauri 前端 |
| `cli` | clap 入口、子命令 | panes 是 GUI 应用 |
| `commands` | 斜杠命令(`/share`、`/goal` 等) | 命令体系由 panes 前端 own |
| `buddy` | companion "Rustle" | 与 panes 无关 |
| `bridge` | 远程 CLI↔本地文件桥接 | in-process 直接操作本地 FS |
| `plugins` | 插件 marketplace(`zip` 下载/校验) | panes 有自己的扩展机制 |
| `acp` | ACP server(stdio JSON-RPC) | in-process 不需要 sidecar |

### 4.3 丢弃的横向能力(随 crate 一并去除)

- **wreq / wreq-util / BoringSSL TLS impersonation**(`bun_tls` `api/src/lib.rs:26`):claurst 用它伪装 Bun TLS 指纹绕 Cloudflare;panes 直连 Anthropic API 不需要。
- **hooks 系统**(`PostModelTurn` 等 `query/src/lib.rs:492`):panes 有自己的 hook/事件机制,首期不引入。
- **sessions / oauth / accounts 持久化**(`core/src/lib.rs:3069/3591/3740`):panes 有自己的 DB + 鉴权。
- **`config_dir()` 硬编码 `~/.claurst`**(`core/src/lib.rs:1475`):重写后配置完全由 panes 注入(`ThreadScope`/`SandboxPolicy`/内存 Settings),无独立配置目录。

### 4.4 可选(后续单独评估)

- **`mcp`**(`rmcp = "1.4.0"`,`mcp/Cargo.toml:31`):标准 MCP 客户端。若 panes 需要 MCP 工具支持,可作为独立可选 crate 引入(注意 rmcp 版本与 panes 依赖协调);**首期不做**。

> 验证解耦:核心 crate 间依赖为 `query → {core, api, tools}`、`tools → {core, api, mcp}`、`api → {core}`,**核心不依赖任何 TUI crate**,故 TUI 可整体丢弃。

---

## 5. 技术方案

### 5.1 代码组织

新增 workspace 成员 crate `panes-agent`(MIT),与 `vendor/claude-code-rust` 模式对齐:

```
panes/
├─ Cargo.toml                  # workspace.members 加 "crates/panes-agent"
├─ crates/
│  └─ panes-agent/             # 新增,MIT,clean-room 重写
│     ├─ Cargo.toml            # 仅依赖 panes workspace 依赖(reqwest 0.12/rustls/tokio/serde...)
│     └─ src/
│        ├─ lib.rs             # 公共 API:AgentClient / run_agent_loop / AgentEvent / ToolContext / Tool trait
│        ├─ api.rs             # reqwest 0.12 + rustls,Anthropic Messages SSE 流式客户端
│        ├─ loop.rs            # agent 循环(compact/tool-budget/cost/多轮),产出 AgentEvent
│        ├─ tools/             # FileRead/FileWrite/FileEdit/Bash/Glob/Grep... (优先复用 panes 现有实现)
│        ├─ permission.rs      # 异步 PermissionHandler + PermissionRequest/Decision
│        └─ types.rs           # Message/ContentBlock/Usage(对齐 Anthropic API + EngineEvent)
└─ src-tauri/src/
   └─ engines/
      └─ panes_native.rs       # 新增:Engine trait 适配层(AgentEvent → EngineEvent)
```

`panes-agent` 对外暴露的公共表面(自定,不照搬 claurst 命名):

```rust
// 等价 claurst AnthropicClient —— 用 panes 的 reqwest 0.12 + rustls
pub struct AgentClient { /* reqwest::Client + 配置 */ }
impl AgentClient {
    pub fn new(config: AgentClientConfig) -> anyhow::Result<Self>;
    pub async fn stream_messages(/* ... */) -> impl Stream<Item = ApiStreamEvent>; // SSE
}

// 等价 claurst run_query_loop —— 异步,产出 AgentEvent
pub async fn run_agent_loop(
    client: &AgentClient,
    messages: &mut Vec<AgentMessage>,
    tools: &[Box<dyn Tool>],
    ctx: &ToolContext,
    config: &AgentLoopConfig,        // max_turns/system_prompt/output_style/...
    cost: Arc<CostTracker>,
    event_tx: mpsc::Sender<AgentEvent>,
    cancel: CancellationToken,
) -> anyhow::Result<AgentOutcome>;

// 异步权限(替代 claurst 同步 PermissionHandler)
#[async_trait]
pub trait PermissionHandler: Send + Sync {
    async fn request(&self, req: PermissionRequest) -> PermissionDecision;
}
```

### 5.2 API client(无 wreq)

- 用 panes workspace 的 `reqwest 0.12`(`rustls-tls`,`Cargo.toml:14`)+ tokio SSE,实现 Anthropic Messages `/v1/messages` 流式。
- 解析标准 SSE 事件:`message_start` / `content_block_start` / `content_block_delta`(text_delta / input_json_delta / thinking_delta)/ `message_delta`(usage、stop_reason)/ `message_stop` / `error`。
- **不做 TLS 指纹伪装**(claurst 的 `bun_tls`/Bun profile 不引入)。
- API key / model / base_url 由 panes 注入(从 panes 的 engine settings),不读 `~/.claurst`。

### 5.3 Agent 循环

行为参照 claurst `run_query_loop`(`query/src/lib.rs:703`),骨架借鉴 panes `claude_code_native::send_message`(`mod.rs:1060`,已是"自写循环 + MAX_AGENT_ROUNDS"模式)。集成:

- **多轮工具循环**:模型输出 tool_use → 经权限审批 → 执行工具 → tool_result 回灌 → 继续直到 `end_turn`/`stop_reason` 或达 `max_turns`。
- **auto-compact**:参照 claurst `compact::AutoCompactState`(`query/src/lib.rs:715`),上下文超阈值时压缩历史(首期可简化为软上限告警,按需补全)。
- **cost tracker**:累计 input/output/cache token 与美元成本,供 `TurnCompleted.token_usage`。
- **tool-budget / max_turns**:可配置上限,超额 emit `Error{recoverable}`。
- **取消**:`CancellationToken` 贯穿(照 `claude_code_native` `mod.rs:1470` 的 interrupt 模式)。

### 5.4 工具集

优先**复用 panes 已有实现**,避免重复造:

- `claude_code_native::execute_native_tool`(`mod.rs:270`)已封装 FileRead/FileEdit/FileWrite/Command 等。
- `engines/cuelight_tools.rs` 已有领域工具。
- 仅在 panes 缺失时按 `panes-agent::Tool` trait 新增(如 Glob/Grep/WebSearch)。工具执行结果一次性返回(对齐 claurst `ToolEnd`),映射到 `EngineEvent::ActionCompleted`。

### 5.5 权限(异步 oneshot,关键简化)

照搬 `claude_code_native` 已验证的模式,而非 claurst 的同步 handler:

```rust
// 引擎层(mod.rs:461)已有:emit ApprovalRequested + 注册 oneshot
let (tx, rx) = oneshot::channel::<Value>();
pending_approvals.lock().await.insert(approval_id.clone(), tx);
event_tx.send(EngineEvent::ApprovalRequested { approval_id, action_type, summary, details }).await;
let decision = rx.await; // 异步等待 panes UI 决策

// respond_to_approval(mod.rs:1451)唤醒:
let sender = pending_approvals.lock().await.remove(approval_id);
if let Some(tx) = sender { let _ = tx.send(response); }
```

`panes-agent` 的 `PermissionHandler::request` 为 `async fn`,内部走这套 oneshot。**完全规避原 vendor 方案 §3.2/§4.4 的同步阻塞 + `block_in_place` 死锁问题。**

### 5.6 事件映射(`AgentEvent`/`QueryEvent` → `EngineEvent`)

`panes_native.rs` 适配层把 `panes-agent` 的 `AgentEvent` 翻译为 panes `EngineEvent`(`engines/events.rs:113`)。映射(参照 claurst `QueryEvent` `query/src/lib.rs:450`):

| `AgentEvent`(等价 claurst `QueryEvent`) | panes `EngineEvent`(`events.rs`) |
|---|---|
| `Stream(TextDelta)` | `TextDelta{content}`(`:123`) |
| `Stream(ThinkingDelta)` | `ThinkingDelta{content}`(`:126`) |
| `ToolStart{tool_name, tool_id, input_json}` | `ActionStarted{action_id, action_type, summary, ...}`(`:129`) |
| `ToolEnd{tool_id, result, is_error}` | `ActionCompleted{result: ActionResult{success: !is_error, output: result, ...}}`(`:147`) |
| `TurnComplete{turn, stop_reason, usage}` | `TurnCompleted{token_usage, status: Completed}`(`:119`) |
| `Status(String)` | `Notice` 或忽略 |
| `Error(String)` | `Error{message, recoverable}`(`:175`) |
| `TokenWarning{state, pct_used}` | `UsageLimitsUpdated`(部分字段,`:161`) |
| (工具执行前需审批) | `ApprovalRequested{approval_id, action_type, summary, details}`(`:155`) |

- **工具名 → ActionType**(`events.rs:191`):`Read`→FileRead、`Write`→FileWrite、`Edit`→FileEdit、`Bash/PtyBash/PowerShell`→Command、`Glob/Grep/Web*`→Search。
- 注:工具输出在 `ToolEnd` 一次性返回(非流式),panes 的 `ActionOutputDelta`/`DiffUpdated` 无直接对应——`ActionCompleted.output` 已足够。
- 循环起止由适配层补发 `TurnStarted`(`:116`)/`TurnCompleted`(`:119`)。

### 5.7 Engine trait 实现 + 注册

新增 `engines/panes_native.rs`,实现 `Engine` trait(`engines/mod.rs:426`),结构照抄 `ClaudeCodeNativeEngine`(`mod.rs:78`):

- `PanesNativeEngine { threads: Arc<Mutex<HashMap<String, ThreadState>>>, pending_approvals: Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>> }`
- `start_thread`(`:433`):建 `AgentClient`、配 `ToolContext`(working_dir=`scope.root`、permission_mode、异步 `PermissionHandler`),存 `ThreadState{history, root_path, ...}`
- `send_message`(`:441`):spawn 调 `panes_agent::run_agent_loop`,bridge task 把 `AgentEvent` → `EngineEvent` 推 `event_tx`
- `respond_to_approval`(`:1451` 模式):唤醒 oneshot
- `interrupt`(`:1470` 模式):触发 `CancellationToken`

能力注册(`engines/mod.rs`):

```rust
const PANES_NATIVE_CAPABILITIES: EngineCapabilities = EngineCapabilities {
    permission_modes: &["restricted", "standard", "trusted"],
    sandbox_modes: &["read-only", "workspace-write"],
    approval_decisions: &["accept", "decline", "accept_for_session"],
};
// capabilities_for_engine(:151) 加: "panes-native" => PANES_NATIVE_CAPABILITIES,
```

`EngineManager`(`:469`)加 `panes_native: Arc<PanesNativeEngine>` + `new()`/`with_db()` 初始化 + 各 `match engine_id` 路由分支(`ensure_engine_thread` `:776` / `send_message` `:813` / `steer` `:869` / `respond_to_approval` `:901` / `interrupt` `:914` / `archive` `:929`)。前端引擎选择 UI 自动从 `list_engines` 获取,无需硬编码。

---

## 6. 关键文件

**新增**
- `crates/panes-agent/`(MIT,clean-room 重写:api/loop/tools/permission/types)
- `src-tauri/src/engines/panes_native.rs`(Engine trait 适配层)

**改动**
- `Cargo.toml`(workspace.members 加 `crates/panes-agent`)
- `src-tauri/Cargo.toml`(`panes-agent = { path = "../crates/panes-agent" }`)
- `src-tauri/src/engines/mod.rs`(`#[path]` 声明 + `EngineManager` 字段 + `capabilities_for_engine` + 各路由分支)
- `vendor/README.md`(补 claurst 条目:只读行为参考、GPL-3.0、不参与编译)

**参考(只读,不改动、不依赖)**
- `vendor/claurst/src-rust/`(行为参考)、`vendor/claurst/spec/`(规格参考)

**模板(复用其结构)**
- `src-tauri/src/contexts/agent_runtime/claude_code_native/mod.rs`(`ThreadState`/`build_client`/`build_system_prompt`/审批 oneshot/`send_message` 循环/cancel 全套)

---

## 7. 实施步骤

1. **规格先行**:基于 claurst `spec/` + `run_query_loop` 可观察行为,写一份 `panes-agent` 的接口/算法规格(clean-room 规格阶段,见 §3.3)。
2. 建 `crates/panes-agent`(MIT,含 clean-room 声明头),workspace 注册。
3. `types.rs`:`Message`/`ContentBlock`/`Usage`/`CostTracker`(对齐 Anthropic API + `EngineEvent` 字段)。
4. `api.rs`:`AgentClient`(reqwest 0.12 + rustls)+ SSE Messages 流式解析。
5. `permission.rs`:异步 `PermissionHandler` + `PermissionRequest`/`PermissionDecision`。
6. `tools/`:封装 panes 现有工具实现为 `Tool` trait;补缺失项(Glob/Grep/Web)。
7. `loop.rs`:`run_agent_loop`(多轮 + tool-budget + cost;auto-compact 首期简化)。
8. `engines/panes_native.rs`:Engine trait 实现 + `AgentEvent`→`EngineEvent` 映射。
9. `engines/mod.rs`:`EngineManager` 字段 + `capabilities_for_engine` + 路由分支注册。
10. `vendor/README.md` + 合规清单(§3.4)逐项确认。

---

## 8. 验证

1. `cargo build -p panes-agent && cd src-tauri && cargo build` —— 验证无 wreq、reqwest 单版本、无符号冲突。
2. 单测:`run_agent_loop` 断言 `AgentEvent`/`EngineEvent` 序列(`TurnStarted → (TextDelta|ActionStarted|ApprovalRequested|ActionCompleted)* → TurnCompleted`)。
3. 权限:触发写工具,验证 `ApprovalRequested`(`events.rs:155`)↔ `respond_to_approval` 闭环。
4. 端到端:`pnpm tauri:dev`,选 Panes Native 引擎发 prompt,观察前端事件流/审批 UI/终端。
5. `cargo test -p agent_workspace_lib`。
6. **合规复核**:确认 `Cargo.toml` 无 claurst path 依赖;grep 确认 `panes-agent` 源码无逐行照搬 claurst(§3.3 纪律)。

---

## 9. 风险与权衡(vendor vs 干净重构)

| 维度 | 干净重构(本文,推荐) | vendor path 依赖(原方案) |
|---|---|---|
| License | ✅ 保 MIT(clean-room) | ❌ 须转 GPL |
| TLS 冲突(wreq/openssl) | ✅ 不存在 | ❌ 首要阻断 |
| reqwest 版本 | ✅ 单一 0.12 | ⚠️ 0.12/0.13 双版本 |
| 权限死锁 | ✅ 异步 oneshot | ⚠️ 同步 + block_in_place |
| 冗余/TUI | ✅ 全部丢弃 | ❌ 随 workspace 进入 |
| 上游同步 | ❌ 需手动移植 claurst 新行为 | ✅ 可跟上游升级 |
| 实现成本 | ⚠️ 重写 loop+api+tools(有模板,中等) | ✅ 低(复用) |
| 法律确定性 | ⚠️ 依赖 clean-room 纪律(§3.5) | ✅ 确定(但结果是 GPL) |

**净判断**:对 MIT 闭源/半闭源分发的 panes,license 与 TLS 两个硬阻断使 vendor 不可行;干净重构是唯一能同时保 license、去冗余、绕冲突的路径。主要代价是放弃上游自动同步与一定的实现成本——可接受。

---

## 10. 待解决问题

1. **auto-compact 实现深度**:首期软上限告警 vs 完整 compact(参照 claurst `AutoCompactState`)——按实际上下文膨胀频率定。
2. **工具集缺口盘点**:panes 现有(`claude_code_native`/`cuelight_tools`)覆盖了哪些、缺哪些(Glob/Grep/Web/Task?),需逐项核对 `Tool` trait 覆盖。
3. **cost/token 语义对齐**:claurst `CostTracker` 与 panes `TokenUsage`(`events.rs:245`)字段映射,尤其 `reasoning`/`cache_read`/`cache_write`。
4. **多 provider**:claurst 有 `codex_adapter`(`api/src/lib.rs:27`)等;panes 首期仅 Anthropic,多 provider 留待后续。
5. **MCP 可选引入时机**(§4.4)。

---

## 11. 与其他文档的关系

| 文档 | 视角 | 与本文关系 |
|---|---|---|
| `claurst-engine-integration.md` | ACP sidecar 接入(本地/远程、文件桥接) | 走本文 in-process 重构后,**ACP sidecar 与文件桥接均不再需要**(in-process 直接操作本地 FS,与 `claude_code_native` 一致) |
| `vendor/README.md` | vendor 目录说明 | 需补充 claurst 条目(只读参考/GPL/不编译) |
| **本文** | **clean-room 干净重构(in-process,取代 vendor 与 ACP)** | **主线决策版** |
