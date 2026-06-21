use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use panes_agent::ToolSpec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::models::CueLightBindingDto;

/// 固定的 CueLight 服务器 URL
pub const CUELIGHT_SERVER_URL: &str = "https://cuelight.app";

/// 全局 Token 存储（由前端通过 IPC 设置）
static GLOBAL_AUTH_TOKEN: Mutex<Option<String>> = Mutex::new(None);

/// 设置全局 CueLight Token（由前端调用）
pub fn set_global_auth_token(token: String) {
    if let Ok(mut guard) = GLOBAL_AUTH_TOKEN.lock() {
        *guard = Some(token);
    }
}

/// 获取全局 CueLight Token
pub fn get_global_auth_token() -> Option<String> {
    GLOBAL_AUTH_TOKEN
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

/// CueLight 影视模式上下文，注入到 ThreadState 中
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CueLightThreadContext {
    pub project_id: String,
    pub project_name: String,
    pub project_type: Option<String>,
    pub video_aspect_ratio: Option<String>,
    pub style_prompt_summary: Option<String>,
    pub episode_count: usize,
    pub character_count: usize,
    pub storyboard_count: usize,
}

impl CueLightThreadContext {
    /// 从数据库绑定记录构建上下文
    pub async fn from_binding(binding: &CueLightBindingDto) -> Result<Self, String> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/projects/{}",
            CUELIGHT_SERVER_URL, binding.project_id
        );

        let mut request = client.get(&url);
        if let Some(token) = get_global_auth_token() {
            if !token.is_empty() {
                request = request.header("Authorization", format!("Bearer {}", token));
            }
        }

        let response = request
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("failed to fetch project: {}", e))?;

        let project: Value = response
            .json()
            .await
            .map_err(|e| format!("failed to parse project: {}", e))?;

        let episodes = project["episodes"]
            .as_array()
            .map(|arr| arr.len())
            .unwrap_or(0);

        let storyboards = project["storyboards"]
            .as_array()
            .map(|arr| arr.len())
            .unwrap_or(0);

        Ok(Self {
            project_id: binding.project_id.clone(),
            project_name: binding.project_name.clone(),
            project_type: project["projectType"].as_str().map(|s| s.to_string()),
            video_aspect_ratio: project["videoAspectRatio"].as_str().map(|s| s.to_string()),
            style_prompt_summary: None,
            episode_count: episodes,
            character_count: 0,
            storyboard_count: storyboards,
        })
    }
}

/// 构建 CueLight 影视工具定义列表
#[allow(dead_code)]
pub fn build_cuelight_tool_definitions() -> Vec<Value> {
    build_cuelight_tool_specs()
        .into_iter()
        .map(tool_spec_to_openai_function)
        .collect()
}

/// 构建 provider-neutral CueLight 工具规格。
pub fn build_cuelight_tool_specs() -> Vec<ToolSpec> {
    build_cuelight_openai_tool_definitions()
        .into_iter()
        .filter_map(openai_function_to_tool_spec)
        .collect()
}

fn build_cuelight_openai_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_download_original_script",
                "description": "下载当前 CueLight 项目的剧本原文到本地 workspace 的 .cuelight/original-script/，方便后续用 file_read/search/list_files 分析原文。只下载原始剧本文本，不下载项目详情、bible、角色、场景、道具、分集或分镜等派生数据。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "source_document_id": {
                            "type": "string",
                            "description": "原文文档 ID（可选）。未知时不要传，工具会自动使用项目最新原文文档。"
                        }
                    },
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_project_status",
                "description": "查看 CueLight 项目的完整状态和进度",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_get_story_bible",
                "description": "读取项目故事设计（worldView 等核心故事设定）",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_update_story_bible",
                "description": "更新项目故事设计字段。只写入传入字段，不从原文文件伪造项目状态。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "fields": {
                            "type": "object",
                            "description": "要写入 /api/projects/:id/bible 的字段，例如 worldView、stylePrompt、autoAttachAssets"
                        }
                    },
                    "required": ["fields"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_get_visual_bible",
                "description": "读取项目视觉设计/风格相关字段",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_update_visual_bible",
                "description": "更新项目视觉设计/风格字段",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "fields": {
                            "type": "object",
                            "description": "要写入 /api/projects/:id/bible 的视觉字段，例如 stylePrompt、visualStyle、visualMode、aspectRatio"
                        }
                    },
                    "required": ["fields"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_list_characters",
                "description": "列出项目中的所有角色及其参考图",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }),
        entity_tool_definition(
            "cuelight_get_character",
            "读取单个角色详情（当前 API 无角色单项 GET，工具会从角色列表中过滤）",
            "character_id",
            "角色 ID",
            None,
        ),
        entity_tool_definition(
            "cuelight_create_character",
            "创建角色。fields 按 CueLight REST 字段传入，例如 name、description、basePrompt、referenceImageUrl",
            "",
            "",
            Some(true),
        ),
        entity_tool_definition(
            "cuelight_update_character",
            "更新角色。fields 按 CueLight REST 字段传入，例如 name、description、basePrompt、referenceImageUrl",
            "character_id",
            "角色 ID",
            Some(false),
        ),
        entity_tool_definition(
            "cuelight_delete_character",
            "删除角色",
            "character_id",
            "角色 ID",
            None,
        ),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_list_scenes",
                "description": "列出项目中的所有场景及其参考图",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }),
        entity_tool_definition(
            "cuelight_get_scene",
            "读取单个场景详情（当前 API 无场景单项 GET，工具会从场景列表中过滤）",
            "scene_id",
            "场景 ID",
            None,
        ),
        entity_tool_definition(
            "cuelight_create_scene",
            "创建场景。fields 按 CueLight REST 字段传入，例如 name、description、basePrompt、referenceImageUrl",
            "",
            "",
            Some(true),
        ),
        entity_tool_definition(
            "cuelight_update_scene",
            "更新场景。fields 按 CueLight REST 字段传入，例如 name、description、basePrompt、referenceImageUrl",
            "scene_id",
            "场景 ID",
            Some(false),
        ),
        entity_tool_definition(
            "cuelight_delete_scene",
            "删除场景",
            "scene_id",
            "场景 ID",
            None,
        ),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_list_props",
                "description": "列出项目中的所有道具及其参考图",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }),
        entity_tool_definition(
            "cuelight_get_prop",
            "读取单个道具详情（当前 API 无道具单项 GET，工具会从道具列表中过滤）",
            "prop_id",
            "道具 ID",
            None,
        ),
        entity_tool_definition(
            "cuelight_create_prop",
            "创建道具。fields 按 CueLight REST 字段传入，例如 name、description、basePrompt、referenceImageUrl",
            "",
            "",
            Some(true),
        ),
        entity_tool_definition(
            "cuelight_update_prop",
            "更新道具。fields 按 CueLight REST 字段传入，例如 name、description、basePrompt、referenceImageUrl",
            "prop_id",
            "道具 ID",
            Some(false),
        ),
        entity_tool_definition(
            "cuelight_delete_prop",
            "删除道具",
            "prop_id",
            "道具 ID",
            None,
        ),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_list_episodes",
                "description": "列出项目的所有集数及剧本状态",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }),
        entity_tool_definition(
            "cuelight_get_episode",
            "读取单集详情和剧本正文",
            "episode_id",
            "集数 ID",
            None,
        ),
        entity_tool_definition(
            "cuelight_create_episode",
            "创建集数。fields 按 CueLight REST 字段传入，例如 title、summary、content",
            "",
            "",
            Some(true),
        ),
        entity_tool_definition(
            "cuelight_update_episode",
            "更新集数。fields 按 CueLight REST 字段传入，例如 title、summary、content",
            "episode_id",
            "集数 ID",
            Some(false),
        ),
        entity_tool_definition(
            "cuelight_delete_episode",
            "删除集数",
            "episode_id",
            "集数 ID",
            None,
        ),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_list_storyboards",
                "description": "查看某集的分镜列表",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "episode_id": {
                            "type": "string",
                            "description": "集数 ID"
                        }
                    },
                    "required": ["episode_id"]
                }
            }
        }),
        entity_tool_definition(
            "cuelight_get_storyboard",
            "读取单个分镜详情",
            "storyboard_id",
            "分镜 ID",
            None,
        ),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_create_storyboard",
                "description": "为某集创建新分镜",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "episode_id": {
                            "type": "string",
                            "description": "集数 ID"
                        },
                        "video_prompt": {
                            "type": "string",
                            "description": "视频生成提示词（英文）"
                        },
                        "reference_character_ids": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "关联角色 ID 列表"
                        },
                        "fields": {
                            "type": "object",
                            "description": "其他分镜 REST 字段，例如 sceneNumber、shotType、cameraMovement、description、visualPrompt、dialogue、videoGenMode"
                        }
                    },
                    "required": ["episode_id"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_update_storyboard",
                "description": "更新分镜的 videoPrompt、关联角色等",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "storyboard_id": {
                            "type": "string",
                            "description": "分镜 ID"
                        },
                        "video_prompt": {
                            "type": "string",
                            "description": "视频生成提示词（英文）"
                        },
                        "reference_character_ids": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "关联角色 ID 列表"
                        },
                        "fields": {
                            "type": "object",
                            "description": "其他分镜 REST 字段，例如 sceneNumber、shotType、cameraMovement、description、visualPrompt、dialogue、videoGenMode"
                        }
                    },
                    "required": ["storyboard_id"]
                }
            }
        }),
        entity_tool_definition(
            "cuelight_delete_storyboard",
            "删除分镜",
            "storyboard_id",
            "分镜 ID",
            None,
        ),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_batch_update_storyboards",
                "description": "批量更新分镜字段",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "storyboard_id": {
                            "type": "string",
                            "description": "用于定位批量更新路由的分镜 ID"
                        },
                        "updates": {
                            "type": "array",
                            "items": { "type": "object" },
                            "description": "更新数组，每项至少包含 id，并包含要更新的分镜字段"
                        }
                    },
                    "required": ["storyboard_id", "updates"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_upload_file",
                "description": "上传当前 workspace 内的本地图片/视频/音频到 CueLight /v1/files，返回可放入 image_urls 的 URL",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "workspace 内本地文件路径，支持相对路径或 workspace 内绝对路径"
                        },
                        "purpose": {
                            "type": "string",
                            "description": "用途标签，如 image_input、reference"
                        }
                    },
                    "required": ["path"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_generate_image",
                "description": "异步生成图片（角色参考图、场景图等）",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "图片生成提示词"
                        },
                        "model": {
                            "type": "string",
                            "description": "图片生成模型名称"
                        },
                        "size": {
                            "type": "string",
                            "description": "宽高比或尺寸，如 16:9、1:1、9:16、landscape、portrait、auto"
                        },
                        "aspect_ratio": {
                            "type": "string",
                            "description": "宽高比，等价于 size"
                        },
                        "image_urls": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "参考图 URL 列表"
                        }
                    },
                    "required": ["prompt"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_generate_video",
                "description": "异步生成视频片段",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "视频生成提示词"
                        },
                        "model": {
                            "type": "string",
                            "description": "视频生成模型名称"
                        },
                        "negative_prompt": {
                            "type": "string",
                            "description": "负面提示词"
                        },
                        "duration": {
                            "type": "number",
                            "description": "视频时长（秒）"
                        },
                        "resolution": {
                            "type": "string",
                            "description": "分辨率，如 480p、720p、1080p、1k、2k、4k"
                        },
                        "aspect_ratio": {
                            "type": "string",
                            "description": "视频宽高比，如 16:9 或 9:16"
                        },
                        "image_urls": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "参考图 URL 列表，当前通常最多 1 张"
                        },
                        "seed": {
                            "type": "number",
                            "description": "随机种子"
                        }
                    },
                    "required": ["prompt"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_task_status",
                "description": "查询异步生成任务的状态",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "task_id": {
                            "type": "string",
                            "description": "任务 ID"
                        }
                    },
                    "required": ["task_id"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "cuelight_list_models",
                "description": "查看可用的图片/视频生成模型",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "media_type": {
                            "type": "string",
                            "enum": ["image", "video"],
                            "description": "可选，按 image 或 video 过滤模型"
                        }
                    },
                    "required": []
                }
            }
        }),
    ]
}

