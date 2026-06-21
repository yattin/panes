# claurst-native 重构 + claude-code-rust 废弃 + CueLight 品牌化方案(待确认版)

> 本文取代早先"把 claurst 整个 workspace 作为 path 依赖 vendor 进 panes"以及"继续扩展 claude-code-rust native"的设想。
> **主线建议:废弃 `vendor/claude-code-rust` 作为 Native 基础,新的 Native 以 claurst 的 agent 行为作为基线做 clean-room 重构**,产出 panes 自有的 MIT 代码,**丢弃 claurst 的 TUI 与冗余 crate**。
> 状态:**主线建议已明确,并已按默认方案开始快速落地**。仍需确认的冲突点集中在产品/合规/迁移边界,不会阻塞当前 MVP 实现,但会决定后续清理、分发和品牌化范围。License 阻断点可由"干净重构"绕过,但前提是严格执行 §3 合规纪律。
> 配套视角:`claurst-engine-integration.md`(ACP sidecar 接入)——若走本文的 claurst-native in-process 重构,ACP sidecar 与文件桥接均不再需要。

---

## 待确认冲突点(先讨论)

| 编号 | 冲突点 | 推荐默认 | 需要确认的问题 |
|---|---|---|---|
| C1 | **主线形态**:claurst-native clean-room in-process vs ACP sidecar vs GPL vendor | 走本文 claurst-native clean-room in-process | 当前已按默认落地。是否最终接受放弃 ACP/file bridge 方案,并明确不把 claurst crate 编译进 panes? |
| C2 | **代码位置**:`crates/panes-agent` 新 crate vs 先放 `src-tauri` 内部模块 | 新建 `crates/panes-agent`,作为 claurst-native 的新 runtime crate | 当前已按默认落地。是否接受新增顶层 `crates/` 目录和 workspace member 作为长期结构? |
| C3 | **claude-code-rust 废弃策略**:保留兼容期 vs 立即移除 | 标记 deprecated,只作为迁移对照;新功能只进 claurst-native | 当前已移除 `claude_code_rs` 编译依赖、旧 runtime 和 `vendor/claude-code-rust`;是否接受后续仅保留 `claude-code-native` engine id alias? |
| C4 | **首期能力范围**:完整 claurst 行为 vs MVP | MVP 只做 Anthropic Messages + 多轮工具 + 权限 + 基础 compact/cost | 当前已按 MVP 快速落地。MCP、插件、hooks、sub-agent、cron、LSP、WebSearch 是否全部后置? |
| C5 | **配置来源**:读取 `~/.claurst`/`claude_code_rs::Settings` vs panes 自己的 engine settings/env | 由 panes 注入 API key/base_url/model,env 仅作 fallback | 当前实现使用 `ANTHROPIC_API_KEY`/`ANTHROPIC_BASE_URL`/`ANTHROPIC_MODEL` fallback。是否接受后续接 panes engine settings,但不读取 `~/.claurst`? |
| C6 | **`vendor/claurst` 处置**:保留 git submodule 参考 vs 移出主仓库 | 开发期可保留 submodule,release/source 包明确排除或标注为独立第三方参考 | 当前 `.gitmodules` 已有 `vendor/claurst`。是否要公开仓库保留它,还是仅本地/独立参考仓保留? |
| C7 | **clean-room 严格度**:直接看 `src-rust` 实现写代码 vs 规格阶段隔离 | 实施前先产出 panes 自有行为规格,实现时只按该规格写 | 是否按“两阶段隔离”执行?如果不能,则 MIT clean-room 结论会变弱。 |
| C8 | **CueLight 品牌化范围**:整 App 改名 CueLight vs 仅影视工作流品牌化 | **已按双 flavor 决策落地**:主 Panes 发行保持不变,CueLight 作为独立 Tauri flavor 分发 | 后续若要把主发行也改名为 CueLight,作为单独 rename/release 迁移处理 |
| C9 | **Native engine id 迁移**:`claude-code-native` 复用/别名 vs 新 `claurst-native` id | 新增 `claurst-native`,旧 `claude-code-native` 作为兼容 alias/迁移入口 | 是否接受历史线程通过 alias 迁移到新 Native,而不是继续维护旧 runtime? |
| C10 | **CueLight 与通用编码能力关系**:CueLight-only 产品 vs CueLight + coding engine | CueLight 作为主品牌,保留通用 coding agent 能力为高级/开发者能力 | 是否需要隐藏 Codex/OpenCode/Claude 等通用引擎,还是作为 secondary engines 保留? |
| C11 | **DDD 架构边界**:按 domain/application/infrastructure 分层 vs 横向模块堆叠 | `panes-agent` 与 Tauri adapter 均按 DDD 分层设计 | 当前已按 DDD 拆分,工具执行也拆到 `native_tools/*`。是否接受该边界作为后续约束? |

> 以上冲突点不影响本文主线判断。当前已按推荐默认快速实现 MVP;仍建议尽快确认 C1-C11,避免后续在迁移、分发或品牌化阶段返工。

---

## 0. 建议摘要(TL;DR)

| 议题 | 决策 | 理由 |
|---|---|---|
| 集成方式 | **clean-room 干净重构**,不 vendor claurst 任何 crate 进编译 | vendor(静态链接 GPL)→ GPL-3.0 传染,panes 须转 GPL;clean-room 产出独立版权作品,可保 MIT |
| Native 基础 | **claurst-native 取代 claude-code-rust native** | claurst 的 agent loop/工具生态更接近目标;`claude-code-rust` 进入废弃,只保留迁移窗口 |
| License | 新代码 **MIT**(随 panes) | 思想/表达二分(Baker v. Selden)+ clean-room 先例(Phoenix v. IBM,claurst 自身亦据此) |
| TUI | **全部丢弃**(tui/cli/commands/buddy/bridge/plugins/acp) | panes 有自己的 Tauri 前端;这些 crate 与核心 agent 循环解耦 |
| HTTP 栈 | **reqwest 0.12 + rustls**(panes 现有),重写 SSE Messages 客户端 | 彻底回避 `wreq`/BoringSSL ↔ `git2`/openssl-sys 符号冲突;统一 reqwest 版本(0.13→0.12) |
| Agent 循环 | **以 claurst `run_query_loop` 行为为基线重写**(compact/tool-budget/cost) | `claude_code_native::send_message` 仅作旧行为兼容对照,不再作为新架构骨架 |
| 权限 | **异步 oneshot**(沿用 panes 已验证审批模式),不照搬 claurst 同步 `PermissionHandler` | 规避同步阻塞 channel + `block_in_place` 死锁(原 vendor 方案 §3.2 难点消失) |
| MCP | **可选,后续单独引入**(`rmcp`) | 非首期必需;panes 已有 cuelight_tools 等扩展机制 |
| 代码组织 | 新增 workspace crate **`panes-agent`**(MIT),引擎适配层 `engines/claurst_native.rs` | `panes-agent` 是 claurst-native runtime;`claude-code-native` 仅保留为兼容 alias |
| 架构风格 | **DDD 分层**:domain / application / infrastructure / interfaces | 让 claurst 行为规格沉淀为 panes 自有领域模型,避免重构变成 API client + 工具脚本拼接 |
| `vendor/claurst/` | **降级为只读参考 submodule**,不参与编译、不入分发包二进制 | 保留作行为参考;公开/release 分发时需明确第三方 GPL 参考身份或排除 |
| CueLight 品牌化 | **与 agent 重构并行推进**,但只碰产品表层与 CueLight 工作流 | 避免把品牌 rename 与底层 clean-room 重构耦合;内部 ID/API 先保持稳定 |

