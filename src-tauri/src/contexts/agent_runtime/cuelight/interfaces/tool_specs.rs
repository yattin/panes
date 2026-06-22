use panes_agent::ToolSpec;
use serde_json::{json, Value};

#[cfg(test)]
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
        .filter(|tool| {
            tool["function"]["name"]
                .as_str()
                .map(|name| !is_retired_cuelight_tool_name(name))
                .unwrap_or(true)
        })
        .filter_map(openai_function_to_tool_spec)
        .collect()
}

pub fn is_cuelight_tool_name(name: &str) -> bool {
    build_cuelight_tool_specs()
        .iter()
        .any(|spec| spec.name == name)
}

fn is_retired_cuelight_tool_name(name: &str) -> bool {
    matches!(
        name,
        "cuelight_project_status"
            | "cuelight_get_story_bible"
            | "cuelight_update_story_bible"
            | "cuelight_get_visual_bible"
            | "cuelight_update_visual_bible"
            | "cuelight_list_characters"
            | "cuelight_get_character"
            | "cuelight_create_character"
            | "cuelight_update_character"
            | "cuelight_delete_character"
            | "cuelight_list_scenes"
            | "cuelight_get_scene"
            | "cuelight_create_scene"
            | "cuelight_update_scene"
            | "cuelight_delete_scene"
            | "cuelight_list_props"
            | "cuelight_get_prop"
            | "cuelight_create_prop"
            | "cuelight_update_prop"
            | "cuelight_delete_prop"
            | "cuelight_list_episodes"
            | "cuelight_get_episode"
            | "cuelight_create_episode"
            | "cuelight_update_episode"
            | "cuelight_delete_episode"
            | "cuelight_list_storyboards"
            | "cuelight_get_storyboard"
            | "cuelight_create_storyboard"
            | "cuelight_update_storyboard"
            | "cuelight_delete_storyboard"
            | "cuelight_batch_update_storyboards"
            | "save_character"
            | "save_scene"
    )
}

fn build_cuelight_openai_tool_definitions() -> Vec<Value> {
    let mut tools = vec![json!({
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
    })];
    tools.extend(build_drama_openai_tool_definitions());
    tools.extend(vec![
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
    ]);
    tools
}

