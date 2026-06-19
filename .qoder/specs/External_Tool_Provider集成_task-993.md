# CueLight 影视业务化集成方案

## Context

Panes 当前是通用 AI coding agent，右侧面板有 Chat/Terminal/Editor 三种编程导向 Surface。需要与 ai-drama (CueLight) 打通，**绑定 CueLight 项目后彻底影视业务化**：隐藏 Terminal/Editor，替换为影视业务面板。复用 CueLight CLI 使用的现有 REST API，无需新增端点。UI 按**单季结构**展示，不显示多季切换。

---

## 面板模式切换

绑定 CueLight 后，Header Surface 按钮动态替换：

| 编程模式（默认） | 影视模式（CueLight 绑定后） |
|-----------------|---------------------------|
| `chat` 💬 AI对话 | `chat` 💬 创作对话 |
| `terminal` ⬛ 终端 | `overview` 📊 项目总览 |
| `editor` 📝 编辑器 | `storyboard` 📋 剧本分镜 |
| | `assets` 🖼 角色/场景/素材 |

---

## 复用的 CueLight 现有 API

| 用途 | 端点 |
|------|------|
| 项目列表（绑定选择） | `GET /api/projects` |
| 项目详情 | `GET /api/projects/:id` |
| 圣经（世界观+风格） | `GET /api/projects/:id/bible` |
| 角色列表 | `GET /api/projects/:pid/characters` |
| 场景列表 | `GET /api/projects/:pid/scenes` |
| 道具列表 | `GET /api/projects/:pid/props` |
| 集数列表 | `GET /api/projects/:pid/episodes` |
| 集数详情 | `GET /api/episodes/:id` |
| 分镜列表 | `GET /api/episodes/:eid/storyboards` |
| 分镜详情 | `GET /api/storyboards/:id` |
| 视频资产 | `GET /api/projects/:pid/video-assets` |
| 视频版本 | `GET /api/projects/:pid/storyboards/:sid/video-versions` |
| 资源版本 | `GET /api/projects/:pid/resource-versions` |
| 健康检查 | `GET /api/health` |
| 生图 | `POST /v1/images/generations` |
| 生视频 | `POST /v1/videos/generations` |
| 任务状态 | `GET /v1/tasks/:taskId` |
| 模型列表 | `GET /v1/models` |

---

## Task 1: Rust — CueLight HTTP 代理 + 绑定管理

**新建** `src-tauri/src/commands/cuelight.rs`

```rust
#[tauri::command]
pub async fn cuelight_proxy(method, server_url, path, auth_token, body, query) -> Result<Value, String>;

#[tauri::command]
pub async fn bind_cuelight_project(workspace_id, binding) -> Result<(), String>;

#[tauri::command]
pub async fn unbind_cuelight_project(workspace_id) -> Result<(), String>;

#[tauri::command]
pub async fn get_cuelight_binding(workspace_id) -> Result<Option<CueLightBindingDto>, String>;
```

**修改**：`src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/models.rs`

---

## Task 2: 工作区绑定数据模型

**修改** `src/types.ts`：
```typescript
export interface Workspace {
  // ... 现有 ...
  cueLightBinding?: CueLightProjectBinding | null;
}
export interface CueLightProjectBinding {
  serverUrl: string;
  projectId: string;
  projectName: string;
  boundAt: string;
}
```

**修改** `src-tauri/src/commands/workspace.rs` — 持久化绑定（SQLite JSON 字段）

**修改** `src/lib/ipc.ts` — 追加 cuelight IPC 调用

---

## Task 3: Surface Mode 切换

**修改** `src/stores/workspacePaneStore.ts`：
```typescript
export type WorkspacePaneSurfaceKind =
  | "chat" | "terminal" | "editor"          // 编程模式
  | "overview" | "storyboard" | "assets";   // 影视模式
```

**修改** `src/components/workspace/WorkspacePaneShell.tsx`：
- `SURFACE_ORDER` 改为动态：根据 `cueLightBinding` 是否存在切换
- `SurfaceIcon` 扩展 overview/storyboard/assets 图标
- `SurfaceView` 增加对应 lazy 组件分支
- `isSurfaceKind` 校验函数扩展

---

## Task 4: 影视面板 — 📊 项目总览 (Overview)

**新建** `src/components/cuelight/CueLightOverview.tsx`

展示内容（单季，不显示多季切换）：
- 项目标题 + 类型 + 画幅
- 进度概览：集数总数/已有剧本/已有分镜/已生成视频
- 世界观摘要（来自 bible.worldView 前200字）
- 风格提示词（来自 bible.stylePrompt）
- 最近生成的媒体缩略图（最新3-4个 video-assets）

数据源：`GET /api/projects/:id` + `GET /api/projects/:id/bible` + `GET /api/projects/:id/video-assets`

---

## Task 5: 影视面板 — 📋 剧本分镜 (Storyboard)

**新建** `src/components/cuelight/CueLightStoryboard.tsx`

结构（单季）：
- **顶部**：集数水平滚动 Tab（Episode 1 | Episode 2 | ...）
- **主体**：选中集数的分镜卡片网格

分镜卡片：
- 场次号
- 缩略图（首帧 / referenceImageUrl）
- videoPrompt 摘要（截断60字）
- 关联角色 avatar 小圆标
- 视频状态：✅已生成 / ⏳生成中 / ○ 未生成