---

## 1. 背景与动机

panes 曾有 in-process Native:`vendor/claude-code-rust`(`claude_code_rs`,**MIT**)作为 path 依赖,`claude_code_native` 引擎直接 `use claude_code_rs::api::{ApiClient, ModelStreamEvent, ...}` / `::tools::{FileReadTool, ...}`,in-process 跑 agent 并产出 panes 的 `EngineEvent`。

这个实现已证明 panes 可以做 in-process engine,但**不再作为未来 Native 的基础**。后续 Native 的行为基线切到 claurst,`claude-code-rust` 已退出编译路径;当前只保留 `claude-code-native` 这个历史 engine id 作为后端 alias,路由到 `claurst-native`。

claurst(`vendor/claurst/`,GPL-3.0)是 Claude Code 行为的 Rust clean-room 重实现,提供等价能力且循环更成熟(`run_query_loop`:auto-compact / tool-budget / cost tracker / 多轮)。panes 希望把 claurst 作为新的 Native 行为基础,但 claurst 的 license 与 TUI 负担使其**不能**像 claude-code-rust 那样直接 vendor/link。

由此引出本文的核心判断:**Native 基础从 claude-code-rust 迁移到 claurst 行为基线;实现方式仍是 clean-room 重写,而非搬运/链接 claurst GPL 源码。**

### 1.1 当前仓库事实

- 根 `Cargo.toml` 当前 workspace members 已只包含 `crates/panes-agent` 与 `src-tauri`;legacy `vendor/claude-code-rust` 已从 workspace 移除。
- `src-tauri/Cargo.toml` 已移除 `claude_code_rs` path 依赖。
- `src-tauri/Cargo.toml` 已新增 `panes-agent = { path = "../crates/panes-agent" }`,作为 `claurst-native` 的 runtime crate。
- `.gitmodules` 已存在 `vendor/claurst` submodule,上游为 `https://github.com/Kuberwastaken/claurst.git`。
- claurst `src-rust/Cargo.toml` 当前为 12-crate workspace,workspace license = GPL-3.0,并同时声明 `reqwest = 0.13` 与 `wreq`/`wreq-util`。
- panes 当前 workspace dependency 为 `reqwest = 0.12` + `rustls-tls`,`src-tauri` 同时依赖 `git2` with `vendored-openssl`。
- 旧 `claude_code_native` runtime 源码已移除;本地 FileRead/List/Search/FileWrite/FileEdit/execute_command/task/CueLight 行为已迁入 `panes-agent` native tools 与 CueLight provider-neutral tool registry。
- CueLight 已有完整业务触点:前端 `src/components/cuelight/*`、右侧面板 `ThreeColumnLayout`/`CueLightPanel`、workspace 绑定 `workspaceStore`、IPC `src/lib/ipc.ts`、后端命令 `src-tauri/src/commands/cuelight.rs`、后端工具 `src-tauri/src/contexts/agent_runtime/cuelight/tools.rs`。
- `src-tauri/src/engines/claurst_native.rs` 已新增并注册 `claurst-native`;UI display name 当前为 `CueLight Agent`。
- 当前默认聊天引擎、onboarding 默认选择与新线程 fallback 已切到 `claurst-native` / `CueLight Agent`;`claude-code-native` 不再出现在 `list_engines`,后端仅作为 legacy alias 路由到 `claurst-native`。

---

## 2. 为什么放弃 vendor、转向干净重构

### 2.1 原 vendor 方案的致命阻断

| 项 | claude-code-rust(当前 legacy vendor) | claurst(原计划 vendor) |
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
| §3.2 同步 `PermissionHandler`(`tools/src/lib.rs:604`)↔ 异步审批,需 `block_in_place` 防死锁 | **改为异步 oneshot**(沿用 panes 已验证审批模式),难点消失 |
| §3.3 12-crate workspace vendor + workspace 级依赖协调 | 不引入 claurst workspace,只写 panes 需要的子集 |
| TUI/插件/companion/远程桥等冗余代码随 vendor 进来 | 整体丢弃(见 §4) |

**结论:干净重构在 license、TLS 冲突、版本协调、权限死锁、冗余五个维度上全面优于 vendor。** 代价是要重写 agent 循环 + SSE 客户端 + 工具集;claurst `run_query_loop` 提供新的 Native 行为基线,`claude_code_native` 只提供 panes 事件/审批兼容经验,工作量可控(见 §9 权衡)。

### 2.3 为什么废弃 claude-code-rust Native

`claude-code-rust` 已经完成了"证明 panes 可以内嵌 Native agent"的阶段性任务,但不适合继续作为长期基础:

| 问题 | 影响 | 新方案 |
|---|---|---|
| Agent loop 能力较薄 | 多轮、compact、tool-budget、cost、恢复策略需要持续补丁式扩展 | 以 claurst `run_query_loop` 行为为目标一次性重构 |
| 工具/消息类型绑定 `claude_code_rs` | 新工具层、CueLight 展示、审批与 sandbox 难以做成 panes 自有抽象 | `panes-agent` 定义 panes 自有 Message/Tool/Event/Permission |
| 配置依赖旧 Settings 路径 | 与 panes engine settings、CueLight 品牌化和多环境分发不一致 | 配置全部由 panes 注入,env 只作 fallback |
| 继续维护会形成双 Native | `claude-code-native` 与 claurst-native 并行会增加测试、UI、迁移负担 | `claude-code-native` 冻结,parity 后移除 |
| 行为基线不是 claurst | 后续还要反复把 claurst 行为移植进旧结构 | 直接把 claurst 行为设为新 Native 基线 |

因此本文不是在 `claude-code-rust` 上继续"增强 native",而是把它作为 legacy 兼容层退出;新的 Native 基础是 claurst 行为基线 + panes 自有 clean-room runtime。

---

## 3. License / 合规决策(关键章节)

### 3.1 决策

> **基于 claurst 的思想与行为,在 panes 内 clean-room 重写;新代码以 MIT 发布;不静态链接 claurst,不把 claurst GPL 源码作为 panes 产品组成部分分发。**

### 3.2 法律依据

