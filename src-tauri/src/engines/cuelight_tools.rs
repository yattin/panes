use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
    GLOBAL_AUTH_TOKEN.lock().ok().and_then(|guard| guard.clone())
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
    pub async fn from_binding(
        binding: &CueLightBindingDto,
    ) -> Result<Self, String> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/projects/{}",
            CUELIGHT_SERVER_URL,
            binding.project_id
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
pub fn build_cuelight_tool_definitions() -> Vec<Value> {
    vec![
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
                "name": "cuelight_list_characters",
                "description": "列出项目中的所有角色及其参考图",
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
                "name": "cuelight_list_scenes",
                "description": "列出项目中的所有场景及其参考图",
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
                "name": "cuelight_list_episodes",
                "description": "列出项目的所有集数及剧本状态",
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
                        }
                    },
                    "required": ["episode_id", "video_prompt"]
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
                        }
                    },
                    "required": ["storyboard_id"]
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
                        "image_url": {
                            "type": "string",
                            "description": "参考图片 URL（可选）"
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
                    "properties": {},
                    "required": []
                }
            }
        }),
    ]
}

/// 执行 CueLight 工具调用
pub async fn execute_cuelight_tool(
    name: &str,
    args: &Value,
    ctx: &CueLightThreadContext,
) -> (bool, String) {
    let client = reqwest::Client::new();
    let server_url = CUELIGHT_SERVER_URL;

    let make_request = |method: &str, path: &str, body: Option<Value>| {
        let url = format!("{}{}", server_url, path);
        let mut req = match method {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
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
        "cuelight_project_status" => {
            let resp = make_request("GET", &format!("/api/projects/{}", ctx.project_id), None)
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
        "cuelight_create_storyboard" => {
            let episode_id = args["episode_id"].as_str().unwrap_or("");
            if episode_id.is_empty() {
                return (false, "episode_id is required".to_string());
            }
            let body = json!({
                "videoPrompt": args["video_prompt"],
                "referenceCharacterIds": args.get("reference_character_ids")
            });
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
            let resp = make_request(
                "PUT",
                &format!("/api/storyboards/{}", storyboard_id),
                Some(body),
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "cuelight_generate_image" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            if prompt.is_empty() {
                return (false, "prompt is required".to_string());
            }
            let mut body = json!({ "prompt": prompt });
            if let Some(model) = args.get("model") {
                body["model"] = model.clone();
            }
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
            if let Some(model) = args.get("model") {
                body["model"] = model.clone();
            }
            if let Some(image_url) = args.get("image_url") {
                body["imageUrl"] = image_url.clone();
            }
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
            let resp = make_request("GET", "/v1/models", None).send().await;
            handle_response(resp).await
        }
        _ => (false, format!("unknown cuelight tool: {}", name)),
    };

    result
}

async fn handle_response(
    resp: Result<reqwest::Response, reqwest::Error>,
) -> (bool, String) {
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

/// 构建 CueLight 影视模式的 system prompt
pub fn build_cuelight_system_prompt(ctx: &CueLightThreadContext) -> String {
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
        r#"你是一个 AI 影视创作助手，运行在 CueLight 影视制作平台上。

## 身份与职责
你是专业的 AI 影视导演/编剧助手，帮助用户完成短剧/短视频的全流程制作：
- 剧本创作：世界观构建、角色设计、场景规划、分集剧本撰写
- 视觉设计：角色参考图生成、场景氛围图生成、视觉风格设定
- 分镜制作：为每集编写分镜脚本（videoPrompt）、设定镜头语言
- 视频生成：提交图片/视频生成任务、管理生成进度

## 当前项目
- 项目名称：{}
- 项目 ID：{}
{}
{}
{}
- 集数：{} 集
- 分镜数：{} 个

## 可用工具
你拥有以下 CueLight 影视制作工具：
- `cuelight_project_status`：查看项目完整状态和进度
- `cuelight_list_characters`：列出项目中的所有角色及其参考图
- `cuelight_list_scenes`：列出所有场景及其参考图
- `cuelight_list_episodes`：列出所有集数及剧本状态
- `cuelight_list_storyboards`：查看某集的分镜列表
- `cuelight_create_storyboard`：为某集创建新分镜
- `cuelight_update_storyboard`：更新分镜的 videoPrompt、关联角色等
- `cuelight_generate_image`：异步生成图片（角色参考图、场景图等）
- `cuelight_generate_video`：异步生成视频片段
- `cuelight_task_status`：查询异步生成任务的状态
- `cuelight_list_models`：查看可用的图片/视频生成模型

## 工作规范
1. 操作前先查询：修改或创建前，先用查询工具了解当前项目状态
2. 分镜 Prompt 规范：videoPrompt 应包含画面描述、镜头运动、光影氛围，使用英文撰写
3. 角色一致性：分镜中涉及的角色必须通过 referenceCharacterIds 关联
4. 生成模型选择：生图/生视频前先用 cuelight_list_models 确认可用模型
5. 异步任务：图片/视频生成是异步的，提交后告知用户 taskId，可用 cuelight_task_status 查询进度
6. 使用中文与用户交流，videoPrompt 和技术参数使用英文"#,
        ctx.project_name,
        ctx.project_id,
        project_type,
        aspect_ratio,
        style_prompt,
        ctx.episode_count,
        ctx.storyboard_count
    )
}
