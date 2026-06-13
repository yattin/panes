# Panes改造实施计划

## 目标
1. 添加简体中文（zh-CN）i18n支持
2. 整合claude-code-rust作为内置agent
3. 支持独立的内置agent架构

## 项目分析

### 当前状态
- **panes项目**：基于Tauri v2 + React + TypeScript的桌面应用
- **i18n实现**：已使用i18next/react-i18next，支持en和pt-BR
- **引擎架构**：支持codex、claude、opencode三种引擎
- **后端实现**：Rust (src-tauri)，包含engines模块
- **claude-code-rust**：独立的Rust CLI工具，已有完整功能和i18n支持

### 关键文件
```
panes/
├── src/
│   ├── i18n/
│   │   ├── index.ts                    # i18n初始化
│   │   └── resources/
│   │       ├── en/                     # 英文资源（7个JSON文件）
│   │       └── pt-BR/                  # 葡萄牙语资源
│   ├── stores/engineStore.ts           # 引擎状态管理
│   └── components/chat/
│       └── engineCapabilities.ts       # 引擎能力定义
├── src-tauri/
│   ├── Cargo.toml                      # Rust依赖
│   └── src/
│       └── engines/
│           ├── mod.rs                  # 引擎核心
│           ├── codex.rs                # Codex引擎
│           ├── claude_sidecar.rs       # Claude引擎
│           └── opencode.rs             # OpenCode引擎

claude-code-rust/
├── Cargo.toml                          # 已有i18n feature
├── locales/
│   ├── en.ftl                          # Fluent英文
│   └── zh.ftl                          # Fluent中文
└── src/
    ├── i18n/                           # i18n实现
    ├── api/                            # API客户端
    ├── cli/                            # CLI命令
    ├── mcp/                            # MCP协议
    └── services/                       # 服务层（含agents）
```

## 实施方案

### 阶段1：添加简体中文i18n支持（优先级：高）

**目标**：为panes前端添加完整的zh-CN语言支持

**实施步骤**：

1. **创建中文资源文件结构**
   - 创建 `panes/src/i18n/resources/zh-CN/` 目录
   - 复制所有7个JSON文件（common.json, app.json, chat.json, workspace.json, setup.json, git.json, native.json）
   - 翻译所有1763行英文内容到简体中文

2. **更新i18n配置**
   - 修改 `panes/src/i18n/index.ts`：
     - 导入zh-CN资源
     - 在resources对象中添加"zh-CN"键
     - 更新ns数组包含所有命名空间
   - 修改 `panes/src/lib/locale.ts`（如果存在）添加zh-CN识别

3. **更新UI语言选择器**
   - 在 `common.json` 中添加：
     ```json
     "simplifiedChinese": "简体中文"
     ```
   - 确保语言切换功能支持zh-CN

4. **测试验证**
   - 启动应用，切换到简体中文
   - 验证所有UI文本正确显示
   - 检查字符编码无乱码

**关键文件修改**：
- 新建：`src/i18n/resources/zh-CN/*.json` (7个文件)
- 修改：`src/i18n/index.ts`
- 修改：`src/i18n/resources/en/common.json`
- 修改：`src/i18n/resources/pt-BR/common.json`

### 阶段2：整合claude-code-rust到panes（优先级：高）

**目标**：将claude-code-rust作为嵌入式库集成到panes的Tauri后端

**方案选择**：使用**工作区依赖**方式

**实施步骤**：

1. **重组项目结构**
   ```
   panes-claude/
   ├── Cargo.toml (workspace root)
   ├── panes/
   │   └── src-tauri/
   │       └── Cargo.toml (member)
   └── claude-code-rust/
       └── Cargo.toml (member, as library)
   ```

2. **创建workspace Cargo.toml**
   ```toml
   [workspace]
   members = [
       "panes/src-tauri",
       "claude-code-rust"
   ]
   resolver = "2"
   ```

3. **修改claude-code-rust/Cargo.toml**
   - 确保有 `[lib]` 定义
   - 导出核心功能模块
   - 保留i18n feature启用

4. **修改panes/src-tauri/Cargo.toml**
   - 添加依赖：
     ```toml
     [dependencies]
     claude_code_rs = { path = "../../../claude-code-rust" }
     ```

5. **创建Rust桥接模块**
   - 创建 `panes/src-tauri/src/engines/claude_code_native.rs`
   - 实现ClaudeCodeNativeEngine结构
   - 调用claude-code-rust的核心API
   - 实现ChatEngine trait