- **思想/表达二分**(*Baker v. Selden*, 1879):版权保护*表达*(expression),不保护*思想、方法、行为*。agent 循环算法、工具契约、事件模型属于行为/方法层面,不受版权保护。
- **clean-room 先例**(*Phoenix Technologies v. IBM*, 1984,BIOS clean-room):通过"规格→独立实现"两阶段隔离,产出独立版权作品,不构成衍生作品。claurst 自身即据此从 Claude Code TypeScript 规格重写为 Rust(见 `vendor/claurst/README.md` "Important Notice")。
- 因此:**参照 claurst 的行为独立重写,产出的是 panes 的独立版权作品,不因链接/复制触发 GPL copyleft,可保持 MIT。** 但该结论依赖事实执行:不复制表达层代码/文档、不把 claurst crate 编译或链接进 panes。

### 3.3 必须遵守的 clean-room 纪律(否则结论不成立)

clean-room 的安全性**完全取决于执行纪律**,不是"看了 GPL 代码就能随便重写":

1. **两阶段隔离**:
   - **规格阶段**:从 claurst 的*行为*提炼接口契约与算法描述,并产出一份 panes 自己的规格文档。可参考公开 API 行为、README/文档层说明与可观察行为;若参考 `vendor/claurst/spec/`,只能提炼行为事实,不能复制其文字。
   - **实现阶段**:实现者按 panes 自有规格**独立写 Rust**,目标是"按行为规格实现",而非"翻译 claurst 源码"。
2. **避免实质性相似**(*substantial similarity* 是衍生作品的判定核心):不得逐行/逐函数翻译 claurst 的 `src-rust/`。命名、结构、注释应体现 panes 自身设计,而非照搬。
3. **表达层只读、不复制**:`vendor/claurst/src-rust/` 仅作"理解行为"的只读参考;任何代码片段都不进入 panes 仓库(连"改改变量名"也不行)。
4. **spec 文本不可复制**:`vendor/claurst/spec/*.md` 文本本身是文字作品受版权保护;只能"按其描述的行为实现",不能复制其文字或表格。
5. **声明与致谢**:新 crate 头部加 clean-room 声明——"Behavior inspired by claurst (GPL-3.0 by kuberwastaken); clean-room reimplementation, no source code derived."
6. **审查记录**:实施 PR 中保留一份 checklist,记录实现文件由 panes 自有规格驱动,并 grep/人工抽查无 claurst 源码片段、注释文本、测试文本迁移。

### 3.4 合规清单(分发前逐项确认)

- [x] `panes-agent` crate 与引擎层 license = MIT,头含 clean-room 声明 + claurst 致谢
- [x] panes `Cargo.toml` **不**出现任何 `path = "../vendor/claurst/..."` 依赖(不编译、不链接)
- [x] 分发包二进制中不含 claurst GPL 目标码:新增 `pnpm audit:claurst-clean-room`,基于 Cargo metadata/manifest/vendor README 审计确认 `vendor/claurst` 未进入 build graph
- [x] `vendor/claurst/` 若保留在公开仓库中,必须明确标注"GPL-3.0 第三方只读参考 submodule,非 panes 产品组成部分";release source/binary 包可选择排除该 submodule
- [x] `vendor/README.md` 补充 claurst 条目:定位=行为参考、license=GPL-3.0、不参与编译

### 3.5 诚实声明(不确定性)

- clean-room 是**法律上可行的路径**,但"是否构成衍生作品"最终是**事实问题**(看实质相似性 + 接触),需个案判断。本方案给出的是工程上最稳妥的姿势,**不等同于法律意见**;面向公开/商业分发前建议由法务复核重写产出与 claurst 的相似度。
- 若团队无法保证 §3.3 纪律(例如实现者不可避免地逐行参照),则应退回备选:① 接受 panes GPL 化;② 向 claurst 作者(kuberwastaken)寻求商业/dual 授权;③ 放弃 claurst、改用其他 MIT agent。三者均非本文主线。

---

## 4. 范围:保留重写 vs 丢弃

claurst workspace 共 12 crate(`vendor/claurst/src-rust/Cargo.toml:3-16`)。按下表处置:

### 4.1 保留并重写(panes 自有 `panes-agent`)

| claurst 来源(行为参照) | 重写目标 | 说明 |
|---|---|---|
| `query`(`run_query_loop` `query/src/lib.rs:703`,`QueryEvent` `:450`,`QueryConfig` `:76`) | agent loop | compact / tool-budget / cost / 多轮;作为 claurst-native 行为基线 |
| `api`(`AnthropicClient` `api/src/lib.rs:457`) | api client | reqwest 0.12 + rustls + SSE Messages 流;**不带 wreq/BoringSSL impersonation** |
| `tools`(`all_tools()` `tools/src/lib.rs:525`,`ToolContext` `:278`,`Tool` trait) | 工具集 | 以 claurst tool contract 为基线重写;CueLight 工具作为 panes 扩展注入 |
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

### 5.1 DDD 代码组织

推荐新增 workspace 成员 crate `panes-agent`(MIT),作为 claurst-native runtime。该选择需要确认 C2:当前仓库尚无 `crates/` 目录,因此这是新增结构,不是沿用既有布局。

`panes-agent` 必须按 DDD 分层,而不是按技术文件横向堆 `api.rs/loop.rs/tools.rs`:

- **domain**:纯领域模型与规则。表达 agent 会话、消息、内容块、工具调用、权限请求、token/cost、loop policy、compact policy。不得依赖 reqwest、tokio channel、Tauri、文件系统、环境变量。
- **application**:用例编排。实现 `RunAgentTurn`/`ContinueToolLoop`/`RequestPermission`/`CompactConversation` 等 application service,依赖 domain 与端口 trait。
- **infrastructure**:外部技术适配。Anthropic Messages SSE client、native tool executor、command runner、filesystem、token estimator、clock/id generator、环境配置读取 fallback。
- **interfaces**:crate 对外 API/DTO/事件转换边界。向 Tauri engine 暴露稳定 API,并隔离 domain 类型与前端/EngineEvent 细节。
- **engine adapter**:`src-tauri/src/engines/claurst_native.rs` 是 Tauri 侧 anti-corruption layer,只负责 ThreadScope/SandboxPolicy/EngineEvent/ApprovalRequested 转换,不承载 agent 领域规则。

```
panes/
├─ Cargo.toml                  # workspace.members 加 "crates/panes-agent"
├─ crates/
│  └─ panes-agent/             # 新增,MIT,clean-room 重写
│     ├─ Cargo.toml            # 仅依赖 panes workspace 依赖(reqwest 0.12/rustls/tokio/serde...)
│     └─ src/
│        ├─ lib.rs             # 只 re-export interfaces 层 API
│        ├─ domain/
│        │  ├─ conversation.rs # AgentMessage / ContentBlock / ToolUse / ToolResult
│        │  ├─ policy.rs       # LoopPolicy / SandboxRule / CompactPolicy
│        │  ├─ permission.rs   # PermissionRequest / PermissionDecision
│        │  ├─ telemetry.rs    # TokenUsage / CostEstimate
│        │  └─ error.rs        # AgentError / Recoverability
│        ├─ application/
│        │  ├─ run_agent_turn.rs
│        │  ├─ tool_loop.rs
│        │  ├─ compact_conversation.rs
│        │  └─ ports.rs        # ModelClient / ToolExecutor / PermissionGateway / EventSink
│        ├─ infrastructure/
│        │  ├─ anthropic_sse.rs
│        │  ├─ native_tools/
│        │  ├─ token_estimator.rs
│        │  └─ runtime_config.rs
│        └─ interfaces/
│           ├─ agent_runtime.rs # public AgentRuntime / run_agent_turn facade
│           ├─ events.rs        # AgentEvent DTO
│           └─ tool_specs.rs    # public tool definitions/schema DTOs
└─ src-tauri/src/
   └─ engines/
      └─ claurst_native.rs     # 新增:Engine trait 适配层(AgentEvent → EngineEvent)
```