fn build_drama_openai_tool_definitions() -> Vec<Value> {
    vec![
        simple_tool_definition(
            "query_project_state",
            "读取当前项目真实状态。drama 主代理做任何保存前优先调用，用于判断故事、视觉、资产、分集、正文和分镜缺口。",
        ),
        simple_tool_definition("query_story_bible", "读取故事设计、世界观和叙事基础设定。"),
        simple_tool_definition("query_visual_bible", "读取视觉设计、stylePrompt、画幅和视频时长能力等视觉基准。"),
        json!({
            "type": "function",
            "function": {
                "name": "list_assets",
                "description": "按类型列出角色、场景或道具资产。更新已有资产前先用它定位真实 ID。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["character", "scene", "prop", "all"],
                            "description": "资产类型；all 返回完整项目状态中的资产索引。"
                        },
                        "filter": {
                            "type": "string",
                            "description": "可选名称关键词；当前适配层透传查询后由模型筛选。"
                        }
                    },
                    "required": []
                }
            }
        }),
        entity_tool_definition("query_character", "读取角色详情。", "character_id", "角色 ID", None),
        entity_tool_definition("query_scene", "读取场景详情。", "scene_id", "场景 ID", None),
        entity_tool_definition("query_prop", "读取道具详情。", "prop_id", "道具 ID", None),
        simple_tool_definition("list_episode_outlines", "列出分集大纲、节拍和正文状态。批量更新大纲前先调用。"),
        json!({
            "type": "function",
            "function": {
                "name": "query_episode",
                "description": "读取单集详情和正文。可传 episode_id 或 episode_number；保存正文、写分镜前必须先查询。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "episode_id": { "type": "string", "description": "集数 ID，若已知则优先传。" },
                        "episode_number": { "type": "integer", "description": "集号，未知 ID 时传。" }
                    },
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "query_storyboards",
                "description": "读取某集已有分镜列表。可传 episode_id 或 episode_number。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "episode_id": { "type": "string", "description": "集数 ID。" },
                        "episode_number": { "type": "integer", "description": "集号。" }
                    },
                    "required": []
                }
            }
        }),
        entity_tool_definition("query_storyboard", "读取单个分镜详情。", "storyboard_id", "分镜 ID", None),
        json!({
            "type": "function",
            "function": {
                "name": "save_story_blueprint",
                "description": "保存故事基础设定。用于 proposal、design、worldView、stylePrompt 等文字基准；只写入传入字段。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "proposal": { "type": "string" },
                        "design": { "type": "string" },
                        "worldView": { "type": "string" },
                        "stylePrompt": { "type": "string" }
                    },
                    "required": []
                }
            }
        }),
        drama_asset_save_tool_definition("save_drama_character", "保存 drama 角色档案。新建不需要 ID；覆盖已有角色必须先 query_character。"),
        drama_asset_save_tool_definition("save_drama_scene", "保存 drama 场景档案。新建不需要 ID；覆盖已有场景必须先 query_scene。"),
        drama_asset_save_tool_definition("save_prop", "保存道具档案。兼容 visualPrompt，会落到 basePrompt。"),
        json!({
            "type": "function",
            "function": {
                "name": "save_episode_outline_batch",
                "description": "批量保存分集大纲和节拍；每次最多 5 集。正文保存前必须先有 summary 和 beats。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "outlines": {
                            "type": "array",
                            "maxItems": 5,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "number": { "type": "integer" },
                                    "title": { "type": "string" },
                                    "summary": { "type": "string" },
                                    "beats": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "id": { "type": "string" },
                                                "timeRange": { "type": "string" },
                                                "description": { "type": "string" }
                                            },
                                            "required": ["id", "timeRange", "description"]
                                        }
                                    }
                                },
                                "required": ["number", "title", "summary"]
                            }
                        }
                    },
                    "required": ["outlines"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "save_episode_text",
                "description": "保存单集正文。必须是可拍摄正文，不是摘要；保存前应 query_episode。长正文必须先用 file_write 写入 workspace，再传 contentPath 导入。REST 路径要求该集已有 summary 和 beats。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "episodeNumber": { "type": "integer" },
                        "episode_number": { "type": "integer" },
                        "title": { "type": "string" },
                        "summary": { "type": "string" },
                        "content": { "type": "string" },
                        "contentPath": { "type": "string", "description": "workspace 相对路径，如 .cuelight/drafts/episode-1-script.txt。与 content 二选一。" },
                        "content_path": { "type": "string", "description": "contentPath 的 snake_case 兼容字段。" },
                        "sourceRefs": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["title"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "generate_visual_style_prompt",
                "description": "根据项目和用户偏好生成全局视觉文字基准。本工具只返回文本，不自动落库；保存请继续调用 update_visual_bible。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "visualStyle": { "type": "string" },
                        "shootingMode": { "type": "string" },
                        "preference": { "type": "string" }
                    },
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "update_visual_bible",
                "description": "保存全局视觉文字基准、stylePrompt、visualMode 等视觉字段。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "fields": { "type": "object", "description": "视觉字段对象，例如 stylePrompt、visualStyle、visualMode。" },
                        "stylePrompt": { "type": "string" },
                        "visualStyle": { "type": "string" },
                        "visualMode": { "type": "string" }
                    },
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "save_storyboard_scripts",
                "description": "追加保存分镜脚本文本。inline 每次 1-3 条；整集或长分镜必须先用 file_write 写入 JSON 文件，再传 storyboardsPath 导入。保存前先 query_episode/query_visual_bible/list_assets。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "episodeId": { "type": "string" },
                        "episode_id": { "type": "string" },
                        "episodeNumber": { "type": "integer" },
                        "episode_number": { "type": "integer" },
                        "storyboardsPath": { "type": "string", "description": "workspace 相对 JSON 文件路径，内容为数组或 { storyboards: [...] }。与 storyboards 二选一。" },
                        "storyboards_path": { "type": "string", "description": "storyboardsPath 的 snake_case 兼容字段。" },
                        "storyboards": {
                            "type": "array",
                            "minItems": 1,
                            "maxItems": 3,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "sceneNumber": { "type": "integer" },
                                    "videoPrompt": { "type": "string" },
                                    "scriptExcerpt": { "type": "string" },
                                    "plannedVideoDurationSeconds": { "type": "integer" },
                                    "shotSize": { "type": "string" },
                                    "dialogues": { "type": "array" },
                                    "soundEffects": { "type": "array" },
                                    "referenceCharacterIds": { "type": "array", "items": { "type": "string" } },
                                    "referenceSceneId": { "type": "string" },
                                    "referencePropIds": { "type": "array", "items": { "type": "string" } }
                                },
                                "required": ["videoPrompt"]
                            }
                        }
                    },
                    "required": ["storyboards"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "update_storyboard_script",
                "description": "更新已有单镜 videoPrompt 或分镜脚本文字。调用前必须 query_storyboard。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "storyboardId": { "type": "string" },
                        "storyboard_id": { "type": "string" },
                        "videoPrompt": { "type": "string" },
                        "fields": { "type": "object" }
                    },
                    "required": ["videoPrompt"]
                }
            }
        }),
    ]
}

fn simple_tool_definition(name: &str, description: &str) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }
    })
}

fn drama_asset_save_tool_definition(name: &str, description: &str) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "已有资产 ID；新建时省略。" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "basePrompt": { "type": "string" },
                    "visualPrompt": { "type": "string", "description": "兼容字段，会映射到 basePrompt。" },
                    "voicePrompt": { "type": "string" },
                    "referenceImageUrl": { "type": "string" }
                },
                "required": ["name"]
            }
        }
    })
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