6. **注册新引擎**
   - 在 `engines/mod.rs` 中添加claude_code_native模块
   - 在engine_list中注册"claude-code-native"引擎
   - 实现health check

7. **前端集成**
   - 在 `src/types.ts` 中添加 "claude-code-native" 到 ChatEngineId
   - 在 `src/components/chat/engineCapabilities.ts` 添加能力定义
   - 在UI中添加引擎选择器选项

**关键代码示例**：
```rust
// panes/src-tauri/src/engines/claude_code_native.rs
use claude_code_rs::{cli, api, config};

pub struct ClaudeCodeNativeEngine {
    config: config::Settings,
    api_client: api::Client,
}

impl ClaudeCodeNativeEngine {
    pub async fn new(api_key: String) -> Result<Self> {
        let config = config::Settings::load()?;
        let api_client = api::Client::new(api_key)?;
        Ok(Self { config, api_client })
    }
}

#[async_trait]
impl ChatEngine for ClaudeCodeNativeEngine {
    // 实现所有必需方法
}
```

### 阶段3：支持独立的内置agent架构（优先级：中）

**目标**：设计可扩展的agent插件系统

**设计原则**：
1. 插件化：agent作为可插拔模块
2. 统一接口：所有agent实现相同的trait
3. 独立配置：每个agent有独立配置
4. 并行支持：可同时运行多个agent

**实施步骤**：

1. **定义Agent trait**
   ```rust
   // src-tauri/src/agents/mod.rs
   #[async_trait]
   pub trait Agent: Send + Sync {
       fn id(&self) -> &str;
       fn name(&self) -> &str;
       fn capabilities(&self) -> AgentCapabilities;
       async fn initialize(&mut self, config: AgentConfig) -> Result<()>;
       async fn process_message(&self, msg: Message) -> Result<Response>;
       async fn health_check(&self) -> AgentHealth;
   }
   ```

2. **创建Agent注册中心**
   ```rust
   pub struct AgentRegistry {
       agents: HashMap<String, Box<dyn Agent>>,
   }
   
   impl AgentRegistry {
       pub fn register(&mut self, agent: Box<dyn Agent>);
       pub fn get(&self, id: &str) -> Option<&dyn Agent>;
       pub fn list_available(&self) -> Vec<AgentInfo>;
   }
   ```

3. **实现内置agents**
   - ClaudeCodeAgent（基于claude-code-rust）
   - CodexAgent（现有）
   - ClaudeAgent（现有）
   - OpenCodeAgent（现有）

4. **前端Agent选择器**
   - 创建 `src/components/agents/AgentPicker.tsx`
   - 显示可用agents列表
   - 支持agent切换和配置

5. **数据库schema扩展**
   - 在threads表添加 `agent_id` 字段
   - 支持每个thread绑定特定agent

**架构图**：
```
┌─────────────────────────────────────┐
│         Panes Frontend              │
│  (React + TypeScript + i18next)     │
└────────────┬────────────────────────┘
             │ IPC
┌────────────┴────────────────────────┐
│      Tauri Backend (Rust)           │
│  ┌──────────────────────────────┐   │
│  │    Agent Registry            │   │
│  │  - list_agents()             │   │
│  │  - get_agent(id)             │   │
│  └───────────┬──────────────────┘   │
│              │                       │
│  ┌───────────┴──────────────────┐   │
│  │     Agent Trait              │   │
│  │  + process_message()         │   │
│  │  + health_check()            │   │
│  └───┬────────────────┬─────────┘   │
│      │                │              │
│  ┌───┴────┐      ┌────┴────┐        │
│  │ Codex  │      │ Claude  │        │
│  │ Agent  │      │ Agent   │        │
│  └────────┘      └─────────┘        │
│                                      │
│  ┌──────────────────────────────┐   │
│  │  ClaudeCodeNativeAgent       │   │
│  │  (使用 claude-code-rust)      │   │
│  │  - cli commands              │   │
│  │  - MCP protocol              │   │
│  │  - plugin system             │   │
│  └───────────┬──────────────────┘   │
│              │                       │
└──────────────┼───────────────────────┘
               │ FFI
┌──────────────┴───────────────────────┐
│      claude-code-rust Library        │
│  - API client                        │
│  - MCP server                        │
│  - Plugin system                     │
│  - i18n (Fluent)                     │
└──────────────────────────────────────┘
```

## 实施顺序

### 第一阶段（立即执行）：简体中文i18n
1. 创建zh-CN资源文件
2. 翻译所有内容
3. 更新i18n配置
4. 测试验证