依赖方向:

```text
domain <- application <- infrastructure
domain <- interfaces
application <- interfaces
src-tauri engine adapter -> panes-agent::interfaces
```

禁止方向:

- `domain` 不得引用 `infrastructure`、`interfaces`、Tauri、reqwest、tokio process。
- `application` 不得构造 HTTP request 或直接访问 FS/command;只能调用 ports。
- `infrastructure` 可以实现 application ports,但不能把 provider-specific DTO 泄漏回 domain。
- `src-tauri` adapter 不得直接操作 `panes-agent::domain` 内部结构;通过 `interfaces` facade 与 DTO 交互。

`panes-agent` 对外暴露的公共表面(自定,不照搬 claurst 命名):

```rust
// interfaces 层 facade:Tauri adapter 只依赖这里。
pub struct AgentRuntime<P: RuntimePorts> { /* application services + ports */ }
impl<P: RuntimePorts> AgentRuntime<P> {
    pub async fn run_turn(&self, command: RunTurnCommand) -> anyhow::Result<AgentOutcome>;
}

pub struct RunTurnCommand {
    pub conversation_id: String,
    pub messages: Vec<AgentMessageDto>,
    pub system_context: SystemContextDto,
    pub policy: RuntimePolicyDto,
}

// application::ports:基础设施通过 trait 注入。
#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn stream(&self, request: ModelRequest) -> anyhow::Result<ModelEventStream>;
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, call: ToolCall) -> ToolExecutionResult;
}

#[async_trait]
pub trait PermissionGateway: Send + Sync {
    async fn request(&self, req: PermissionRequest) -> PermissionDecision;
}
```

### 5.2 API client(无 wreq)

- 位于 `infrastructure/anthropic_sse.rs`,实现 application port `ModelClient`。
- 用 panes workspace 的 `reqwest 0.12`(`rustls-tls`,`Cargo.toml:14`)+ tokio SSE,实现 Anthropic Messages `/v1/messages` 流式。
- 解析标准 SSE 事件:`message_start` / `content_block_start` / `content_block_delta`(text_delta / input_json_delta / thinking_delta)/ `message_delta`(usage、stop_reason)/ `message_stop` / `error`。
- **不做 TLS 指纹伪装**(claurst 的 `bun_tls`/Bun profile 不引入)。
- API key / model / base_url 由 panes 注入(从 panes 的 engine settings),环境变量仅作 fallback;不读 `~/.claurst`,也不依赖 `claude_code_rs::config::Settings::load()`。
- provider-specific SSE/JSON DTO 只存在于 infrastructure,转换成 domain/application 的 `ModelStreamEvent` 后再向上流动。

### 5.3 Agent 循环

行为基线为 claurst `run_query_loop`(`query/src/lib.rs:703`)。在 DDD 中它落到 application 层的 `RunAgentTurn`/`ToolLoop` 用例,而不是一个巨大的 procedural function。旧 `claude_code_native` runtime 已删除;`claurst_native` adapter 现在直接消费 `panes-agent` facade 并负责 `EngineEvent`、审批与取消语义转换。集成:

- **多轮工具循环**:模型输出 tool_use → 经权限审批 → 执行工具 → tool_result 回灌 → 继续直到 `end_turn`/`stop_reason` 或达 `max_turns`。
- **auto-compact**:domain 定义 `CompactPolicy`,application 调 `CompactConversation` 用例;首期 infrastructure 可用简化 token estimator。
- **cost tracker**:domain 定义 `TokenUsage`;Anthropic infrastructure 根据模型 tier 估算 `cost_usd`,未知模型不估价。
- **tool-budget / max_turns**:domain `LoopPolicy` 统一表达,application 执行,超额 emit `AgentError{recoverable}`。
- **取消**:`CancellationToken` 贯穿 application loop 与 tool executor,命令运行中取消会 kill 子进程。

### 5.4 工具集

工具层以 **claurst tool contract + panes 安全边界** 为基线重写,不再从 `claude-code-rust` 扩展:

- 旧 `claude_code_native::execute_native_tool` 已删除;等价行为已迁入 `panes-agent/src/infrastructure/native_tools/*` 与 `cuelight/tools.rs` 的 provider-neutral tool spec registry。
- `engines/cuelight_tools.rs` 已有领域工具。
- 推荐新增 panes 自有 `infrastructure/native_tools/`,实现 application port `ToolExecutor`;由 `claurst_native` adapter 注入 runtime。
- 首期工具范围建议:FileRead/ListFiles/Search/FileWrite/FileEdit/ExecuteCommand/TaskManagement/CueLight。Glob/Grep 可合并到 Search 行为;WebSearch、Agent/Task 子代理、LSP、Notebook、Computer Use 后置。
- 工具执行结果一次性返回(对齐 claurst `ToolEnd` 行为),映射到 `EngineEvent::ActionCompleted`。如需终端实时输出,后续再补 `ActionOutputDelta`。
- CueLight 工具作为 infrastructure adapter 注入,不进入通用 domain;domain 只认识 `ToolCall`/`ToolResult`/`ToolSpec`。

### 5.5 权限(异步 oneshot,关键简化)

沿用 panes 已验证的异步审批模式,但以 application port `PermissionGateway` 表达,而非 claurst 的同步 handler:

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

`panes-agent` 的 `PermissionGateway::request` 为 `async fn`,Tauri adapter 用 oneshot 实现该 port。**完全规避原 vendor 方案 §3.2/§4.4 的同步阻塞 + `block_in_place` 死锁问题。**

### 5.6 事件映射(`AgentEvent`/`QueryEvent` → `EngineEvent`)

`claurst_native.rs` 适配层把 `panes-agent` 的 `AgentEvent` 翻译为 panes `EngineEvent`(`engines/events.rs:113`)。映射(参照 claurst `QueryEvent` `query/src/lib.rs:450`):

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

新增 `engines/claurst_native.rs`,实现 `Engine` trait(`engines/mod.rs:426`)。结构沿用 panes engine contract,不沿用 `claude_code_rs` runtime:

