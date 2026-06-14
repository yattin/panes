<p align="center">
  <img src="app-icon.svg" alt="Panes" width="128" height="128" />
</p>

<h1 align="center">Panes</h1>

<p align="center">
  <a href="./README.md">English</a> &bull; <a href="./README.pt-BR.md">Português (Brasil)</a> &bull; <strong>简体中文</strong>
</p>

<p align="center">
  <strong>本地优先的 AI 辅助编码驾驶舱。</strong>
</p>

<p align="center">
  <a href="https://panesade.com">官网</a> &bull;
  <a href="#功能特性">功能特性</a> &bull;
  <a href="#快速开始">快速开始</a> &bull;
  <a href="#开发">开发</a> &bull;
  <a href="#架构">架构</a> &bull;
  <a href="#贡献">贡献</a> &bull;
  <a href="#许可证">许可证</a>
</p>

<p align="center">
  <a href="https://github.com/wygoralves/panes/releases/latest"><img src="https://img.shields.io/github/v/release/wygoralves/panes?label=download&color=blue" alt="Latest Release" /></a>
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License" />
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg" alt="Platform" />
  <img src="https://img.shields.io/badge/tauri-v2-blue?logo=tauri" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/auto--update-OTA-green.svg" alt="OTA Auto-Update" />
</p>

---

Panes 在外部编码 agent、git、终端工作流以及轻量级文件编辑之外，包了一层原生桌面 UI。它让开发者在一个地方与 agent 对话、查看 diff、审批动作、管理多仓库工作，并保留发生过什么的可审计记录。

Panes 并不是一个完整的 IDE，但内置了一个多标签编辑器，方便在不离开应用的情况下快速查看和修改文件。

## 功能特性

### 对话与 Agent

- 流式对话，支持文本、思考、动作、diff、审批、附件和用量更新等结构化内容块
- 通过 `codex app-server` 集成 Codex 对话
- 通过 Claude Agent SDK sidecar 集成 Claude 对话
- Plan 模式、附件、reasoning effort 控制、按 thread 的审批/网络覆盖，以及 Codex 专属的 sandbox 模式覆盖
- 基于 FTS 的全局消息搜索，支持键盘导航
- 针对长 thread 和动作输出的窗口化加载与延迟水合

### Git

- 多仓库感知，每个仓库可单独启用并设置信任级别
- Changes、diff、暂存、取消暂存、丢弃、commit 和软重置
- 分支管理，支持分页与搜索
- Commit 历史、stash 操作、worktree 管理以及 remote 管理
- 通过 UI 完成 repo 初始化流程
- 文件系统监听，并对大型 repo 采用带缓存/截断的文件树扫描

### 终端与 Harness

- 基于 xterm.js + WebGL 的原生 PTY 终端
- 终端分组、分屏、可拖拽调整大小，以及广播模式
- 会话回放/恢复，以及渲染器诊断
- 针对 Codex CLI、Claude Code、Gemini CLI、Kiro、OpenCode、Kilo Code 和 Factory Droid 的 harness 检测、安装与启动流程
- 多启动模式，可为每个 harness 各开一个会话，并可选为每个会话分配一个 git worktree

### 编辑器与桌面体验

- 多标签 CodeMirror 编辑器，支持脏状态跟踪、保存以及外部修改提醒
- 内置查找/替换（`Cmd+F`、`Cmd+H`）以及编辑器开关（`Cmd+E`）
- 命令面板，覆盖命令、文件、thread、workspace、harness 以及 git 动作
- 针对 Node.js 与 Codex 依赖的安装向导，以及 Git 检测
- 更新对话框，内置下载/安装流程
- 崩溃恢复、toast 通知以及会话持久化

## 快速开始

### 前置要求