**预估时间**：2-3小时
**风险**：低
**优先级**：最高（用户可见功能）

### 第二阶段（核心功能）：整合claude-code-rust
1. 设置workspace结构
2. 修改Cargo配置
3. 创建桥接模块
4. 实现引擎接口
5. 前端集成
6. 测试验证

**预估时间**：4-6小时
**风险**：中（需要处理依赖冲突）
**优先级**：高

### 第三阶段（架构优化）：Agent系统
1. 定义Agent trait
2. 创建注册中心
3. 重构现有引擎为agents
4. 实现新agent（claude-code-native）
5. 前端UI更新
6. 完整测试

**预估时间**：6-8小时
**风险**：中（架构变更）
**优先级**：中

## 潜在问题与解决方案

### 问题1：依赖版本冲突
**现象**：panes和claude-code-rust使用不同版本的共同依赖
**解决方案**：
- 使用workspace统一版本管理
- 在workspace Cargo.toml中定义共享依赖版本
- 使用 `[workspace.dependencies]` 统一版本

### 问题2：i18n系统差异
**现象**：panes用i18next（JS），claude-code-rust用Fluent（Rust）
**解决方案**：
- 前端继续使用i18next
- 后端claude-code-rust内部使用Fluent
- 桥接层不暴露i18n细节，只返回已翻译文本

### 问题3：MCP协议冲突
**现象**：claude-code-rust有自己的MCP服务器实现
**解决方案**：
- 作为库使用时，只调用API层，不启动MCP服务器
- 或者让panes转发MCP请求到claude-code-rust

### 问题4：异步运行时
**现象**：两个项目都使用tokio但可能配置不同
**解决方案**：
- 使用相同的tokio版本
- 共享同一个runtime或正确桥接

## 成功标准

### 阶段1完成标准
- [ ] 所有UI文本有简体中文翻译
- [ ] 语言切换功能正常
- [ ] 无乱码或编码问题
- [ ] 通过i18n资源测试

### 阶段2完成标准
- [ ] claude-code-rust成功编译为库
- [ ] panes可以调用claude-code-rust核心功能
- [ ] 新引擎出现在引擎列表
- [ ] 可以创建使用claude-code-native引擎的thread
- [ ] 基本对话功能正常

### 阶段3完成标准
- [ ] Agent trait定义清晰
- [ ] 所有现有引擎迁移到Agent架构
- [ ] 可以动态注册/注销agents
- [ ] UI可以选择和切换agents
- [ ] 每个thread可以绑定独立agent

## 测试计划

### 单元测试
- i18n资源完整性测试
- Agent trait实现测试
- 引擎能力测试

### 集成测试
- claude-code-rust与panes集成测试
- 多agent并行运行测试
- 语言切换端到端测试

### 手动测试
- 创建各类型thread
- 切换语言
- 切换agent
- 发送消息验证功能

## 回滚方案

### 如果集成失败
1. 保持claude-code-rust作为独立项目
2. 通过IPC或子进程方式调用
3. 减少耦合度

### 如果Agent架构过于复杂
1. 保持现有引擎架构
2. 只添加claude-code-native作为新引擎
3. 延后完整Agent系统

## 依赖清单

### 新增Rust依赖
无（复用claude-code-rust现有依赖）

### 新增前端依赖
无（复用现有i18next）

### 文件清单（新增/修改）

**新增文件**：
- `src/i18n/resources/zh-CN/common.json`
- `src/i18n/resources/zh-CN/app.json`
- `src/i18n/resources/zh-CN/chat.json`
- `src/i18n/resources/zh-CN/workspace.json`
- `src/i18n/resources/zh-CN/setup.json`
- `src/i18n/resources/zh-CN/git.json`
- `src/i18n/resources/zh-CN/native.json`
- `Cargo.toml` (workspace root)
- `src-tauri/src/engines/claude_code_native.rs`
- `src-tauri/src/agents/mod.rs`
- `src/components/agents/AgentPicker.tsx`

**修改文件**：
- `src/i18n/index.ts`
- `src/i18n/resources/en/common.json`
- `src/i18n/resources/pt-BR/common.json`
- `panes/src-tauri/Cargo.toml`
- `claude-code-rust/Cargo.toml`
- `src-tauri/src/engines/mod.rs`
- `src/types.ts`
- `src/components/chat/engineCapabilities.ts`
- `src/stores/engineStore.ts`

---

**计划创建日期**：2026-06-13
**预计总时间**：12-17小时
**风险等级**：中
**建议开始时间**：立即执行阶段1