- `ClaurstNativeEngine { threads: Arc<Mutex<HashMap<String, ThreadState>>>, pending_approvals: Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>> }`
- `start_thread`(`:433`):建 `AgentClient`、配 `ToolContext`(working_dir=`scope.root`、permission_mode、异步 `PermissionHandler`),存 `ThreadState{history, root_path, ...}`
- `send_message`(`:441`):spawn 调 `panes_agent::run_agent_loop`,bridge task 把 `AgentEvent` → `EngineEvent` 推 `event_tx`
- `respond_to_approval`(`:1451` 模式):唤醒 oneshot
- `interrupt`(`:1470` 模式):触发 `CancellationToken`

能力注册(`engines/mod.rs`):

```rust
const CLAURST_NATIVE_CAPABILITIES: EngineCapabilities = EngineCapabilities {
    permission_modes: &["restricted", "standard", "trusted"],
    sandbox_modes: &["read-only", "workspace-write"],
    approval_decisions: &["accept", "decline", "accept_for_session"],
};
// capabilities_for_engine(:151) 加: "claurst-native" => CLAURST_NATIVE_CAPABILITIES,
// 兼容期可让 "claude-code-native" 返回同一 capabilities,但 runtime 路由到 claurst-native。
```

`EngineManager`(`:469`)加 `claurst_native: Arc<ClaurstNativeEngine>` + `new()`/`with_db()` 初始化 + 各 `match engine_id` 路由分支(`ensure_engine_thread` `:776` / `send_message` `:813` / `steer` `:869` / `respond_to_approval` `:901` / `interrupt` `:914` / `archive` `:929`)。前端引擎选择 UI 自动从 `list_engines` 获取,无需硬编码。

兼容期路由策略:

- 新线程默认 `engine_id = "claurst-native"`。
- 历史 `engine_id = "claude-code-native"` 线程先路由到 `claurst_native` 兼容入口;旧 engine id 保留为后端 alias,不再作为新建线程入口暴露。
- `ClaudeCodeNativeEngine`、`claude_code_rs` path 依赖、`vendor/claude-code-rust` workspace member 与旧 runtime 源码已移除。

### 5.8 claude-code-rust 废弃与迁移计划

`vendor/claude-code-rust` 不再是 Native 的基础,进入明确废弃流程:

| 阶段 | 状态 | 动作 |
|---|---|---|
| D0 当前 | complete | 旧 `claude-code-native` runtime 已退出编译路径 |
| D1 claurst-native MVP | complete | 新线程默认走 `claurst-native`;旧 engine id 不再作为新建入口暴露 |
| D2 parity | partial | 历史 `claude-code-native` 路由到 claurst-native 兼容层;持久化 metadata/迁移提示仍待补 |
| D3 removal | complete | 已删除 `claude_code_rs` path 依赖、`vendor/claude-code-rust` workspace member、旧 engine runtime 与 vendor tree |

废弃判定条件:

- claurst-native 覆盖文本流、工具循环、审批、取消、上下文压缩、CueLight 工具、基础 token usage。
- `claude-code-native` 关键回归用例在 claurst-native 上有等价覆盖。
- 历史线程可打开/继续,或有明确的一次性迁移策略。
- release notes 明确说明 Native 基础已从 claude-code-rust 切换到 claurst-native。

### 5.9 CueLight 品牌化并行线

CueLight 品牌化与 claurst-native clean-room 重构**并行推进,但不共享同一批底层改动**。品牌化的目标是让用户第一眼感知这是 CueLight 影视创作工作台,而不是把所有内部命名、DB 字段、IPC 命令和 engine id 立即重命名。

**首期推荐边界**

- **对外品牌**:应用标题、欢迎页/token gate、workspace 创建/绑定流程、右侧业务面板、发行物名称/图标/描述统一强化 CueLight。
- **产品定位**:CueLight 是主工作流;通用 coding engines 保留为底层 agent 能力或开发者/高级入口。
- **内部稳定性**:保留 `cuelight_*` API、`CueLight*` 类型、workspace binding 字段;新 Native 内部 ID 用 `claurst-native`,UI display name 可品牌化为 CueLight Agent。
- **样式系统**:现有 `.cuelight-*` CSS 命名保留;如需要 app-wide brand token,新增 `--brand-*`/`--cuelight-*` 变量,不做全量 class rename。

**不建议首期做**

- 不把 crate、DB migration、IPC command、engine id 从 `panes`/`native` 全量改名为 `cuelight`。
- 不把 `panes-agent` crate 改名为 `cuelight-agent`;该 crate 是通用 agent runtime,品牌层由 Tauri/前端决定。
- 不在同一 PR 中同时大规模改 UI branding、agent loop、tool executor 与分发配置;可以同一里程碑并行,但 PR 要拆开。

**品牌化实施面**

| 面向 | 首期动作 | 备注 |
|---|---|---|
| App shell | `App.tsx`/页面标题/空状态/默认入口强调 CueLight 工作台 | 保留 Panes 内部模块名 |
| Onboarding | `OnboardingWizard` 默认引导选择 CueLight 项目与本地 workspace | CueLight 项目绑定成为主路径 |
| Workspace | settings/nav 中 CueLight 从普通 section 升级为项目上下文 | 不改绑定存储结构 |
| Chat engine | engine display name 可改为 "CueLight Agent" 或 "Native Agent" | engine id 不改,避免历史线程迁移 |
| CueLight panel | 右侧 overview/storyboard/assets 作为核心首屏信号 | 现有组件可直接增强 |
| Distribution | `tauri.conf.json`、图标、安装包名、README/landing copy | 需确认是替换 Panes 还是提供 CueLight flavor |
| Backend | `cuelight_proxy`/`cuelight_tools` 文案、错误消息、日志统一 | 函数名/API 不改 |

**与 agent 重构的接口约束**

- `panes-agent` 只暴露通用 tool/event/permission 抽象;CueLight 工具通过 `native_tool_executor` 或 adapter 注入。
- CueLight system prompt 由 engine 适配层或 tool context 注入,不要写死在 `panes-agent` 核心循环。
- `ActionStarted.display_label/display_subtitle` 继续作为 CueLight 业务展示层,避免让模型工具名直接暴露给影视用户。
- 品牌化验证以用户路径为准:启动 → token → 绑定项目 → 查看项目/分镜/资产 → 聊天中调用 CueLight 工具。

### 5.10 DDD 边界与命名规则

为避免重构后出现新的耦合层,实现时按以下规则审查:

| 层 | 可以依赖 | 禁止依赖 | 命名倾向 |
|---|---|---|---|
| domain | std、serde(必要时)、纯 value object | reqwest、tokio process、Tauri、环境变量、DB、FS | `Conversation`, `ToolCall`, `LoopPolicy`, `PermissionDecision` |
| application | domain、application ports、async trait | provider DTO、Tauri DTO、具体工具实现、文件系统细节 | `RunAgentTurn`, `ExecuteToolLoop`, `CompactConversation` |
| infrastructure | domain/application ports、reqwest、tokio、FS/command | Tauri `EngineEvent`、前端 DTO | `AnthropicSseClient`, `NativeToolExecutor`, `TokenEstimator` |
| interfaces | domain/application DTO 映射、facade | provider-specific DTO 泄漏、业务规则实现 | `AgentRuntime`, `RunTurnCommand`, `AgentEvent` |
| Tauri adapter | panes engine types、interfaces facade | domain 内部模块、provider DTO | `ClaurstNativeEngine`, `EngineEventMapper` |

