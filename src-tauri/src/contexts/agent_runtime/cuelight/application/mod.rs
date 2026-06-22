use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

use super::domain::CueLightThreadContext;
use super::infrastructure::auth::get_global_auth_token;
use super::infrastructure::CUELIGHT_SERVER_URL;

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
        "query_project_state" => {
            let resp = make_request("GET", &format!("/api/projects/{}", ctx.project_id), None)
                .send()
                .await;
            handle_response(resp).await
        }
        "query_story_bible" | "query_visual_bible" => {
            let resp = make_request(
                "GET",
                &format!("/api/projects/{}/bible", ctx.project_id),
                None,
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "list_assets" => match args.get("type").and_then(Value::as_str).unwrap_or("all") {
            "character" => {
                let resp = make_request(
                    "GET",
                    &format!("/api/projects/{}/characters", ctx.project_id),
                    None,
                )
                .send()
                .await;
                handle_response(resp).await
            }
            "scene" => {
                let resp = make_request(
                    "GET",
                    &format!("/api/projects/{}/scenes", ctx.project_id),
                    None,
                )
                .send()
                .await;
                handle_response(resp).await
            }
            "prop" => {
                let resp = make_request(
                    "GET",
                    &format!("/api/projects/{}/props", ctx.project_id),
                    None,
                )
                .send()
                .await;
                handle_response(resp).await
            }
            "all" => {
                let resp = make_request("GET", &format!("/api/projects/{}", ctx.project_id), None)
                    .send()
                    .await;
                handle_response(resp).await
            }
            other => (
                false,
                structured_blocked(
                    "invalid_asset_type",
                    &format!("unsupported asset type `{other}`"),
                    "type must be character, scene, prop, or all",
                    None,
                ),
            ),
        },
        "query_character" => {
            get_item_from_project_list(&client, server_url, ctx, "characters", "character_id", args)
                .await
        }
        "query_scene" => {
            get_item_from_project_list(&client, server_url, ctx, "scenes", "scene_id", args).await
        }
        "query_prop" => {
            get_item_from_project_list(&client, server_url, ctx, "props", "prop_id", args).await
        }
        "list_episode_outlines" => {
            let resp = make_request(
                "GET",
                &format!("/api/projects/{}/episodes", ctx.project_id),
                None,
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "query_episode" => {
            let episode_id = match resolve_episode_id(&client, server_url, ctx, args).await {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let resp = make_request("GET", &format!("/api/episodes/{}", episode_id), None)
                .send()
                .await;
            handle_response(resp).await
        }
        "query_storyboards" => {
            let episode_id = match resolve_episode_id(&client, server_url, ctx, args).await {
                Ok(value) => value,
                Err(err) => return (false, err),
            };
            let resp = make_request(
                "GET",
                &format!("/api/episodes/{}/storyboards", episode_id),
                None,
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "query_storyboard" => {
            let storyboard_id = match required_string_arg(args, "storyboard_id") {
                Ok(value) => value,
                Err(_) => match required_string_arg(args, "storyboardId") {
                    Ok(value) => value,
                    Err(err) => return (false, err),
                },
            };
            let resp = make_request("GET", &format!("/api/storyboards/{}", storyboard_id), None)
                .send()
                .await;
            handle_response(resp).await
        }
        "save_story_blueprint" => {
            let mut body = json!({});
            copy_optional_fields(
                args,
                &mut body,
                &["worldView", "stylePrompt", "proposal", "design"],
            );
            if body.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                return (
                    false,
                    structured_blocked(
                        "empty_story_blueprint",
                        "没有可保存的故事基础字段。",
                        "传入 worldView、stylePrompt、proposal 或 design 后重试。",
                        Some("save_story_blueprint"),
                    ),
                );
            }
            let resp = make_request(
                "PUT",
                &format!("/api/projects/{}/bible", ctx.project_id),
                Some(body),
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "save_drama_character" => {
            save_drama_asset(&client, server_url, ctx, "characters", "character", args).await
        }
        "save_drama_scene" => {
            save_drama_asset(&client, server_url, ctx, "scenes", "scene", args).await
        }
        "save_prop" => save_drama_asset(&client, server_url, ctx, "props", "prop", args).await,
        "save_episode_outline_batch" => {
            save_episode_outline_batch(&client, server_url, ctx, args).await
        }
        "save_episode_text" => save_episode_text(&client, server_url, ctx, args, root).await,
        "generate_visual_style_prompt" => {
            let visual_style = args
                .get("visualStyle")
                .and_then(Value::as_str)
                .unwrap_or("grounded cinematic short drama");
            let shooting_mode = args
                .get("shootingMode")
                .and_then(Value::as_str)
                .unwrap_or("live-action vertical drama");
            let preference = args
                .get("preference")
                .and_then(Value::as_str)
                .unwrap_or("consistent characters, natural lighting, production-ready continuity");
            (
                true,
                json!({
                    "success": true,
                    "saved": false,
                    "stylePrompt": format!("{visual_style}; {shooting_mode}; {preference}; coherent art direction, reusable character and scene visual baseline, no subtitles, no watermarks."),
                    "nextAction": "Call update_visual_bible with this stylePrompt to save it."
                })
                .to_string(),
            )
        }
        "update_visual_bible" => {
            let mut body = body_from_optional_fields(args);
            copy_optional_fields(
                args,
                &mut body,
                &["stylePrompt", "visualStyle", "visualMode"],
            );
            if body.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                return (
                    false,
                    structured_blocked(
                        "empty_visual_bible_update",
                        "没有可保存的视觉字段。",
                        "传入 fields.stylePrompt 或 stylePrompt 后重试。",
                        Some("update_visual_bible"),
                    ),
                );
            }
            let resp = make_request(
                "PUT",
                &format!("/api/projects/{}/bible", ctx.project_id),
                Some(body),
            )
            .send()
            .await;
            handle_response(resp).await
        }
        "save_storyboard_scripts" => {
            save_storyboard_scripts(&client, server_url, ctx, args, root).await
        }
        "update_storyboard_script" => {
            let storyboard_id = optional_string_arg(args, "storyboardId")
                .or_else(|| optional_string_arg(args, "storyboard_id"));
            let Some(storyboard_id) = storyboard_id else {
                return (false, "storyboardId is required".to_string());
            };
            let mut body = body_from_optional_fields(args);
            if let Some(prompt) = args.get("videoPrompt") {
                body["videoPrompt"] = prompt.clone();
            }
            if body.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                return (
                    false,
                    structured_blocked(
                        "empty_storyboard_update",
                        "没有可更新的分镜字段。",
                        "传入 videoPrompt 或 fields 后重试。",
                        Some("update_storyboard_script"),
                    ),
                );
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

pub(crate) fn original_script_output_dir(root: &Path) -> Result<PathBuf, String> {
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

fn optional_string_arg(args: &Value, name: &str) -> Option<String> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn optional_i64_arg(args: &Value, names: &[&str]) -> Option<i64> {
    names.iter().find_map(|name| {
        args.get(*name)
            .and_then(|value| value.as_i64().or_else(|| value.as_u64().map(|n| n as i64)))
    })
}

fn structured_blocked(
    reason: &str,
    message: &str,
    guidance: &str,
    retry_tool: Option<&str>,
) -> String {
    let mut value = json!({
        "success": false,
        "blocked": true,
        "reason": reason,
        "message": message,
        "userMessage": message,
        "guidance": guidance,
        "nextAction": "fix_then_retry",
    });
    if let Some(retry_tool) = retry_tool {
        value["retryTool"] = json!(retry_tool);
    }
    value.to_string()
}

async fn resolve_episode_id(
    client: &reqwest::Client,
    server_url: &str,
    ctx: &CueLightThreadContext,
    args: &Value,
) -> Result<String, String> {
    if let Some(id) =
        optional_string_arg(args, "episode_id").or_else(|| optional_string_arg(args, "episodeId"))
    {
        return Ok(id);
    }
    let Some(number) = optional_i64_arg(args, &["episode_number", "episodeNumber", "number"])
    else {
        return Err(structured_blocked(
            "missing_episode_identifier",
            "需要 episode_id 或 episodeNumber 才能定位剧集。",
            "先调用 list_episode_outlines，或传入 query_episode 返回的真实 episode.id。",
            Some("query_episode"),
        ));
    };
    let payload = get_json_response(
        make_cuelight_request(
            client,
            server_url,
            "GET",
            &format!("/api/projects/{}/episodes", ctx.project_id),
            None,
        )
        .send()
        .await,
    )
    .await?;
    let episodes = payload
        .as_array()
        .or_else(|| payload.get("data").and_then(Value::as_array))
        .ok_or_else(|| "episode list response was not an array".to_string())?;
    episodes
        .iter()
        .find(|episode| episode.get("number").and_then(Value::as_i64) == Some(number))
        .and_then(|episode| episode.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .ok_or_else(|| {
            structured_blocked(
                "episode_not_found",
                &format!("没有找到第 {number} 集。"),
                "先调用 save_episode_outline_batch 创建该集大纲，或确认集号后重试。",
                Some("save_episode_outline_batch"),
            )
        })
}

async fn load_episode_by_id(
    client: &reqwest::Client,
    server_url: &str,
    episode_id: &str,
) -> Result<Value, String> {
    get_json_response(
        make_cuelight_request(
            client,
            server_url,
            "GET",
            &format!("/api/episodes/{episode_id}"),
            None,
        )
        .send()
        .await,
    )
    .await
}

async fn save_drama_asset(
    client: &reqwest::Client,
    server_url: &str,
    ctx: &CueLightThreadContext,
    collection: &str,
    id_arg_prefix: &str,
    args: &Value,
) -> (bool, String) {
    let mut body = json!({});
    copy_optional_fields(
        args,
        &mut body,
        &[
            "name",
            "description",
            "basePrompt",
            "voicePrompt",
            "referenceImageUrl",
        ],
    );
    if body.get("basePrompt").is_none() {
        if let Some(visual_prompt) = args.get("visualPrompt") {
            body["basePrompt"] = visual_prompt.clone();
        }
    }
    if body.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        return (
            false,
            structured_blocked(
                "empty_asset_payload",
                "没有可保存的资产字段。",
                "至少传入 name、description 或 basePrompt。",
                None,
            ),
        );
    }
    let id_key = format!("{id_arg_prefix}_id");
    let id = optional_string_arg(args, "id")
        .or_else(|| optional_string_arg(args, &id_key))
        .or_else(|| optional_string_arg(args, &format!("{id_arg_prefix}Id")));
    let (method, path) = if let Some(id) = id {
        (
            "PUT",
            format!("/api/projects/{}/{}/{}", ctx.project_id, collection, id),
        )
    } else {
        (
            "POST",
            format!("/api/projects/{}/{}", ctx.project_id, collection),
        )
    };
    let resp = make_cuelight_request(client, server_url, method, &path, Some(body))
        .send()
        .await;
    handle_response(resp).await
}

async fn save_episode_outline_batch(
    client: &reqwest::Client,
    server_url: &str,
    ctx: &CueLightThreadContext,
    args: &Value,
) -> (bool, String) {
    let Some(outlines) = args.get("outlines").and_then(Value::as_array) else {
        return (false, "outlines array is required".to_string());
    };
    if outlines.is_empty() || outlines.len() > 5 {
        return (
            false,
            structured_blocked(
                "invalid_episode_outline_batch_size",
                "save_episode_outline_batch 每次必须保存 1-5 集。",
                "把大纲拆成最多 5 集一批后重试。",
                Some("save_episode_outline_batch"),
            ),
        );
    }
    let patch_episodes: Vec<Value> = outlines
        .iter()
        .map(|outline| {
            json!({
                "number": outline["number"],
                "title": outline["title"],
                "summary": outline["summary"],
                "seasonId": outline.get("seasonId").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();
    let resp = make_cuelight_request(
        client,
        server_url,
        "PATCH",
        &format!("/api/projects/{}/episodes", ctx.project_id),
        Some(json!({ "episodes": patch_episodes })),
    )
    .send()
    .await;
    let (ok, output) = handle_response(resp).await;
    if !ok {
        return (ok, output);
    }

    let saved = parse_json_or_raw(&output);
    let episodes = saved
        .as_array()
        .or_else(|| saved.get("data").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default();
    let mut updated_beats = Vec::new();
    for outline in outlines {
        let number = outline.get("number").and_then(Value::as_i64);
        let beats = outline.get("beats").cloned().unwrap_or_else(|| json!([]));
        if number.is_none()
            || !beats
                .as_array()
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        {
            continue;
        }
        let Some(episode_id) = episodes
            .iter()
            .find(|episode| episode.get("number").and_then(Value::as_i64) == number)
            .and_then(|episode| episode.get("id").and_then(Value::as_str))
        else {
            continue;
        };
        let body = json!({
            "title": outline["title"],
            "summary": outline["summary"],
            "beats": beats,
        });
        let resp = make_cuelight_request(
            client,
            server_url,
            "PUT",
            &format!("/api/episodes/{episode_id}"),
            Some(body),
        )
        .send()
        .await;
        let (beats_ok, beats_output) = handle_response(resp).await;
        if !beats_ok {
            return (false, beats_output);
        }
        updated_beats.push(parse_json_or_raw(&beats_output));
    }
    (
        true,
        json!({
            "success": true,
            "saved": true,
            "toolName": "save_episode_outline_batch",
            "episodes": saved,
            "beatsUpdated": updated_beats,
        })
        .to_string(),
    )
}

async fn save_episode_text(
    client: &reqwest::Client,
    server_url: &str,
    ctx: &CueLightThreadContext,
    args: &Value,
    root: Option<&Path>,
) -> (bool, String) {
    let inline_content = optional_string_arg(args, "content");
    let content_path = optional_string_arg(args, "contentPath")
        .or_else(|| optional_string_arg(args, "content_path"));
    let content = match (inline_content, content_path) {
        (Some(_), Some(_)) => {
            return (
                false,
                structured_blocked(
                    "ambiguous_episode_text_source",
                    "content 和 contentPath 只能二选一。",
                    "长正文请先用 file_write 写入 .cuelight/drafts/，再只传 contentPath。",
                    Some("save_episode_text"),
                ),
            )
        }
        (Some(value), None) => value,
        (None, Some(path)) => match read_workspace_text_file(root, &path, "contentPath").await {
            Ok(value) => value,
            Err(err) => return (false, err),
        },
        (None, None) => {
            return (
                false,
                structured_blocked(
                    "missing_episode_text_source",
                    "缺少正文内容。",
                    "传入 content，或先用 file_write 写入 workspace 相对路径后传 contentPath。",
                    Some("save_episode_text"),
                ),
            )
        }
    };
    if content.chars().count() < 20 {
        return (
            false,
            structured_blocked(
                "episode_text_too_short",
                "正文太短，不能保存为单集剧本。",
                "传入可拍摄的场次、动作、对白和旁白正文，至少 20 字。",
                Some("save_episode_text"),
            ),
        );
    }
    let episode_id = match resolve_episode_id(client, server_url, ctx, args).await {
        Ok(value) => value,
        Err(err) => return (false, err),
    };
    let episode = match load_episode_by_id(client, server_url, &episode_id).await {
        Ok(value) => value,
        Err(err) => return (false, err),
    };
    let summary = optional_string_arg(args, "summary").or_else(|| {
        episode
            .get("summary")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let beats_ready = episode
        .get("beats")
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false);
    if summary.as_deref().map(str::trim).unwrap_or("").is_empty() || !beats_ready {
        return (
            false,
            structured_blocked(
                "missing_episode_outline_or_beats",
                "保存正文前必须先补充分集梗概和节拍。",
                "先调用 save_episode_outline_batch 为该集保存 title、summary 和 beats，再重试 save_episode_text。",
                Some("save_episode_outline_batch"),
            ),
        );
    }
    let title = optional_string_arg(args, "title")
        .or_else(|| {
            episode
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "未命名剧集".to_string());
    let body = json!({
        "title": title,
        "summary": summary,
        "content": content,
        "contentSourceMode": "generated",
        "sourceRefs": args.get("sourceRefs").cloned().unwrap_or_else(|| json!([])),
    });
    let resp = make_cuelight_request(
        client,
        server_url,
        "PUT",
        &format!("/api/episodes/{episode_id}"),
        Some(body),
    )
    .send()
    .await;
    let (ok, output) = handle_response(resp).await;
    if !ok {
        return (ok, output);
    }
    (
        true,
        json!({
            "success": true,
            "saved": true,
            "toolName": "save_episode_text",
            "episodeId": episode_id,
            "episode": parse_json_or_raw(&output),
        })
        .to_string(),
    )
}

async fn save_storyboard_scripts(
    client: &reqwest::Client,
    server_url: &str,
    ctx: &CueLightThreadContext,
    args: &Value,
    root: Option<&Path>,
) -> (bool, String) {
    let inline_storyboards = args.get("storyboards").and_then(Value::as_array);
    let storyboards_path = optional_string_arg(args, "storyboardsPath")
        .or_else(|| optional_string_arg(args, "storyboards_path"));
    let items: Vec<Value> = match (inline_storyboards, storyboards_path) {
        (Some(_), Some(_)) => {
            return (
                false,
                structured_blocked(
                    "ambiguous_storyboards_source",
                    "storyboards 和 storyboardsPath 只能二选一。",
                    "整集分镜请先用 file_write 写入 JSON 文件，再只传 storyboardsPath。",
                    Some("save_storyboard_scripts"),
                ),
            )
        }
        (Some(items), None) => {
            if items.is_empty() || items.len() > 3 {
                return (
                    false,
                    structured_blocked(
                        "invalid_storyboard_batch_size",
                        "inline save_storyboard_scripts 每次只能保存 1-3 条分镜。",
                        "整集分镜请先写入 .cuelight/drafts/episode-N-storyboards.json，再传 storyboardsPath。",
                        Some("save_storyboard_scripts"),
                    ),
                );
            }
            items.clone()
        }
        (None, Some(path)) => match read_storyboards_from_workspace_file(root, &path).await {
            Ok(items) => items,
            Err(err) => return (false, err),
        },
        (None, None) => {
            return (
                false,
                "storyboards array or storyboardsPath is required".to_string(),
            )
        }
    };
    if items.is_empty() {
        return (
            false,
            structured_blocked(
                "invalid_storyboard_batch_size",
                "save_storyboard_scripts 至少需要 1 条分镜。",
                "传入 storyboards，或用 storyboardsPath 指向包含分镜数组的 JSON 文件。",
                Some("save_storyboard_scripts"),
            ),
        );
    }
    let episode_id = match resolve_episode_id(client, server_url, ctx, args).await {
        Ok(value) => value,
        Err(err) => return (false, err),
    };
    let mut saved = Vec::new();
    for item in &items {
        let Some(prompt) = item.get("videoPrompt").and_then(Value::as_str) else {
            return (
                false,
                structured_blocked(
                    "empty_storyboard_prompt",
                    "分镜 videoPrompt 不能为空。",
                    "为每条分镜写入可渲染的七要素链条和声音设计后重试。",
                    Some("save_storyboard_scripts"),
                ),
            );
        };
        if prompt.trim().is_empty() {
            return (
                false,
                structured_blocked(
                    "empty_storyboard_prompt",
                    "分镜 videoPrompt 不能为空。",
                    "为每条分镜写入可渲染的七要素链条和声音设计后重试。",
                    Some("save_storyboard_scripts"),
                ),
            );
        }
        let mut body = json!({ "videoPrompt": prompt });
        copy_optional_fields(
            item,
            &mut body,
            &[
                "sceneNumber",
                "scriptExcerpt",
                "plannedVideoDurationSeconds",
                "shotSize",
                "dialogues",
                "soundEffects",
                "frameMode",
                "firstFrameSource",
                "referenceCharacterIds",
                "referenceSceneId",
                "referencePropIds",
            ],
        );
        let resp = make_cuelight_request(
            client,
            server_url,
            "POST",
            &format!("/api/episodes/{episode_id}/storyboards"),
            Some(body),
        )
        .send()
        .await;
        let (ok, output) = handle_response(resp).await;
        if !ok {
            return (false, output);
        }
        saved.push(parse_json_or_raw(&output));
    }
    let next_scene_number = saved
        .iter()
        .filter_map(|item| item.get("sceneNumber").and_then(Value::as_i64))
        .max()
        .map(|value| value + 1);
    (
        true,
        json!({
            "success": true,
            "saved": true,
            "toolName": "save_storyboard_scripts",
            "episodeId": episode_id,
            "storyboards": saved,
            "nextSceneNumber": next_scene_number,
        })
        .to_string(),
    )
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

pub(crate) fn resolve_existing_file_within_root(
    root: &Path,
    requested: &str,
) -> Result<PathBuf, String> {
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

fn resolve_existing_relative_file_within_root(
    root: Option<&Path>,
    requested: &str,
    field_name: &str,
) -> Result<PathBuf, String> {
    let Some(root) = root else {
        return Err(format!("{field_name} requires a workspace root"));
    };
    let requested_path = Path::new(requested);
    if requested_path.is_absolute() {
        return Err(format!("{field_name} path must be relative"));
    }
    if requested_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("{field_name} path escapes workspace root"));
    }
    resolve_existing_file_within_root(root, requested)
        .map_err(|err| err.replace("upload path", field_name))
}

async fn read_workspace_text_file(
    root: Option<&Path>,
    requested: &str,
    field_name: &str,
) -> Result<String, String> {
    let path = resolve_existing_relative_file_within_root(root, requested, field_name)?;
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("failed to read {field_name}: {e}"))
}

async fn read_storyboards_from_workspace_file(
    root: Option<&Path>,
    requested: &str,
) -> Result<Vec<Value>, String> {
    let text = read_workspace_text_file(root, requested, "storyboardsPath").await?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse storyboardsPath JSON: {e}"))?;
    let storyboards = value
        .as_array()
        .or_else(|| value.get("storyboards").and_then(Value::as_array))
        .ok_or_else(|| {
            "storyboardsPath JSON must be an array or an object with a storyboards array"
                .to_string()
        })?;
    Ok(storyboards.clone())
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

pub(crate) async fn get_json_response(
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

pub(crate) struct OriginalScriptManifestInput<'a> {
    pub(crate) project_id: &'a str,
    pub(crate) project_name: &'a str,
    pub(crate) source_document_id: &'a str,
    pub(crate) filename: Option<&'a str>,
    pub(crate) char_count: usize,
    pub(crate) byte_size: usize,
    pub(crate) source_materials_path: &'a str,
    pub(crate) original_path: &'a str,
    pub(crate) output_dir: &'a Path,
    pub(crate) script_path: &'a Path,
    pub(crate) manifest_path: &'a Path,
    pub(crate) original_text_available: bool,
}

pub(crate) fn build_original_script_manifest(input: OriginalScriptManifestInput<'_>) -> Value {
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
                        let body = parse_json_or_raw(&text);
                        let normalized = json!({
                            "success": false,
                            "blocked": body.get("blocked").and_then(Value::as_bool).unwrap_or(false),
                            "status": status.as_u16(),
                            "error": body.get("error").cloned().unwrap_or_else(|| json!(format!("API error {}", status))),
                            "reason": body.get("reason").or_else(|| body.get("code")).cloned(),
                            "message": body.get("message").or_else(|| body.get("userMessage")).cloned().unwrap_or_else(|| json!(text)),
                            "userMessage": body.get("userMessage").or_else(|| body.get("message")).cloned().unwrap_or_else(|| json!(text)),
                            "guidance": body.get("guidance").cloned(),
                            "requiredTools": body.get("requiredTools").cloned(),
                            "requiredReads": body.get("requiredReads").cloned(),
                            "retryTool": body.get("retryTool").cloned(),
                            "retryArgs": body.get("retryArgs").cloned(),
                            "failedItems": body.get("failedItems").cloned(),
                            "qualityWarnings": body.get("qualityWarnings").cloned(),
                            "bindingDiagnostics": body.get("bindingDiagnostics").cloned(),
                            "raw": body,
                        });
                        (false, normalized.to_string())
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