点击卡片 → 侧滑详情：
- 完整 videoPrompt
- 关联角色、场景名
- 视频版本列表（可预览）

数据源：`GET /api/projects/:pid/episodes` → `GET /api/episodes/:eid/storyboards`

---

## Task 6: 影视面板 — 🖼 角色/场景/素材 (Assets)

**新建** `src/components/cuelight/CueLightAssets.tsx`

顶部子 Tab：角色 | 场景 | 道具 | 生成记录

**角色 Tab**：
- 卡片网格：参考图缩略图 + 名称 + 描述摘要
- 数据源：`GET /api/projects/:pid/characters`

**场景 Tab**：
- 卡片网格：参考图 + 场景名 + 描述
- 数据源：`GET /api/projects/:pid/scenes`

**道具 Tab**：
- 卡片网格：参考图 + 道具名
- 数据源：`GET /api/projects/:pid/props`

**生成记录 Tab**：
- 最近异步任务列表（图片/视频生成状态）
- 数据源：`GET /api/projects/:pid/video-assets`

---

## Task 7: CueLight 数据 Store

**新建** `src/stores/cueLightStore.ts`：
```typescript
interface CueLightState {
  projectDetail: any | null;
  bible: { worldView?: string; stylePrompt?: string } | null;
  episodes: any[];
  characters: any[];
  scenes: any[];
  props: any[];
  storyboards: Record<string, any[]>;  // episodeId → storyboards
  videoAssets: any[];
  selectedEpisodeId: string | null;
  assetsTab: "characters" | "scenes" | "props" | "history";
  loading: Record<string, boolean>;
  error: string | null;
  
  loadOverview(binding): Promise<void>;
  loadEpisodes(binding): Promise<void>;
  loadStoryboards(binding, episodeId): Promise<void>;
  loadCharacters(binding): Promise<void>;
  loadScenes(binding): Promise<void>;
  loadProps(binding): Promise<void>;
  loadVideoAssets(binding): Promise<void>;
  setSelectedEpisodeId(id): void;
  setAssetsTab(tab): void;
}
```

---

## Task 8: Agent 工具（影视业务）

**新建** `src-tauri/src/engines/cuelight_tools.rs`

硬编码工具 schema，执行时代理调 CueLight API：

| 工具名 | 对应 API |
|--------|----------|
| `cuelight_project_status` | `GET /api/projects/:id` |
| `cuelight_list_characters` | `GET /api/projects/:pid/characters` |
| `cuelight_list_scenes` | `GET /api/projects/:pid/scenes` |
| `cuelight_list_episodes` | `GET /api/projects/:pid/episodes` |
| `cuelight_list_storyboards` | `GET /api/episodes/:eid/storyboards` |
| `cuelight_generate_image` | `POST /v1/images/generations` |
| `cuelight_generate_video` | `POST /v1/videos/generations` |
| `cuelight_task_status` | `GET /v1/tasks/:taskId` |
| `cuelight_list_models` | `GET /v1/models` |
| `cuelight_create_storyboard` | `POST /api/episodes/:eid/storyboards` |
| `cuelight_update_storyboard` | `PUT /api/storyboards/:id` |

**修改** `src-tauri/src/engines/claude_code_native.rs`：
- 绑定 CueLight 时注入影视工具（替代编程工具）
- System prompt 切换为影视创作导向

---

## Task 9: 绑定 UI 入口

**修改** `src/components/workspace/WorkspaceSettingsModal.tsx` — 常规标签页增加 CueLight 绑定区域

**新建** `src/components/cuelight/CueLightProjectPicker.tsx` — URL+Token → 项目列表 → 选择绑定

---

## 文件清单

| 文件 | 操作 |
|------|------|
| `src-tauri/src/commands/cuelight.rs` | 新建 |
| `src-tauri/src/engines/cuelight_tools.rs` | 新建 |
| `src-tauri/src/engines/claude_code_native.rs` | 修改 |
| `src-tauri/src/commands/mod.rs` | 修改 |
| `src-tauri/src/lib.rs` | 修改 |
| `src-tauri/src/models.rs` | 修改 |
| `src/types.ts` | 修改 |
| `src/lib/ipc.ts` | 修改 |
| `src/stores/workspacePaneStore.ts` | 修改 |
| `src/stores/cueLightStore.ts` | 新建 |
| `src/components/workspace/WorkspacePaneShell.tsx` | 修改 |
| `src/components/workspace/WorkspaceSettingsModal.tsx` | 修改 |
| `src/components/cuelight/CueLightProjectPicker.tsx` | 新建 |
| `src/components/cuelight/CueLightOverview.tsx` | 新建 |
| `src/components/cuelight/CueLightStoryboard.tsx` | 新建 |
| `src/components/cuelight/CueLightAssets.tsx` | 新建 |

---

## 验证

1. `cd C:\codes\mogu\ai-drama && bun dev`（CueLight 运行在 localhost:3000）
2. Workspace Settings → Link CueLight → 选择项目 → 绑定成功
3. Header 变为 [💬][📊][📋][🖼]，Terminal/Editor 消失
4. 📊 总览：项目信息、进度、风格正确
5. 📋 分镜：集数 Tab 切换 → 分镜卡片网格（含缩略图和状态）
6. 🖼 素材：角色/场景/道具卡片正确（含参考图）
7. Chat：要求 "列出角色" → Agent 调 cuelight_list_characters → 返回
8. 解绑后恢复编程模式 [💬][⬛][📝]