测试也按层拆:

- domain tests:纯同步单测,覆盖 policy、message/tool result 组装、错误 recoverability。
- application tests:用 fake `ModelClient`/`ToolExecutor`/`PermissionGateway`,覆盖多轮循环、审批、取消、预算、compact。
- infrastructure tests:覆盖 SSE parser、命令/文件工具 sandbox、CueLight tool adapter。
- adapter tests:覆盖 `AgentEvent` → `EngineEvent`、legacy `claude-code-native` alias、thread metadata 迁移。

---

## 6. 关键文件

**新增**
- `crates/panes-agent/src/domain/`(MIT,clean-room 领域模型:conversation/policy/permission/telemetry/error)
- `crates/panes-agent/src/application/`(用例:run_agent_turn/tool_loop/compact_conversation + ports)
- `crates/panes-agent/src/infrastructure/`(Anthropic SSE/native tools/token estimator/runtime config)
- `crates/panes-agent/src/interfaces/`(public facade/DTO/events/tool specs)
- `src-tauri/src/engines/claurst_native.rs`(Engine trait 适配层)

**改动**
- `Cargo.toml`(workspace.members 加 `crates/panes-agent`)
- `src-tauri/Cargo.toml`(`panes-agent = { path = "../crates/panes-agent" }`)
- `src-tauri/src/engines/mod.rs`(`claurst-native` 注册 + `claude-code-native` alias 路由 + `capabilities_for_engine` 分支)
- `src-tauri/Cargo.toml`/根 `Cargo.toml` 已移除 `claude_code_rs` 与 `vendor/claude-code-rust`
- `src-tauri/src/contexts/agent_runtime/claude_code_native/mod.rs` 已删除
- `vendor/README.md` 已更新:说明 `vendor/claude-code-rust` 已移除,active native runtime 位于 `crates/panes-agent`
- CueLight branding: `src/App.tsx`、`src/components/cuelight/*`、`src/components/onboarding/OnboardingWizard.tsx`、`src/components/workspace/*Settings*`、`src/components/layout/ThreeColumnLayout.tsx`、`src/globals.css`、`src-tauri/tauri.conf.json`、README/landing copy

**参考(只读,不改动、不依赖)**
- `vendor/claurst/src-rust/`(行为参考)、`vendor/claurst/spec/`(规格参考)

**迁移对照(只读/少改)**
- 已移除旧 `claude_code_native` 源码;后续迁移对照以 `panes-agent` 测试和 `claurst_native` adapter 测试为准。

---

## 7. 实施步骤

0. **确认 C1-C11**:确认主线、代码位置、`claude-code-rust` 废弃策略、首期能力、配置来源、`vendor/claurst` 处置、clean-room 执行纪律、CueLight 品牌范围、Native engine id 迁移、通用引擎可见性与 DDD 边界。
1. **规格先行**:基于公开行为与可观察行为,写一份 panes 自有 `panes-agent` 接口/算法规格(clean-room 规格阶段,见 §3.3);不得复制 claurst `spec/*.md` 原文。
2. **冻结旧 Native**:给 `claude-code-native` 标记 deprecated,停止新增功能;当前已进一步移除旧 runtime,仅保留 engine id alias。
3. 建 `crates/panes-agent`(MIT,含 clean-room 声明头),workspace 注册,先建立 DDD 目录与依赖边界。
4. **domain first**:实现 `Conversation`/`AgentMessage`/`ContentBlock`/`ToolCall`/`ToolResult`/`LoopPolicy`/`PermissionDecision`/`TokenUsage` 等纯领域模型。
5. **application ports**:定义 `ModelClient`/`ToolExecutor`/`PermissionGateway`/`EventSink`/`ConversationStore` 等端口。
6. **application use cases**:实现 `RunAgentTurn`、`ExecuteToolLoop`、`CompactConversation`,使用 fake ports 写用例测试。
7. **infrastructure adapters**:实现 `AnthropicSseClient`(reqwest 0.12 + rustls)、`NativeToolExecutor`、`TokenEstimator`、runtime config 注入。
8. **interfaces facade**:实现 `AgentRuntime`、`RunTurnCommand`、`AgentEvent`、tool specs DTO,作为 Tauri adapter 的唯一入口。
9. `engines/claurst_native.rs`:Engine trait 实现 + `AgentEvent`→`EngineEvent` 映射 + oneshot permission gateway。
10. `engines/mod.rs`:`EngineManager` 字段 + `capabilities_for_engine` + list/health/start/send/respond/interrupt/archive 路由分支注册;旧 `claude-code-native` alias 路由到新 runtime。
11. **迁移清理**:已删除 `claude_code_rs` path 依赖、`vendor/claude-code-rust` workspace member、旧 runtime 和旧 integration docs。
12. **CueLight 品牌化 PR-A(UI/文案)**:`App`/onboarding/workspace settings/CueLight panels/empty states/token gate,强化 CueLight 主路径。
13. **CueLight 品牌化 PR-B(发行层)**:`tauri.conf.json`、图标、安装包名、README/landing copy;若采用 flavor 策略,明确 Panes 与 CueLight 包名/配置差异。
14. `vendor/README.md` + release/source 包策略 + 合规清单(§3.4)逐项确认。

---

## 8. 验证

1. `cargo build -p panes-agent` 与 `cd src-tauri && cargo build` —— 验证无 wreq、reqwest 单版本、无符号冲突。
2. `cargo test -p panes-agent` —— 覆盖 SSE parser、tool loop、max_turns、取消、token usage。
3. **DDD 边界检查**:domain tests 不启用 reqwest/tokio process/Tauri;application tests 使用 fake ports;infrastructure/provider DTO 不出 infrastructure。
4. `cargo test -p agent_workspace_lib` —— 覆盖 claurst-native engine routing、legacy alias、native tool executor 与 CueLight 工具注入。
5. 单测:`RunAgentTurn`/`ExecuteToolLoop` 断言 `AgentEvent`/`EngineEvent` 序列(`TurnStarted → (TextDelta|ActionStarted|ApprovalRequested|ActionCompleted)* → TurnCompleted`)。
6. 权限:触发命令/写工具,验证 `ApprovalRequested`(`events.rs:155`)↔ `respond_to_approval` 闭环,并验证 `accept_for_session` 仅限当前 engine thread。
7. 端到端:`pnpm tauri:dev`,选 Claurst Native/CueLight Agent 发 prompt,观察前端事件流/审批 UI/终端。
8. CueLight 品牌化手测:`pnpm tauri:dev` 后走 token gate → workspace 创建 → 绑定 CueLight 项目 → overview/storyboard/assets → chat 调 CueLight 工具,确认首屏品牌、文案和工具展示一致。
9. 前端回归:`npm test -- --run` 或项目既有 Vitest 命令,至少覆盖 CueLight repository/store/workspace binding 与 action display。
10. **废弃复核**:parity 后确认 `Cargo.toml`/`src-tauri/Cargo.toml` 不再依赖 `claude_code_rs`,旧 `claude-code-native` 只剩兼容 alias 或已移除。
11. **合规复核**:运行 `pnpm audit:claurst-clean-room`,确认 Cargo metadata/manifest/vendor README 均未把 `vendor/claurst` 带入 build graph;再 grep/人工抽查确认 `panes-agent` 源码、注释、测试无 claurst 表达层复制(§3.3 纪律)。

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
| claude-code-rust 去留 | ✅ parity 后移除旧依赖 | ❌ 继续维护两套 Native |
| 实现成本 | ⚠️ 重写 loop+api+tools(中等) | ✅ 低(复用) |
| 法律确定性 | ⚠️ 依赖 clean-room 纪律(§3.5) | ✅ 确定(但结果是 GPL) |