| 要求 | 版本 |
|---|---|
| Rust 工具链 | stable |
| Node.js | 20+ |
| pnpm | 9+ |
| Codex CLI | Codex 对话引擎必需；安装向导可通过 npm 安装 |
| Tauri v2 前置依赖 | [参见 Tauri 文档](https://v2.tauri.app/start/prerequisites/) |

### 在 macOS 上安装

```bash
brew install --cask wygoralves/tap/panes
```

Homebrew 是 macOS 上获取 Panes 预编译版本的主要安装途径。macOS 版本以通用应用形式发布，同一份 DMG 同时适用于 Apple Silicon 和 Intel Mac。后续版本由应用内更新器处理。

Panes 当前未经过 Apple 签名与公证，因此 Homebrew 只是降低了 Gatekeeper 的阻力，并不能完全消除。tap 在安装时会尽力移除隔离属性，但根据系统策略，macOS 仍可能在首次启动时要求手动确认。如果遇到这种情况，可使用 Finder 的“打开”流程，或直接从 [GitHub Releases](https://github.com/wygoralves/panes/releases/latest) 下载 DMG。

如果 Gatekeeper 直接拦截了 DMG 安装，可使用以下命令，而不是全局关闭 Gatekeeper：

```bash
# 如果 macOS 拦截了下载的 DMG 本身
xattr -d com.apple.quarantine ~/Downloads/Panes*.dmg
open ~/Downloads/Panes*.dmg

# 将 Panes.app 拖入 /Applications 后，如果首次启动被拦截
xattr -dr com.apple.quarantine /Applications/Panes.app
open /Applications/Panes.app
```

维护者可在 [docs/homebrew-distribution.md](./docs/homebrew-distribution.md) 查看 tap 与发布自动化配置。

### 在 Windows 上安装

从 [GitHub Releases](https://github.com/wygoralves/panes/releases/latest) 下载最新的 `*-setup.exe` 安装包并运行。后续更新通过 Tauri 更新器在应用内交付。

本次 Windows 版本已验证的范围包括安装包、更新器、启动以及捆绑运行时的兼容性。Codex 与 Claude 在应用内对话流程中的端到端完整性尚未完全验证，相关体验可能仍有瑕疵。

### 在 Linux 上安装

从 [GitHub Releases](https://github.com/wygoralves/panes/releases/latest) 下载最新的 `.AppImage` 或 `.deb`。

AppImage 方式：

```bash
chmod +x Panes*.AppImage
./Panes*.AppImage
```

Debian 系发行版：

```bash
sudo apt install ./Panes*_amd64.deb
```

这两种 Linux 直装方式后续都通过应用内更新器获取新版本。AppImage 更新会直接替换应用包；`.deb` 更新会重新安装已签名的 Debian 包，安装过程中可能请求管理员权限。

Panes 目前未提供 APT 仓库，因此 Debian 系的官方安装途径就是上面的 `.deb` 直接下载。

### 从源码安装并运行

```bash
git clone https://github.com/wygoralves/panes.git
cd panes
pnpm install
pnpm tauri:dev
```

### Codex 终端通知

在应用设置的 `Agent notifications` 中完成一次安装后，Panes 即可展示 Codex 的终端通知。该操作会在你的 Codex 用户配置中写入一条 `notify = [...]` 命令，指向 Panes。

Codex 目前会向配置的 `notify` 程序传入单个 JSON payload。`panes codex-notify` 能够处理当前的 `agent-turn-complete` payload，提取最后一条 assistant 消息，并将其路由回所属的 Panes 终端会话，以便 Panes 同时弹出桌面通知和应用内通知。

该功能仅在 Panes 启动的终端内有效，因为安装的命令依赖 `PANES_NOTIFY_ADDR`、`PANES_NOTIFY_TOKEN`、`PANES_WORKSPACE_ID` 和 `PANES_SESSION_ID`。

### Claude 终端通知

在应用设置的 `Agent notifications` 中完成一次安装后，Panes 即可展示 Claude 的终端通知。该操作会将 Panes 管理的 hook 命令合并到你的 Claude 用户设置中，且不会移除已有 hook。

该 hook 桥接目前处理 Claude 的 `Notification`、`Stop`、`StopFailure`、`SessionStart` 和 `SessionEnd` 事件，将它们路由回所属的 Panes 终端会话，以便 Panes 弹出桌面与应用内通知，并在 Claude 会话开始或结束时清理陈旧状态。

该功能仅在 Panes 启动的终端内有效，因为安装的 hook 命令依赖 Panes 终端会话的环境变量。

### 通用 OSC 终端通知

Panes 也会监听由 Panes 终端会话内运行的程序直接发出的常见桌面通知 OSC 序列。无需任何 Claude 或 Codex 配置即可生效。后端目前会在终端回放被记录之前识别 `OSC 9`、`OSC 777;notify;...` 以及 `OSC 99` 通知 payload，因此在恢复终端会话时，实时通知不会重复触发。

`OSC 9;4` 进度报告会被特意保留，不会被当作通知处理。

### 生产构建

```bash
pnpm tauri:build
```

常见的打包产物包括 macOS 的 DMG/应用归档、Linux 的 DEB/AppImage 产物，以及 Windows 的 NSIS 安装包，具体取决于平台与 target。

Git 在 repo 管理相关功能中是推荐项，但即使没有它，应用仍可启动。

## 开发

```bash
pnpm tauri:dev          # 以开发模式启动完整桌面应用
pnpm tauri:build        # 构建原生桌面打包产物

pnpm dev                # 仅启动前端开发服务器
pnpm build              # 前端生产构建
pnpm test               # 运行 Vitest 测试套件
pnpm typecheck          # TypeScript 仅类型检查（不产出生成）

pnpm build:claude-sidecar   # 打包运行时 Claude sidecar
pnpm build:desktop          # 构建前端及捆绑的 sidecar 资产，不构建原生应用包
pnpm prune:artifacts:check  # 检查可安全移除的已生成产物
pnpm prune:artifacts        # 移除仓库本地的已生成产物，例如 src-tauri/target
pnpm prune:artifacts:stale:check  # 检查存在超过 7 天的陈旧 Rust/Tauri 产物
pnpm prune:artifacts:stale        # 移除存在超过 7 天的陈旧 Rust/Tauri 产物
pnpm release:check          # 评估是否应该发布新版本
pnpm release                # 运行 release-it
```

仅 Rust（从仓库根目录运行）：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml
```

Rust workspace 位于仓库根目录（`Cargo.toml`），包含：

- `src-tauri/` — Tauri 后端
- `vendor/claude-code-rust/` — 内置的 Claude Code 引擎（`claude-code-native`）

在 Tauri/Rust 开发过程中，构建产物会迅速膨胀。`pnpm prune:artifacts` 会移除所有仓库本地的已生成输出，而 `pnpm prune:artifacts:stale` 仅清理超过 7 天的 Rust/Tauri 产物。两者在下一次构建时都能安全重新生成，陈旧模式还支持 `--older-than-days=<n>` 以自定义时间窗口。

### 运行时路径

| 路径 | 用途 |
|---|---|
| macOS / Linux：`~/.agent-workspace/config.toml` | 应用配置 |
| macOS / Linux：`~/.agent-workspace/workspaces.db` | SQLite 数据库 |
| macOS / Linux：`~/.agent-workspace/logs` | 应用日志目录 |
| Windows：`%LOCALAPPDATA%\Panes\config.toml` | 应用配置 |
| Windows：`%LOCALAPPDATA%\Panes\workspaces.db` | SQLite 数据库 |
| Windows：`%LOCALAPPDATA%\Panes\logs` | 应用日志目录 |

### 本地化

面向用户的前端文案使用 `i18next`/`react-i18next` 进行本地化。请把 i18n 视为每个新功能实现的一部分，而不是 UI 完成之后再做的清理工作。

- 不要在组件、对话框、菜单、toast 或空状态中硬编码新的可见 UI 字符串
- 在适用时，于 `src/i18n/resources/en/`、`src/i18n/resources/pt-BR/` 和 `src/i18n/resources/zh-CN/` 中新增或更新翻译键
- 尽可能复用现有的 namespace 结构，并保持各 locale 间的键对齐
- 文案变更时确保 i18n resource 测试持续通过

## 架构

Panes 的前端基于 React + Zustand，运行在 Tauri 外壳中；Rust 后端负责持久化、引擎编排、git 操作、终端管理以及文件系统安全的文件访问。

应用目前将 Native、Codex、Claude（sidecar）以及 OpenCode 作为对话引擎。Native 是默认引擎，将 vendored 的 `claude-code-rust` crate 直接嵌入到后端中；Codex 连接到 `codex app-server`；Claude 通过捆绑的 Claude 运行时 sidecar 桥接。

### 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri v2 |
| 前端 | React 19 + TypeScript 5.5 + Vite 6 |
| 样式 | Tailwind CSS 4 |
| 状态管理 | Zustand 5 |
| Markdown | micromark + highlight.js |
| Diff | diff2html + 自定义 parser |
| 文件编辑器 | CodeMirror 6 |
| 终端 | xterm.js + portable-pty |
| 数据库 | SQLite + FTS5 |
| Git | `git2` + CLI 辅助 |

## 贡献

欢迎贡献。请使用 [CONTRIBUTING.md](./CONTRIBUTING.md) 中描述的 pull request 流程。

所有外部变更都应通过评审后的 pull request 提交。如果变更新增或修改了面向用户的文案，请在同一次变更中同步更新所有 locale 的 resource。

## 许可证

[MIT](LICENSE)