fn openai_function_to_tool_spec(tool: Value) -> Option<ToolSpec> {
    let function = tool.get("function")?;
    Some(ToolSpec {
        name: function.get("name")?.as_str()?.to_string(),
        description: function
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        input_schema: function
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object", "properties": {} })),
    })
}

#[allow(dead_code)]
fn tool_spec_to_openai_function(spec: ToolSpec) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": spec.name,
            "description": spec.description,
            "parameters": spec.input_schema,
        }
    })
}

fn entity_tool_definition(
    name: &str,
    description: &str,
    id_name: &str,
    id_description: &str,
    fields_required: Option<bool>,
) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    if !id_name.is_empty() {
        properties.insert(
            id_name.to_string(),
            json!({
                "type": "string",
                "description": id_description
            }),
        );
        required.push(Value::String(id_name.to_string()));
    }
    if let Some(required_fields) = fields_required {
        properties.insert(
            "fields".to_string(),
            json!({
                "type": "object",
                "description": "要发送给 CueLight REST API 的字段对象"
            }),
        );
        if required_fields {
            required.push(Value::String("fields".to_string()));
        }
    }
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {
                "type": "object",
                "properties": Value::Object(properties),
                "required": required
            }
        }
    })
}

/// 执行 CueLight 工具调用
pub async fn execute_cuelight_tool(
    name: &str,
    args: &Value,
    ctx: &CueLightThreadContext,
    root: Option<&Path>,
    sandbox_mode: Option<&str>,
) -> (bool, String) {
    let client = reqwest::Client::new();
    let server_url = CUELIGHT_SERVER_URL;

    let make_request = |method: &str, path: &str, body: Option<Value>| {
        let url = format!("{}{}", server_url, path);
        let mut req = match method {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "PATCH" => client.patch(&url),
            "DELETE" => client.delete(&url),
            _ => client.get(&url),
        };
        req = req.header("Content-Type", "application/json");
        if let Some(token) = get_global_auth_token() {
            if !token.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
        }
        if let Some(body) = body {
            req = req.json(&body);
        }
        req
    };

    let result = match name {
        "cuelight_download_original_script" => {
            download_original_script(&client, server_url, args, ctx, root, sandbox_mode).await
        }
        "cuelight_project_status" => {
            let resp = make_request("GET", &format!("/api/projects/{}", ctx.project_id), None)
                .send()
                .await;
            handle_response(resp).await
        }
        "cuelight_get_story_bible" | "cuelight_get_visual_bible" => {
            let resp = make_request(
                "GET",
                &format!("/api/projects/{}/bible", ctx.project_id),
                None,
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_update_story_bible" | "cuelight_update_visual_bible" => {
            let body = match body_from_fields(args) {
                Ok(body) => body,
                Err(err) => return (false, err),
            };
            let resp = make_request(
                "PUT",
                &format!("/api/projects/{}/bible", ctx.project_id),
                Some(body),
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_list_characters" => {
            let resp = make_request(
                "GET",
                &format!("/api/projects/{}/characters", ctx.project_id),
                None,
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_get_character" => {
            get_item_from_project_list(&client, server_url, ctx, "characters", "character_id", args)
                .await
        }
        "cuelight_create_character" => {
            create_project_entity(&client, server_url, ctx, "characters", args).await
        }
        "cuelight_update_character" => {
            update_project_entity(&client, server_url, ctx, "characters", "character_id", args)
                .await
        }
        "cuelight_delete_character" => {
            delete_project_entity(&client, server_url, ctx, "characters", "character_id", args)
                .await
        }
        "cuelight_list_scenes" => {
            let resp = make_request(
                "GET",
                &format!("/api/projects/{}/scenes", ctx.project_id),
                None,
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_get_scene" => {
            get_item_from_project_list(&client, server_url, ctx, "scenes", "scene_id", args).await
        }
        "cuelight_create_scene" => {
            create_project_entity(&client, server_url, ctx, "scenes", args).await
        }
        "cuelight_update_scene" => {
            update_project_entity(&client, server_url, ctx, "scenes", "scene_id", args).await
        }
        "cuelight_delete_scene" => {
            delete_project_entity(&client, server_url, ctx, "scenes", "scene_id", args).await
        }
        "cuelight_list_props" => {
            let resp = make_request(
                "GET",
                &format!("/api/projects/{}/props", ctx.project_id),
                None,
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_get_prop" => {
            get_item_from_project_list(&client, server_url, ctx, "props", "prop_id", args).await
        }
        "cuelight_create_prop" => {
            create_project_entity(&client, server_url, ctx, "props", args).await
        }
        "cuelight_update_prop" => {
            update_project_entity(&client, server_url, ctx, "props", "prop_id", args).await
        }
        "cuelight_delete_prop" => {
            delete_project_entity(&client, server_url, ctx, "props", "prop_id", args).await
        }
        "cuelight_list_episodes" => {
            let resp = make_request(
                "GET",
                &format!("/api/projects/{}/episodes", ctx.project_id),
                None,
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_get_episode" => {
            let episode_id = match required_string_arg(args, "episode_id") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let resp = make_request("GET", &format!("/api/episodes/{}", episode_id), None)
                .send()
                .await;
            handle_response(resp).await
        }
        "cuelight_create_episode" => {
            let body = match body_from_fields(args) {
                Ok(body) => body,
                Err(err) => return (false, err),
            };
            let resp = make_request(
                "POST",
                &format!("/api/projects/{}/episodes", ctx.project_id),
                Some(body),
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_update_episode" => {
            let episode_id = match required_string_arg(args, "episode_id") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let body = match body_from_fields(args) {
                Ok(body) => body,
                Err(err) => return (false, err),
            };
            let resp = make_request("PUT", &format!("/api/episodes/{}", episode_id), Some(body))
                .send()
                .await;
            handle_response(resp).await
        }
        "cuelight_delete_episode" => {
            let episode_id = match required_string_arg(args, "episode_id") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let resp = make_request("DELETE", &format!("/api/episodes/{}", episode_id), None)
                .send()
                .await;
            handle_response(resp).await
        }
        "cuelight_list_storyboards" => {
            let episode_id = args["episode_id"].as_str().unwrap_or("");
            if episode_id.is_empty() {
                return (false, "episode_id is required".to_string());
            }
            let resp = make_request(
                "GET",
                &format!("/api/episodes/{}/storyboards", episode_id),
                None,
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_get_storyboard" => {
            let storyboard_id = match required_string_arg(args, "storyboard_id") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let resp = make_request("GET", &format!("/api/storyboards/{}", storyboard_id), None)
                .send()
                .await;
            handle_response(resp).await
        }
        "cuelight_create_storyboard" => {
            let episode_id = args["episode_id"].as_str().unwrap_or("");
            if episode_id.is_empty() {
                return (false, "episode_id is required".to_string());
            }
            let mut body = body_from_optional_fields(args);
            if let Some(prompt) = args.get("video_prompt") {
                body["videoPrompt"] = prompt.clone();
            }
            if let Some(chars) = args.get("reference_character_ids") {
                body["referenceCharacterIds"] = chars.clone();
            }
            let resp = make_request(
                "POST",
                &format!("/api/episodes/{}/storyboards", episode_id),
                Some(body),
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_update_storyboard" => {
            let storyboard_id = args["storyboard_id"].as_str().unwrap_or("");
            if storyboard_id.is_empty() {
                return (false, "storyboard_id is required".to_string());
            }
            let mut body = json!({});
            if let Some(prompt) = args.get("video_prompt") {
                body["videoPrompt"] = prompt.clone();
            }
            if let Some(chars) = args.get("reference_character_ids") {
                body["referenceCharacterIds"] = chars.clone();
            }
            merge_fields(&mut body, args);
            let resp = make_request(
                "PUT",
                &format!("/api/storyboards/{}", storyboard_id),
                Some(body),
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_delete_storyboard" => {
            let storyboard_id = match required_string_arg(args, "storyboard_id") {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let resp = make_request(
                "DELETE",
                &format!("/api/storyboards/{}", storyboard_id),
                None,
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_batch_update_storyboards" => {
            let Some(updates) = args.get("updates").filter(|value| value.is_array()) else {
                return (false, "updates array is required".to_string());
            };
            let mut results = Vec::new();
            for update in updates.as_array().unwrap_or(&Vec::new()) {
                let Some(storyboard_id) = update.get("id").and_then(Value::as_str) else {
                    return (
                        false,
                        "each storyboard update must include an `id` field".to_string(),
                    );
                };
                let Some(update_object) = update.as_object() else {
                    return (
                        false,
                        "each storyboard update must be an object".to_string(),
                    );
                };
                let mut body = serde_json::Map::new();
                for (key, value) in update_object {
                    if key != "id" {
                        body.insert(key.clone(), value.clone());
                    }
                }
                if body.is_empty() {
                    return (
                        false,
                        format!("storyboard update `{storyboard_id}` has no fields to update"),
                    );
                }
                let resp = make_request(
                    "PUT",
                    &format!("/api/storyboards/{}", storyboard_id),
                    Some(Value::Object(body)),
                )
                .send()
                .await;
                let (ok, output) = handle_response(resp).await;
                if !ok {
                    return (
                        false,
                        format!("failed to update storyboard `{storyboard_id}`: {output}"),
                    );
                }
                results.push(parse_json_or_raw(&output));
            }
            (true, json!({ "updated": results }).to_string())
        }
        "cuelight_upload_file" => upload_file(&client, server_url, args, root).await,
        "cuelight_generate_image" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            if prompt.is_empty() {
                return (false, "prompt is required".to_string());
            }
            let mut body = json!({ "prompt": prompt });
            copy_optional_fields(
                args,
                &mut body,
                &["model", "size", "aspect_ratio", "image_urls"],
            );
            let resp = make_request("POST", "/v1/images/generations", Some(body))
                .send()
                .await;
            handle_response(resp).await
        }
        "cuelight_generate_video" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            if prompt.is_empty() {
                return (false, "prompt is required".to_string());
            }
            let mut body = json!({ "prompt": prompt });
            copy_optional_fields(
                args,
                &mut body,
                &[
                    "model",
                    "negative_prompt",
                    "duration",
                    "resolution",
                    "aspect_ratio",
                    "image_urls",
                    "seed",
                ],
            );
            let resp = make_request("POST", "/v1/videos/generations", Some(body))
                .send()
                .await;
            handle_response(resp).await
        }
        "cuelight_task_status" => {
            let task_id = args["task_id"].as_str().unwrap_or("");
            if task_id.is_empty() {
                return (false, "task_id is required".to_string());
            }
            let resp = make_request("GET", &format!("/v1/tasks/{}", task_id), None)
                .send()
                .await;
            handle_response(resp).await
        }
        "cuelight_list_models" => {
            let path = match args["media_type"].as_str() {
                Some("image") => "/v1/models?media_type=image",
                Some("video") => "/v1/models?media_type=video",
                Some(_) => {
                    return (
                        false,
                        "media_type must be either `image` or `video`".to_string(),
                    )
                }
                None => "/v1/models",
            };
            let resp = make_request("GET", path, None).send().await;
            handle_response(resp).await
        }
        _ => (false, format!("unknown cuelight tool: {}", name)),
    };

    result
}

fn original_script_output_dir(root: &Path) -> Result<PathBuf, String> {
    let root_canonical = root
        .canonicalize()
        .map_err(|e| format!("working directory is not accessible: {e}"))?;
    let output_dir = root_canonical.join(".cuelight").join("original-script");
    if !output_dir.starts_with(&root_canonical) {
        return Err("resolved output path escapes the working directory".to_string());
    }
    Ok(output_dir)
}

fn required_string_arg(args: &Value, name: &str) -> Result<String, String> {
    args[name]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{name} is required"))
}

fn body_from_optional_fields(args: &Value) -> Value {
    let mut body = json!({});
    merge_fields(&mut body, args);
    body
}

fn body_from_fields(args: &Value) -> Result<Value, String> {
    let body = body_from_optional_fields(args);
    if body
        .as_object()
        .map(|object| object.is_empty())
        .unwrap_or(true)
    {
        return Err("fields object is required".to_string());
    }
    Ok(body)
}

fn merge_fields(body: &mut Value, args: &Value) {
    let Some(fields) = args.get("fields").and_then(Value::as_object) else {
        return;
    };
    for (key, value) in fields {
        body[key] = value.clone();
    }
}

fn copy_optional_fields(args: &Value, body: &mut Value, names: &[&str]) {
    for name in names {
        if let Some(value) = args.get(*name) {
            if !value.is_null() {
                body[*name] = value.clone();
            }
        }
    }
}

async fn create_project_entity(
    client: &reqwest::Client,
    server_url: &str,
    ctx: &CueLightThreadContext,
    collection: &str,
    args: &Value,
) -> (bool, String) {
    let body = match body_from_fields(args) {
        Ok(body) => body,
        Err(err) => return (false, err),
    };
    let resp = make_cuelight_request(
        client,
        server_url,
        "POST",
        &format!("/api/projects/{}/{}", ctx.project_id, collection),
        Some(body),
    )
    .send()
    .await;
    handle_response(resp).await
}

async fn update_project_entity(
    client: &reqwest::Client,
    server_url: &str,
    ctx: &CueLightThreadContext,
    collection: &str,
    id_arg: &str,
    args: &Value,
) -> (bool, String) {
    let entity_id = match required_string_arg(args, id_arg) {
        Ok(value) => value,
        Err(err) => return (false, err),
    };
    let body = match body_from_fields(args) {
        Ok(body) => body,
        Err(err) => return (false, err),
    };
    let resp = make_cuelight_request(
        client,
        server_url,
        "PUT",
        &format!(
            "/api/projects/{}/{}/{}",
            ctx.project_id, collection, entity_id
        ),
        Some(body),
    )
    .send()
    .await;
    handle_response(resp).await
}

async fn delete_project_entity(
    client: &reqwest::Client,
    server_url: &str,
    ctx: &CueLightThreadContext,
    collection: &str,
    id_arg: &str,
    args: &Value,
) -> (bool, String) {
    let entity_id = match required_string_arg(args, id_arg) {
        Ok(value) => value,
        Err(err) => return (false, err),
    };
    let resp = make_cuelight_request(
        client,
        server_url,
        "DELETE",
        &format!(
            "/api/projects/{}/{}/{}",
            ctx.project_id, collection, entity_id
        ),
        None,
    )
    .send()
    .await;
    handle_response(resp).await
}

async fn get_item_from_project_list(
    client: &reqwest::Client,
    server_url: &str,
    ctx: &CueLightThreadContext,
    collection: &str,
    id_arg: &str,
    args: &Value,
) -> (bool, String) {
    let entity_id = match required_string_arg(args, id_arg) {
        Ok(value) => value,
        Err(err) => return (false, err),
    };
    let payload = match get_json_response(
        make_cuelight_request(
            client,
            server_url,
            "GET",
            &format!("/api/projects/{}/{}", ctx.project_id, collection),
            None,
        )
        .send()
        .await,
    )
    .await
    {
        Ok(value) => value,
        Err(err) => return (false, err),
    };

    let items = payload
        .as_array()
        .or_else(|| payload.get("data").and_then(Value::as_array));
    let Some(items) = items else {
        return (
            false,
            format!(
                "{} response was not an array and did not contain data[]",
                collection
            ),
        );
    };
    let Some(item) = items
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(entity_id.as_str()))
    else {
        return (
            false,
            format!("{} item `{}` was not found", collection, entity_id),
        );
    };
    (true, item.to_string())
}

fn resolve_existing_file_within_root(root: &Path, requested: &str) -> Result<PathBuf, String> {
    let root_canonical = root
        .canonicalize()
        .map_err(|e| format!("working directory is not accessible: {e}"))?;
    let requested_path = Path::new(requested);
    let candidate = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        root_canonical.join(requested_path)
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|e| format!("file is not accessible: {e}"))?;
    if !canonical.starts_with(&root_canonical) {
        return Err("upload path must stay inside the current workspace".to_string());
    }
    if !canonical.is_file() {
        return Err("upload path must point to a file".to_string());
    }
    Ok(canonical)
}

fn infer_mime_type(path: &Path) -> String {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("mp4") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("m4a") => "audio/mp4",
        _ => "application/octet-stream",
    }
    .to_string()
}

async fn upload_file(
    client: &reqwest::Client,
    server_url: &str,
    args: &Value,
    root: Option<&Path>,
) -> (bool, String) {
    let Some(root) = root else {
        return (
            false,
            "cuelight_upload_file requires a workspace root".to_string(),
        );
    };
    let requested = match required_string_arg(args, "path") {
        Ok(value) => value,
        Err(err) => return (false, err),
    };
    let file_path = match resolve_existing_file_within_root(root, &requested) {
        Ok(path) => path,
        Err(err) => return (false, err),
    };
    let bytes = match tokio::fs::read(&file_path).await {
        Ok(bytes) => bytes,
        Err(e) => return (false, format!("failed to read upload file: {e}")),
    };
    let filename = file_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("upload.bin");
    let mut body = json!({
        "filename": filename,
        "data": BASE64.encode(bytes),
        "mimeType": infer_mime_type(&file_path),
    });
    if let Some(purpose) = args.get("purpose").and_then(Value::as_str) {
        if !purpose.trim().is_empty() {
            body["purpose"] = Value::String(purpose.trim().to_string());
        }
    }
    let resp = make_cuelight_request(client, server_url, "POST", "/v1/files", Some(body))
        .send()
        .await;
    handle_response(resp).await
}

fn make_cuelight_request(
    client: &reqwest::Client,
    server_url: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> reqwest::RequestBuilder {
    let url = format!("{}{}", server_url, path);
    let mut req = match method {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "PATCH" => client.patch(&url),
        "DELETE" => client.delete(&url),
        _ => client.get(&url),
    };
    req = req.header("Content-Type", "application/json");
    if let Some(token) = get_global_auth_token() {
        if !token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
    }
    if let Some(body) = body {
        req = req.json(&body);
    }
    req
}

async fn get_json_response(
    resp: Result<reqwest::Response, reqwest::Error>,
) -> Result<Value, String> {
    match resp {
        Ok(response) => {
            let status = response.status();
            let text = response
                .text()
                .await
                .map_err(|e| format!("failed to read response: {}", e))?;
            if !status.is_success() {
                return Err(format!(
                    "API error {}: {}",
                    status,
                    text.chars().take(500).collect::<String>()
                ));
            }
            if text.is_empty() {
                return Ok(Value::Null);
            }
            serde_json::from_str(&text).map_err(|e| {
                format!(
                    "failed to parse CueLight JSON response: {}; body: {}",
                    e,
                    text.chars().take(200).collect::<String>()
                )
            })
        }
        Err(e) => Err(format!("request failed: {}", e)),
    }
}

async fn download_original_script(
    client: &reqwest::Client,
    server_url: &str,
    args: &Value,
    ctx: &CueLightThreadContext,
    root: Option<&Path>,
    sandbox_mode: Option<&str>,
) -> (bool, String) {
    if sandbox_mode == Some("read-only") {
        return (
            false,
            "cuelight_download_original_script requires workspace-write or danger-full-access sandbox because it writes .cuelight/original-script/original-script.txt".to_string(),
        );
    }

    let Some(root) = root else {
        return (
            false,
            "cuelight_download_original_script requires a workspace root to write the original script locally".to_string(),
        );
    };

    let output_dir = match original_script_output_dir(root) {
        Ok(path) => path,
        Err(err) => return (false, err),
    };

    let explicit_source_document_id = args["source_document_id"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let mut source_document_id = explicit_source_document_id.clone();
    let mut filename: Option<String> = None;
    let mut original_text_available = true;
    let source_materials_path = format!("/api/projects/{}/source-materials", ctx.project_id);

    if source_document_id.is_none() {
        let source_materials = match get_json_response(
            make_cuelight_request(client, server_url, "GET", &source_materials_path, None)
                .send()
                .await,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => return (false, format!("failed to fetch source materials: {}", err)),
        };

        original_text_available = source_materials["originalTextAvailable"]
            .as_bool()
            .unwrap_or(false);
        let source_document = &source_materials["sourceDocument"];
        source_document_id = source_document["id"].as_str().map(str::to_string);
        filename = source_document["filename"].as_str().map(str::to_string);

        if source_document_id.is_none() {
            return (
                false,
                "CueLight project has no sourceDocument; no original script is available to download".to_string(),
            );
        }
        if !original_text_available {
            return (
                false,
                "CueLight project source materials report originalTextAvailable=false; no original script text is available to download".to_string(),
            );
        }
    }

    let Some(source_document_id) = source_document_id else {
        return (
            false,
            "source_document_id is required when the latest source document cannot be resolved"
                .to_string(),
        );
    };

    let original_path = format!(
        "/api/projects/{}/source-documents/{}/original",
        ctx.project_id, source_document_id
    );
    let original_payload = match get_json_response(
        make_cuelight_request(client, server_url, "GET", &original_path, None)
            .send()
            .await,
    )
    .await
    {
        Ok(value) => value,
        Err(err) => return (false, format!("failed to fetch original script: {}", err)),
    };

    let Some(content) = original_payload["content"].as_str() else {
        return (
            false,
            "CueLight original script response did not include a string `content` field"
                .to_string(),
        );
    };
    if content.trim().is_empty() {
        return (
            false,
            "CueLight original script content is empty".to_string(),
        );
    }

    if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
        return (
            false,
            format!(
                "failed to create output directory {}: {}",
                output_dir.display(),
                e
            ),
        );
    }

    let script_path = output_dir.join("original-script.txt");
    let manifest_path = output_dir.join("manifest.json");
    if let Err(e) = tokio::fs::write(&script_path, content).await {
        return (
            false,
            format!(
                "failed to write original script {}: {}",
                script_path.display(),
                e
            ),
        );
    }

    let char_count = content.chars().count();
    let byte_size = content.as_bytes().len();
    let manifest = build_original_script_manifest(OriginalScriptManifestInput {
        project_id: &ctx.project_id,
        project_name: &ctx.project_name,
        source_document_id: &source_document_id,
        filename: filename.as_deref(),
        char_count,
        byte_size,
        source_materials_path: &source_materials_path,
        original_path: &original_path,
        output_dir: &output_dir,
        script_path: &script_path,
        manifest_path: &manifest_path,
        original_text_available,
    });

    let manifest_text = match serde_json::to_string_pretty(&manifest) {
        Ok(text) => text,
        Err(e) => return (false, format!("failed to serialize manifest: {}", e)),
    };
    if let Err(e) = tokio::fs::write(&manifest_path, manifest_text).await {
        return (
            false,
            format!(
                "failed to write manifest {}: {}",
                manifest_path.display(),
                e
            ),
        );
    }

    let result = json!({
        "success": true,
        "projectId": ctx.project_id,
        "sourceDocumentId": source_document_id,
        "filename": filename,
        "charCount": char_count,
        "byteSize": byte_size,
        "outputDir": output_dir.to_string_lossy(),
        "scriptPath": script_path.to_string_lossy(),
        "manifestPath": manifest_path.to_string_lossy(),
        "message": "已下载 CueLight 剧本原文。后续请使用 file_read 读取 original-script.txt，或用 search/list_files 在 .cuelight/original-script/ 中分析原文。"
    });

    (true, result.to_string())
}

struct OriginalScriptManifestInput<'a> {
    project_id: &'a str,
    project_name: &'a str,
    source_document_id: &'a str,
    filename: Option<&'a str>,
    char_count: usize,
    byte_size: usize,
    source_materials_path: &'a str,
    original_path: &'a str,
    output_dir: &'a Path,
    script_path: &'a Path,
    manifest_path: &'a Path,
    original_text_available: bool,
}

fn build_original_script_manifest(input: OriginalScriptManifestInput<'_>) -> Value {
    json!({
        "projectId": input.project_id,
        "projectName": input.project_name,
        "sourceDocumentId": input.source_document_id,
        "filename": input.filename,
        "charCount": input.char_count,
        "byteSize": input.byte_size,
        "downloadedAt": chrono::Utc::now().to_rfc3339(),
        "api": {
            "sourceMaterialsPath": input.source_materials_path,
            "originalPath": input.original_path,
            "originalTextAvailable": input.original_text_available
        },
        "files": {
            "outputDir": input.output_dir.to_string_lossy(),
            "scriptPath": input.script_path.to_string_lossy(),
            "manifestPath": input.manifest_path.to_string_lossy()
        },
        "notes": "original-script.txt contains only the raw original script text returned by CueLight; derived project data such as bible, episodes, characters, scenes, props, storyboards, and media assets are intentionally not exported."
    })
}

async fn handle_response(resp: Result<reqwest::Response, reqwest::Error>) -> (bool, String) {
    match resp {
        Ok(response) => {
            let status = response.status();
            match response.text().await {
                Ok(text) => {
                    if status.is_success() {
                        (true, text)
                    } else {
                        (false, format!("API error {}: {}", status, text))
                    }
                }
                Err(e) => (false, format!("failed to read response: {}", e)),
            }
        }
        Err(e) => (false, format!("request failed: {}", e)),
    }
}

fn parse_json_or_raw(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| json!({ "raw": text }))
}

/// 构建 CueLight 影视模式的业务 system prompt 附录。
pub fn build_cuelight_system_prompt_appendix(ctx: &CueLightThreadContext) -> String {
    let style_prompt = ctx
        .style_prompt_summary
        .as_ref()
        .map(|s| format!("- 风格设定：{}", s))
        .unwrap_or_default();

    let aspect_ratio = ctx
        .video_aspect_ratio
        .as_ref()
        .map(|s| format!("- 画幅：{}", s))
        .unwrap_or_else(|| "- 画幅：16:9".to_string());

    let project_type = ctx
        .project_type
        .as_ref()
        .map(|s| format!("- 项目类型：{}", s))
        .unwrap_or_default();

    format!(
        r#"## 当前绑定 CueLight 项目的业务上下文

以下信息来自当前工作区绑定的 CueLight 项目，用于增强基础 agent 的影视制作能力。处理 CueLight 或影视任务时，应结合项目状态与工具结果，帮助用户完成短剧/短视频制作：
- 剧本创作：世界观构建、角色设计、场景规划、分集剧本撰写
- 视觉设计：角色参考图生成、场景氛围图生成、视觉风格设定
- 分镜制作：为每集编写分镜脚本（videoPrompt）、设定镜头语言
- 视频生成：提交图片/视频生成任务、管理生成进度

## CueLight 当前项目
- 项目名称：{}
- 项目 ID：{}
{}
{}
{}
- 集数：{} 集
- 分镜数：{} 个

## CueLight 可用工具
你拥有以下 CueLight 影视制作工具：
- `cuelight_project_status`：查看项目完整状态和进度
- `cuelight_get_story_bible` / `cuelight_update_story_bible`：读取或更新故事设计
- `cuelight_get_visual_bible` / `cuelight_update_visual_bible`：读取或更新视觉设计/风格字段
- `cuelight_list_characters` / `cuelight_get_character` / `cuelight_create_character` / `cuelight_update_character` / `cuelight_delete_character`：读写角色
- `cuelight_list_scenes` / `cuelight_get_scene` / `cuelight_create_scene` / `cuelight_update_scene` / `cuelight_delete_scene`：读写场景
- `cuelight_list_props` / `cuelight_get_prop` / `cuelight_create_prop` / `cuelight_update_prop` / `cuelight_delete_prop`：读写道具
- `cuelight_list_episodes` / `cuelight_get_episode` / `cuelight_create_episode` / `cuelight_update_episode` / `cuelight_delete_episode`：读写集数剧本
- `cuelight_list_storyboards` / `cuelight_get_storyboard` / `cuelight_create_storyboard` / `cuelight_update_storyboard` / `cuelight_delete_storyboard` / `cuelight_batch_update_storyboards`：读写分镜
- `cuelight_upload_file`：上传 workspace 内参考图片/视频/音频，返回可用于生成接口的 URL
- `cuelight_generate_image`：异步生成图片，可使用参考图 URL
- `cuelight_generate_video`：异步生成视频片段，可使用参考图 URL
- `cuelight_task_status`：查询异步生成任务的状态
- `cuelight_list_models`：查看可用的图片/视频生成模型
- `cuelight_download_original_script`：下载项目剧本原文到本地 `.cuelight/original-script/original-script.txt`，之后可用 `file_read` / `search` / `list_files` 分析原文

## CueLight 工作规范
1. 操作前先查询：修改或创建前，先用查询工具了解当前项目状态
2. 分镜 Prompt 规范：videoPrompt 应包含画面描述、镜头运动、光影氛围，使用英文撰写
3. 角色一致性：分镜中涉及的角色必须通过 referenceCharacterIds 关联
4. 生成模型选择：生图/生视频前先用 cuelight_list_models 确认可用模型
5. 异步任务：图片/视频生成是异步的，提交后告知用户 taskId，可用 cuelight_task_status 查询进度
6. 当需要分析剧本原文、查找剧情依据或回答外部原文内容时，先用 `cuelight_download_original_script` 下载原文，再用本地文件工具读取和检索，不要用 bible、episodes、角色、场景、道具或分镜等派生数据冒充原文
7. 不要请求 source chunks、source analysis、分季、keyframes、video assets、composite、export videos 或编剧/source workflow 能力；当前模式只基于本地原文读写核心资产、集数剧本和分镜
8. 使用“故事设计 / 视觉设计 / 剧本设计”等面向用户的中文术语
9. 使用中文与用户交流，videoPrompt 和技术参数使用英文"#,
        ctx.project_name,
        ctx.project_id,
        project_type,
        aspect_ratio,
        style_prompt,
        ctx.episode_count,
        ctx.storyboard_count
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn cuelight_tool_definitions_include_original_script_download() {
        let names: Vec<String> = build_cuelight_tool_definitions()
            .into_iter()
            .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
            .collect();

        assert!(names.contains(&"cuelight_download_original_script".to_string()));
    }

    #[test]
    fn cuelight_tool_specs_expose_provider_neutral_registry() {
        let specs = build_cuelight_tool_specs();
        let original_script = specs
            .iter()
            .find(|spec| spec.name == "cuelight_download_original_script")
            .expect("original script tool spec");

        assert!(original_script
            .description
            .contains("下载当前 CueLight 项目的剧本原文"));
        assert_eq!(original_script.input_schema["type"], "object");
        assert!(original_script.input_schema["properties"]
            .get("source_document_id")
            .is_some());
    }

    #[test]
    fn cuelight_system_prompt_appendix_is_business_scoped() {
        let appendix = build_cuelight_system_prompt_appendix(&CueLightThreadContext {
            project_id: "project-1".to_string(),
            project_name: "Demo Film".to_string(),
            project_type: Some("short-drama".to_string()),
            video_aspect_ratio: Some("9:16".to_string()),
            style_prompt_summary: Some("moody noir".to_string()),
            episode_count: 3,
            character_count: 2,
            storyboard_count: 12,
        });

        assert!(appendix.contains("当前绑定 CueLight 项目的业务上下文"));
        assert!(appendix.contains("增强基础 agent 的影视制作能力"));
        assert!(appendix.contains("项目名称：Demo Film"));
        assert!(appendix.contains("cuelight_project_status"));
        assert!(appendix.contains("故事设计 / 视觉设计 / 剧本设计"));
        assert!(appendix.contains("使用中文与用户交流"));
        assert!(!appendix.contains("圣经"));
        assert!(!appendix.contains("Claurst"));
        assert!(!appendix.contains("native agent runtime inside Panes"));
        assert!(!appendix.contains("通用软件/项目执行 agent"));
        assert!(!appendix.contains("Preserve user work"));
    }

    #[test]
    fn cuelight_tool_definitions_include_core_asset_tools() {
        let names: Vec<String> = build_cuelight_tool_definitions()
            .into_iter()
            .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
            .collect();

        for expected in [
            "cuelight_get_story_bible",
            "cuelight_update_story_bible",
            "cuelight_get_visual_bible",
            "cuelight_update_visual_bible",
            "cuelight_get_character",
            "cuelight_create_character",
            "cuelight_update_character",
            "cuelight_delete_character",
            "cuelight_get_scene",
            "cuelight_create_scene",
            "cuelight_update_scene",
            "cuelight_delete_scene",
            "cuelight_list_props",
            "cuelight_get_prop",
            "cuelight_create_prop",
            "cuelight_update_prop",
            "cuelight_delete_prop",
            "cuelight_get_episode",
            "cuelight_create_episode",
            "cuelight_update_episode",
            "cuelight_delete_episode",
            "cuelight_get_storyboard",
            "cuelight_delete_storyboard",
            "cuelight_batch_update_storyboards",
            "cuelight_upload_file",
        ] {
            assert!(names.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn cuelight_tool_definitions_exclude_out_of_scope_tools() {
        let names: Vec<String> = build_cuelight_tool_definitions()
            .into_iter()
            .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
            .collect();

        for excluded in [
            "grep_source_chunks",
            "search_source_chunks",
            "query_source_chunk",
            "query_source_analysis_status",
            "list_seasons",
            "query_season",
            "generate_keyframe_image",
            "generate_storyboard_video",
            "run_my_script_workflow",
            "segment_source_script",
        ] {
            assert!(
                !names.contains(&excluded.to_string()),
                "unexpectedly exposed {excluded}"
            );
        }
    }

    #[test]
    fn generation_tool_schemas_use_open_api_fields() {
        let tools = build_cuelight_tool_definitions();
        let video = tools
            .iter()
            .find(|tool| tool["function"]["name"] == "cuelight_generate_video")
            .expect("video tool");
        let video_props = &video["function"]["parameters"]["properties"];
        assert!(video_props.get("image_urls").is_some());
        assert!(video_props.get("image_url").is_none());
        assert!(video_props.get("negative_prompt").is_some());
        assert!(video_props.get("duration").is_some());
        assert!(video_props.get("resolution").is_some());
        assert!(video_props.get("aspect_ratio").is_some());
        assert!(video_props.get("seed").is_some());

        let image = tools
            .iter()
            .find(|tool| tool["function"]["name"] == "cuelight_generate_image")
            .expect("image tool");
        let image_props = &image["function"]["parameters"]["properties"];
        assert!(image_props.get("image_urls").is_some());
        assert!(image_props.get("size").is_some());
        assert!(image_props.get("aspect_ratio").is_some());

        let models = tools
            .iter()
            .find(|tool| tool["function"]["name"] == "cuelight_list_models")
            .expect("models tool");
        assert!(models["function"]["parameters"]["properties"]
            .get("media_type")
            .is_some());
    }

    #[test]
    fn upload_file_path_must_stay_under_workspace() {
        let cwd = std::env::current_dir().expect("current dir");
        let cargo_toml = resolve_existing_file_within_root(&cwd, "Cargo.toml")
            .expect("Cargo.toml should be inside src-tauri workspace");
        assert!(cargo_toml.starts_with(cwd.canonicalize().expect("canonical cwd")));

        let outside = resolve_existing_file_within_root(&cwd, "..\\Cargo.toml");
        assert!(outside.is_err());
    }

    #[test]
    fn original_script_output_dir_stays_under_workspace() {
        let cwd = std::env::current_dir().expect("current dir");
        let output_dir = original_script_output_dir(&cwd).expect("output dir");

        assert!(output_dir.ends_with(Path::new(".cuelight").join("original-script")));
        assert!(output_dir.starts_with(cwd.canonicalize().expect("canonical cwd")));
    }

    #[test]
    fn original_script_manifest_contains_expected_contract() {
        let root = PathBuf::from("C:/workspace/.cuelight/original-script");
        let script = root.join("original-script.txt");
        let manifest = root.join("manifest.json");
        let value = build_original_script_manifest(OriginalScriptManifestInput {
            project_id: "project-1",
            project_name: "Test Project",
            source_document_id: "source-1",
            filename: Some("script.txt"),
            char_count: 12,
            byte_size: 24,
            source_materials_path: "/api/projects/project-1/source-materials",
            original_path: "/api/projects/project-1/source-documents/source-1/original",
            output_dir: &root,
            script_path: &script,
            manifest_path: &manifest,
            original_text_available: true,
        });

        assert_eq!(value["projectId"], "project-1");
        assert_eq!(value["sourceDocumentId"], "source-1");
        assert_eq!(value["filename"], "script.txt");
        assert_eq!(value["charCount"], 12);
        assert_eq!(value["byteSize"], 24);
        assert_eq!(value["api"]["originalTextAvailable"], true);
        assert!(value["notes"]
            .as_str()
            .unwrap()
            .contains("derived project data"));
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct LiveVerifyStep {
        name: String,
        success: bool,
        detail: Value,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct LiveVerifyReport {
        success: bool,
        project_id: Option<String>,
        workspace_root: String,
        source_file: String,
        report_path: String,
        steps: Vec<LiveVerifyStep>,
        error: Option<String>,
    }

    struct LiveVerifyRun {
        report: LiveVerifyReport,
    }

    impl LiveVerifyRun {
        fn new(workspace_root: &Path, source_file: &Path) -> Self {
            let report_path = workspace_root
                .join(".cuelight")
                .join("panes-live-verify.json")
                .to_string_lossy()
                .to_string();
            Self {
                report: LiveVerifyReport {
                    success: false,
                    project_id: None,
                    workspace_root: workspace_root.to_string_lossy().to_string(),
                    source_file: source_file.to_string_lossy().to_string(),
                    report_path,
                    steps: Vec::new(),
                    error: None,
                },
            }
        }

        fn step(&mut self, name: impl Into<String>, success: bool, detail: Value) {
            self.report.steps.push(LiveVerifyStep {
                name: name.into(),
                success,
                detail,
            });
        }

        async fn write_report(&self) -> Result<(), String> {
            let path = PathBuf::from(&self.report.report_path);
            let Some(parent) = path.parent() else {
                return Err("live verify report path has no parent".to_string());
            };
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("failed to create report dir: {e}"))?;
            let text = serde_json::to_string_pretty(&self.report)
                .map_err(|e| format!("failed to serialize report: {e}"))?;
            tokio::fs::write(&path, text)
                .await
                .map_err(|e| format!("failed to write report: {e}"))
        }
    }

    async fn live_api_request(
        client: &reqwest::Client,
        method: &str,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let url = format!("{}{}", CUELIGHT_SERVER_URL, path);
        let mut request = match method {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "DELETE" => client.delete(&url),
            other => return Err(format!("unsupported live method: {other}")),
        };
        request = request.header("Authorization", format!("Bearer {token}"));
        request = request.header("Content-Type", "application/json");
        if let Some(body) = body {
            request = request.json(&body);
        }
        get_json_response(request.send().await).await
    }

    fn json_id(value: &Value) -> Option<String> {
        value["id"]
            .as_str()
            .or_else(|| value["data"]["id"].as_str())
            .or_else(|| value["data"][0]["id"].as_str())
            .map(str::to_string)
    }

    fn parse_tool_json(output: &str) -> Value {
        serde_json::from_str(output).unwrap_or_else(|_| json!({ "raw": output }))
    }

    fn json_array(value: &Value) -> Option<&Vec<Value>> {
        value
            .as_array()
            .or_else(|| value.get("data").and_then(Value::as_array))
    }

    fn sanitize_created_project_for_live_report(value: &Value) -> Value {
        json!({
            "id": json_id(value),
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

    async fn call_live_tool(
        run: &mut LiveVerifyRun,
        name: &str,
        args: Value,
        ctx: &CueLightThreadContext,
        root: &Path,
    ) -> Result<Value, String> {
        let (ok, output) =
            execute_cuelight_tool(name, &args, ctx, Some(root), Some("workspace-write")).await;
        let detail = parse_tool_json(&output);
        run.step(
            format!("tool:{name}"),
            ok,
            json!({ "args": args, "output": detail }),
        );
        if ok {
            Ok(parse_tool_json(&output))
        } else {
            Err(format!("{name} failed: {output}"))
        }
    }

    async fn create_live_project(
        client: &reqwest::Client,
        token: &str,
        source_file: &Path,
        source_text: &str,
    ) -> Result<Value, String> {
        let filename = source_file
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("source.txt");
        live_api_request(
            client,
            "POST",
            "/api/projects",
            token,
            Some(json!({
                "title": format!("Panes CueLight Live Verify {}", Uuid::new_v4()),
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

    fn normalize_text(value: &str) -> String {
        value.replace("\r\n", "\n").replace('\r', "\n")
    }

    async fn run_live_original_to_assets_roundtrip() -> Result<LiveVerifyRun, LiveVerifyRun> {
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
                let mut run = LiveVerifyRun::new(&workspace_root, &source_file);
                run.report.error =
                    Some("CUELIGHT_TOKEN is required for live verification".to_string());
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
        let mut run = LiveVerifyRun::new(&workspace_root, &source_file);

        let flow = async {
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
            let created = create_live_project(&client, &token, &source_file, &source_text).await?;
            let project_id = json_id(&created)
                .ok_or_else(|| format!("created project response did not include id: {created}"))?;
            run.report.project_id = Some(project_id.clone());
            run.step(
                "create-project-with-source",
                true,
                sanitize_created_project_for_live_report(&created),
            );

            let project = live_api_request(
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
                .unwrap_or("Panes CueLight Live Verify")
                .to_string();
            run.step("read-created-project", true, project.clone());

            let ctx = CueLightThreadContext {
                project_id: project_id.clone(),
                project_name,
                project_type: project["projectType"].as_str().map(str::to_string),
                video_aspect_ratio: project["videoAspectRatio"].as_str().map(str::to_string),
                style_prompt_summary: None,
                episode_count: 0,
                character_count: 0,
                storyboard_count: 0,
            };

            let materials = live_api_request(
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

            let downloaded = call_live_tool(
                &mut run,
                "cuelight_download_original_script",
                json!({}),
                &ctx,
                &workspace_root,
            )
            .await?;
            let script_path = PathBuf::from(
                downloaded["scriptPath"]
                    .as_str()
                    .ok_or_else(|| format!("download output missing scriptPath: {downloaded}"))?,
            );
            let downloaded_text = tokio::fs::read_to_string(&script_path)
                .await
                .map_err(|e| format!("failed to read downloaded original: {e}"))?;
            let expected = normalize_text(&source_text);
            let actual = normalize_text(&downloaded_text);
            if expected.trim() != actual.trim() && !actual.contains(expected.trim()) {
                return Err(format!(
                    "downloaded original did not match source; source chars={}, downloaded chars={}",
                    expected.chars().count(),
                    actual.chars().count()
                ));
            }
            for forbidden in [
                "\"characters\"",
                "\"scenes\"",
                "\"props\"",
                "\"storyboards\"",
                "\"episodes\"",
                "\"chunks\"",
            ] {
                if downloaded_text.contains(forbidden) {
                    return Err(format!("downloaded original unexpectedly contains {forbidden}"));
                }
            }
            if workspace_root
                .join(".cuelight")
                .join("original-script")
                .join("chunks")
                .exists()
            {
                return Err("original script export unexpectedly created a chunks directory".to_string());
            }
            run.step(
                "verify-downloaded-original",
                true,
                json!({
                    "scriptPath": script_path.to_string_lossy(),
                    "charCount": downloaded_text.chars().count(),
                }),
            );

            call_live_tool(
                &mut run,
                "cuelight_update_story_bible",
                json!({
                    "fields": {
                        "worldView": "Panes live verify outline: 主角在原文事件压力下完成第一集转折，测试大纲严格基于本地原文验证链路写入。",
                        "stylePrompt": "Grounded short drama, natural cinematic lighting, consistent characters, vertical 9:16 framing.",
                        "autoAttachAssets": true
                    }
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            call_live_tool(
                &mut run,
                "cuelight_get_story_bible",
                json!({}),
                &ctx,
                &workspace_root,
            )
            .await?;

            let character_a = call_live_tool(
                &mut run,
                "cuelight_create_character",
                json!({
                    "fields": {
                        "name": "测试主角",
                        "description": "来自原文样本的核心视角角色，用于验证 Panes 工具写入角色资产。",
                        "basePrompt": "Chinese short drama protagonist, grounded, expressive, consistent face"
                    }
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let character_b = call_live_tool(
                &mut run,
                "cuelight_create_character",
                json!({
                    "fields": {
                        "name": "测试对手",
                        "description": "推动第一集冲突的对立角色，用于验证多角色拆解。",
                        "basePrompt": "Chinese short drama supporting rival, sharp eyes, realistic wardrobe"
                    }
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let character_a_id = json_id(&character_a)
                .ok_or_else(|| format!("character A response missing id: {character_a}"))?;
            let character_b_id = json_id(&character_b)
                .ok_or_else(|| format!("character B response missing id: {character_b}"))?;
            call_live_tool(
                &mut run,
                "cuelight_list_characters",
                json!({}),
                &ctx,
                &workspace_root,
            )
            .await?;
            call_live_tool(
                &mut run,
                "cuelight_get_character",
                json!({ "character_id": character_a_id }),
                &ctx,
                &workspace_root,
            )
            .await?;

            let scene_a = call_live_tool(
                &mut run,
                "cuelight_create_scene",
                json!({
                    "fields": {
                        "name": "测试室内冲突场",
                        "description": "第一集主要对话与冲突发生的室内空间。",
                        "basePrompt": "modern Chinese apartment interior, tense atmosphere, practical lighting"
                    }
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let scene_b = call_live_tool(
                &mut run,
                "cuelight_create_scene",
                json!({
                    "fields": {
                        "name": "测试街道路口",
                        "description": "角色离开后发生转折的外景地点。",
                        "basePrompt": "urban Chinese street at dusk, rain reflections, cinematic realism"
                    }
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let scene_a_id =
                json_id(&scene_a).ok_or_else(|| format!("scene A response missing id: {scene_a}"))?;
            let scene_b_id =
                json_id(&scene_b).ok_or_else(|| format!("scene B response missing id: {scene_b}"))?;
            call_live_tool(
                &mut run,
                "cuelight_get_scene",
                json!({ "scene_id": scene_a_id }),
                &ctx,
                &workspace_root,
            )
            .await?;

            let prop = call_live_tool(
                &mut run,
                "cuelight_create_prop",
                json!({
                    "fields": {
                        "name": "测试关键纸条",
                        "description": "第一集推动冲突和转折的信息道具。",
                        "basePrompt": "creased handwritten note, close-up prop, realistic paper texture"
                    }
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let prop_id =
                json_id(&prop).ok_or_else(|| format!("prop response missing id: {prop}"))?;
            call_live_tool(
                &mut run,
                "cuelight_get_prop",
                json!({ "prop_id": prop_id }),
                &ctx,
                &workspace_root,
            )
            .await?;

            let episode = call_live_tool(
                &mut run,
                "cuelight_create_episode",
                json!({
                    "fields": {
                        "title": "第一集：测试开端",
                        "summary": "大纲：主角发现关键纸条，与对手在室内爆发冲突，随后在街道路口做出第一集转折选择。",
                        "beats": [
                            {
                                "id": "beat-live-1",
                                "timeRange": "0-20s",
                                "description": "测试主角发现关键纸条，意识到原本稳定的关系已经被打破。"
                            },
                            {
                                "id": "beat-live-2",
                                "timeRange": "20-45s",
                                "description": "测试对手进入，双方围绕纸条展开对峙，台词短促，情绪逐步升级。"
                            },
                            {
                                "id": "beat-live-3",
                                "timeRange": "45-60s",
                                "description": "测试主角带着纸条离开，决定主动追查真相，第一集结束。"
                            }
                        ]
                    }
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let episode_id =
                json_id(&episode).ok_or_else(|| format!("episode response missing id: {episode}"))?;
            call_live_tool(
                &mut run,
                "cuelight_update_episode",
                json!({
                    "episode_id": episode_id,
                    "fields": {
                        "content": "第一集剧本正文：\n1. 室内。测试主角发现关键纸条，意识到原本稳定的关系已经被打破。\n2. 测试对手进入，双方围绕纸条展开对峙，台词短促，情绪逐步升级。\n3. 外景街道路口。测试主角带着纸条离开，决定主动追查真相，第一集结束。"
                    }
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let episode_read = call_live_tool(
                &mut run,
                "cuelight_get_episode",
                json!({ "episode_id": episode_id }),
                &ctx,
                &workspace_root,
            )
            .await?;
            if !episode_read
                .to_string()
                .contains("第一集剧本正文")
            {
                return Err(format!("episode readback missing script content: {episode_read}"));
            }

            let storyboard_1 = call_live_tool(
                &mut run,
                "cuelight_create_storyboard",
                json!({
                    "episode_id": episode_id,
                    "video_prompt": "Interior medium shot, the protagonist finds a handwritten note on the table, slow push-in, tense realistic lighting.",
                    "reference_character_ids": [character_a_id, character_b_id],
                    "fields": {
                        "sceneNumber": 1,
                        "shotType": "中景",
                        "cameraMovement": "缓慢推进",
                        "description": "测试主角发现关键纸条，冲突即将开始。",
                        "visualPrompt": "modern Chinese apartment, handwritten note on table, tense mood, cinematic realism",
                        "dialogue": "测试主角：这张纸条是谁留下的？",
                        "referenceSceneIds": [scene_a_id]
                    }
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let storyboard_2 = call_live_tool(
                &mut run,
                "cuelight_create_storyboard",
                json!({
                    "episode_id": episode_id,
                    "video_prompt": "Exterior wide shot at a rainy street corner, protagonist walks away with the note, neon reflections, decisive ending beat.",
                    "reference_character_ids": [character_a_id],
                    "fields": {
                        "sceneNumber": 2,
                        "shotType": "远景",
                        "cameraMovement": "横移跟拍",
                        "description": "测试主角带着纸条离开，在路口做出选择。",
                        "visualPrompt": "rainy urban street corner at dusk, neon reflections, dramatic short drama ending",
                        "dialogue": "测试主角：这一次，我要自己查清楚。",
                        "referenceSceneIds": [scene_b_id]
                    }
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let storyboard_1_id = json_id(&storyboard_1)
                .ok_or_else(|| format!("storyboard 1 response missing id: {storyboard_1}"))?;
            let storyboard_2_id = json_id(&storyboard_2)
                .ok_or_else(|| format!("storyboard 2 response missing id: {storyboard_2}"))?;
            call_live_tool(
                &mut run,
                "cuelight_get_storyboard",
                json!({ "storyboard_id": storyboard_1_id }),
                &ctx,
                &workspace_root,
            )
            .await?;
            call_live_tool(
                &mut run,
                "cuelight_batch_update_storyboards",
                json!({
                    "storyboard_id": storyboard_1_id,
                    "updates": [{
                        "id": storyboard_2_id,
                        "cameraMovement": "稳定横移跟拍"
                    }]
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let storyboards = call_live_tool(
                &mut run,
                "cuelight_list_storyboards",
                json!({ "episode_id": episode_id }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let storyboard_count = json_array(&storyboards)
                .map(|items| items.len())
                .unwrap_or_default();
            if storyboard_count < 2 {
                return Err(format!(
                    "expected at least 2 storyboards after write, got {storyboard_count}: {storyboards}"
                ));
            }

            call_live_tool(
                &mut run,
                "cuelight_list_models",
                json!({ "media_type": "image" }),
                &ctx,
                &workspace_root,
            )
            .await?;
            call_live_tool(
                &mut run,
                "cuelight_list_models",
                json!({ "media_type": "video" }),
                &ctx,
                &workspace_root,
            )
            .await?;

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
                        "paid image/video generation"
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
    async fn cuelight_live_original_to_assets_roundtrip() {
        match run_live_original_to_assets_roundtrip().await {
            Ok(run) => {
                eprintln!(
                    "CueLight live verification passed. projectId={:?} report={}",
                    run.report.project_id, run.report.report_path
                );
            }
            Err(run) => {
                panic!(
                    "CueLight live verification failed. projectId={:?} report={} error={:?}",
                    run.report.project_id, run.report.report_path, run.report.error
                );
            }
        }
    }
}