**净判断**:对 MIT 闭源/半闭源分发的 panes,license 与 TLS 两个硬阻断使 vendor 不可行;干净重构是唯一能同时保 license、去冗余、绕冲突的路径。主要代价是放弃上游自动同步与一定的实现成本——可接受。

---

## 10. 待解决问题

确认 C1-C11 后,仍需在实现过程中跟踪:

1. **auto-compact 实现深度**:首期使用简化压缩,后续是否补完整 compact 状态机,取决于真实上下文膨胀频率。
2. **native tool executor 边界**:服务 `claurst_native`,同时用旧 `claude-code-native` 关键用例做迁移回归;不要求继续维护旧 runtime。
3. **工具集缺口盘点**:首期 FileRead/ListFiles/Search/FileWrite/FileEdit/ExecuteCommand/TaskManagement/CueLight 是否足够;Glob/Grep/Web/Agent/LSP 是否后置。
4. **cost/token 语义对齐**:Anthropic usage 与 panes `TokenUsage`(`events.rs:245`)字段映射,尤其 `reasoning`/`cache_read`/`cache_write`;`cost_usd` 需要模型价格表。
5. **多 provider**:claurst 有 `codex_adapter` 等多 provider 能力;本文建议首期仅 Anthropic,多 provider 后续单独立项。
6. **MCP 可选引入时机**(§4.4):首期不做,但要确保 `panes-agent` 的工具 trait 不阻碍后续 MCP wrapper。
7. **CueLight flavor 策略**:是替换 Panes 产品名,还是同时维护 Panes/CueLight 两套发行物;这会影响 bundle id、自动更新、安装路径和 release notes。
8. **通用引擎可见性**:CueLight 品牌化后是否默认隐藏 Codex/OpenCode/Claude,或只在高级设置中显示。
9. **品牌资产**:图标、色彩、启动图、landing copy 是否已有最终素材;没有素材时先做文案/结构品牌化,视觉资产后补。

---

## 11. 与其他文档的关系

| 文档 | 视角 | 与本文关系 |
|---|---|---|
| `claurst-engine-integration.md` | ACP sidecar 接入(本地/远程、文件桥接) | 走本文 claurst-native in-process 重构后,**ACP sidecar 与文件桥接均不再需要**(in-process 直接操作本地 FS) |
| `vendor/README.md` | vendor 目录说明 | 需补充 claurst 条目(只读参考/GPL/不编译) |
| **本文** | **claurst-native clean-room 重构 + claude-code-rust 废弃 + CueLight 品牌化并行线** | **主线建议/待确认版** |

---

## 12. 当前实现进度

已开始落地 claurst-native runtime 的早期实现。前几条按 TDD tracer bullet 推进;当前阶段按最新要求暂停 TDD,改为 DDD 模块化快速实现,后续再补覆盖:

- [x] 新增 workspace member `crates/panes-agent`
- [x] 建立 DDD 目录: `domain/`、`application/`、`infrastructure/`、`interfaces/`
- [x] 通过 public `AgentRuntime` facade 跑通纯文本 turn
- [x] 通过 public `AgentRuntime` facade 跑通最小工具调用回路: `ToolUse` → `ToolExecutor` → `ToolResult` → follow-up model request
- [x] 实现 `NativeToolExecutor` 的首个真实工具切片:`read_file` 读取 workspace 内相对路径文件,并通过 tool result 回填模型
- [x] 实现 `NativeToolExecutor` 的 `list_files` 切片:列出 workspace 内相对目录的一层条目,稳定排序,目录以 `/` 结尾
- [x] 实现 `NativeToolExecutor` 的 `search` 切片:在 workspace 内递归搜索普通文本子串,输出 `relative/path:line:content`
- [x] 将 `NativeToolExecutor` 从单文件拆成 `native_tools/` adapter 模块:路径安全、读文件、列目录、搜索、写文件、编辑、命令、任务分别独立文件
- [x] 快速补齐 `file_write`/`file_edit`/`execute_command`/`task_management` 的最小实现
- [x] 在 `panes-agent` application ports 中定义 `PermissionGateway`,并让 `execute_command` 通过该端口决策后再执行
- [x] 新增 `src-tauri/src/engines/claurst_native.rs` Engine 适配骨架,注册 `claurst-native` engine id、capabilities、models、health、start/send/respond/interrupt/archive 路由
- [x] `src-tauri/Cargo.toml` 接入 `panes-agent` path 依赖
- [x] 新增 `panes-agent::infrastructure::anthropic` 最小 Anthropic Messages adapter:reqwest 0.12 + rustls、`ANTHROPIC_API_KEY`/`ANTHROPIC_BASE_URL`/`ANTHROPIC_MODEL` env 注入、解析 text/tool_use SSE 事件
- [x] Anthropic Messages SSE adapter 已改为逐 chunk 增量解析,`text_delta` 可实时进入 `ModelStreamEvent::TextDelta`;流中错误通过 `ModelStreamEvent::Error` 冒泡到 application 层
- [x] `claurst-native` send_message 已接入 `AnthropicMessagesClient` 与 native tool executor
- [x] `claurst-native` 的 `TauriPermissionGateway` 已接入 panes `ApprovalRequested` / `respond_to_approval` oneshot 闭环,并支持 `accept_for_session`
- [x] `claurst-native` 已加载 workspace root 对应的 CueLight binding,并把 `cuelight_*` tool call 路由到既有 `execute_cuelight_tool`
- [x] `claurst-native` 已在存在 CueLight binding 时把既有 OpenAI function-style CueLight tool definitions 转换为 Anthropic `tools` schema
- [x] native tool schema 已从 Anthropic adapter 迁出到 `infrastructure/native_tools/specs.rs`,Anthropic adapter 仅做 provider schema 转换
- [x] CueLight tool schema 已新增 provider-neutral `build_cuelight_tool_specs()` registry;旧 OpenAI function schema 由兼容函数保留,`claurst-native` 直接消费 `ToolSpec` 转 Anthropic schema
- [x] `EngineManager::with_db` 已把数据库注入 `ClaurstNativeEngine`,用于加载 CueLight thread context
- [x] 前端 `ChatEngineId` 已加入 `claurst-native`,ChatPanel 默认选择切到 `claurst-native`
- [x] onboarding 默认聊天引擎与新线程 fallback 已切到 `claurst-native` / `CueLight Agent`
- [x] onboarding 文案已做 CueLight 品牌化入口(en/zh-CN/pt-BR),同时保留内部 Panes/API 命名
- [x] 新增 CueLight Tauri flavor overlay:`src-tauri/tauri.cuelight.conf.json`,设置 `productName=CueLight`、`identifier=com.panes.cuelight`、窗口标题 `CueLight`,并新增 `pnpm tauri:dev:cuelight` / `pnpm tauri:build:cuelight`
- [x] CueLight Tauri flavor 已开启 updater artifacts,并使用独立 updater endpoint `https://wygoralves.github.io/panes/cuelight/latest.json`;主 `tauri.conf.json` 暂不切到 CueLight,等待 C8 确认
- [x] CueLight 发行说明已新增 `docs/cuelight-distribution.md`,包含 flavor 构建命令、updater feed、release short/long copy、release note baseline 与最终图标替换清单;README/README.zh-CN 已加入 CueLight 发行版入口
- [x] CueLight flavor 已新增独立图标源 `src-tauri/icons-cuelight/source.svg`,并通过 `pnpm tauri icon` 生成 PNG/ICO/ICNS/Windows/iOS/Android 图标资产;`tauri.cuelight.conf.json` 的 `bundle.icon` 已切到 `icons-cuelight/*`
- [x] C8 已按双 flavor 方案落地:主 `tauri.conf.json` 继续发布 Panes,`tauri.cuelight.conf.json` 发布 CueLight;后续整 App rename 作为单独迁移处理
- [x] Clean-room 合规审计脚本已新增 `scripts/audit-claurst-clean-room.mjs` / `pnpm audit:claurst-clean-room`,确认 `vendor/claurst` 未进入 Cargo build graph,并要求 `vendor/README.md` 保留 GPL-3.0 只读参考说明
- [x] `claude-code-native` 已从后端 `list_engines` 隐藏,运行时 start/send/steer/approval/interrupt/archive/unarchive 路由到 `claurst-native` 兼容入口
- [x] `claude_code_rs` path 依赖、`vendor/claude-code-rust` workspace member、旧 `ClaudeCodeNativeEngine` runtime 源码与 vendor tree 已移除;`vendor/README.md`、README workspace 说明已更新
- [x] Anthropic Messages SSE adapter 已解析 `message_start`/`message_delta` usage,并把 input/output/cache/thinking tokens 汇总到 `TurnCompleted.token_usage`
- [x] Anthropic usage 已按模型 tier 估算 `cost_usd` 并透传到 panes `EngineEvent::TurnCompleted`;已覆盖 Sonnet/Haiku/Opus tier、cache read/write、未知模型不估价
- [x] Anthropic pricing 已拆到 `infrastructure/anthropic/pricing.rs`,按当前官方定价覆盖 Fable/Mythos、Opus 4.8/4.7/4.6/4.5、legacy Opus 4.1/4、Sonnet、Haiku;Sonnet 4.6 与 Opus 4.6+ 的 1M 长上下文按标准价格估算
- [x] Anthropic SSE parser 已补 split tool input JSON、stream error event、usage/thinking/cost 的回归覆盖;空 `content_block_start.input={}` 不再污染后续 `input_json_delta` 拼接
- [x] Anthropic SSE 已新增 `messages_tool_use_recording.sse` fixture replay,覆盖 `message_start` usage、text/thinking delta、split tool input JSON、`message_delta` usage 与 `message_stop`
- [x] Anthropic HTTP request 已新增 `RetryPolicy`/`AnthropicRequestError`/`Recoverability` 分类模块:429/529/5xx、`rate_limit_error`、`overloaded_error`、`api_error` 会按 `Retry-After` 或指数 backoff 重试;认证/非法请求等 fatal 错误不重试;SSE stream error 文案也包含 recoverability
- [x] Anthropic request retry 已补本地 HTTP 集成测试:内置 test server 先返回 429 + `Retry-After`,再返回 SSE success,验证真实 `AnthropicMessagesClient::stream` 会重试并产出事件
- [x] Anthropic live fixture 录制流程已补 ignored 测试:`record_anthropic_messages_sse_fixture_from_live_api`,显式设置 `ANTHROPIC_API_KEY` 与 `ANTHROPIC_RECORDING_OUT` 后可刷新原始 SSE fixture;默认测试不会访问外网
- [x] Anthropic `thinking_delta` 已进入 `AgentEvent::ThinkingDelta` 并映射为 panes `EngineEvent::ThinkingDelta`
- [x] usage/thinking 已补 parser 与 runtime 覆盖:`sse_parser_emits_usage_and_thinking_delta`、`runtime_forwards_thinking_and_reports_turn_token_usage`
- [x] `claurst-native` send_message 已持续监听 `CancellationToken`,取消时发送 `TurnCompleted{status=Interrupted}` 并清理当前 thread 的 pending approvals
- [x] `claurst-native` interrupt/archive 会清理对应 thread 的 pending approvals;预取消 turn 已补适配层测试
- [x] `execute_command` 超时已改为 `NativeToolExecutor` 可配置策略,默认 30 秒;命令进程显式 `spawn`、并发读取 stdout/stderr、超时后 `start_kill` 并等待退出,已补短超时 runtime 覆盖
- [x] `execute_command` 审批拒绝已通过 `PermissionGateway` port 覆盖:拒绝时返回 error tool result,且不会启动命令副作用
- [x] `execute_command` `accept_for_session` 已补适配层回归覆盖:同一 engine thread 首次接受后,后续命令请求直接放行且不再发审批事件
- [x] `RunTurnCommand` 已携带 cancellation token,application loop 将取消信号传给 `ToolExecutor`;`execute_command` 运行中取消会主动 kill 子进程并返回 cancelled tool result,已补 runtime/native 覆盖
- [x] `file_write`/`file_edit`/`task_management` 已补行为覆盖;其中 `file_write` 修正为支持创建嵌套父目录,同时继续拒绝 `..` 逃逸路径
- [x] legacy `claude-code-native` 线程继续对话时会写入 `legacyNativeMigration` metadata 标记,清理旧 runtime 的 token/compact/vendor-path metadata,并在首次迁移 turn 中插入一次性 notice,提示已由 CueLight Agent / `claurst-native` 接管
- [x] application 层通过 `ModelClient`/`EventSink`/`ToolExecutor` ports 编排,测试使用 fake infrastructure
- [x] `cargo test -p panes-agent`、`cargo check -p Panes`、`pnpm typecheck` 与相关前端 Vitest 通过

发布前人工事项:

- 无实现阻塞项;后续只剩真实发布前的人工法务/商业复核与正式 release 签名流程
