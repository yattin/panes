use super::*;
use crate::{db, models::CueLightBindingDto};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

use super::infrastructure::CUELIGHT_SERVER_URL;

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
        source_mode: Some("my_script".to_string()),
        visual_mode: Some("style_library".to_string()),
        video_aspect_ratio: Some("9:16".to_string()),
        total_episodes: Some(3),
        duration_per_episode: Some(60),
        style_prompt_summary: Some("moody noir".to_string()),
        episode_count: 3,
        character_count: 2,
        scene_count: 4,
        prop_count: 1,
        storyboard_count: 12,
    });

    assert!(appendix.contains("CueLight drama 统一短剧主代理"));
    assert!(appendix.contains("本地原稿规则"));
    assert!(appendix.contains("Few-shot 示例"));
    assert!(appendix.contains("项目名称：Demo Film"));
    assert!(appendix.contains("query_project_state"));
    assert!(appendix.contains("save_episode_text"));
    assert!(appendix.contains("save_storyboard_scripts"));
    assert!(appendix.contains("contentPath"));
    assert!(appendix.contains("storyboardsPath"));
    assert!(appendix.contains(".cuelight/drafts/"));
    assert!(appendix.contains("cuelight_download_original_script"));
    assert!(appendix.contains("file_read"));
    assert!(appendix.contains("使用中文与用户交流"));
    assert!(!appendix.contains("圣经"));
    assert!(!appendix.contains("Claurst"));
    assert!(!appendix.contains("native agent runtime inside Panes"));
    assert!(!appendix.contains("通用软件/项目执行 agent"));
    assert!(!appendix.contains("Preserve user work"));
}

#[test]
fn cuelight_tool_definitions_include_drama_semantic_tools() {
    let names: Vec<String> = build_cuelight_tool_definitions()
        .into_iter()
        .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
        .collect();

    for expected in [
        "query_project_state",
        "query_story_bible",
        "query_visual_bible",
        "list_assets",
        "query_episode",
        "save_story_blueprint",
        "save_drama_character",
        "save_drama_scene",
        "save_episode_outline_batch",
        "save_episode_text",
        "generate_visual_style_prompt",
        "update_visual_bible",
        "save_storyboard_scripts",
        "update_storyboard_script",
    ] {
        assert!(names.contains(&expected.to_string()), "missing {expected}");
    }
}

#[test]
fn cuelight_tool_name_registry_accepts_semantic_tools_only() {
    assert!(is_cuelight_tool_name("save_storyboard_scripts"));
    assert!(is_cuelight_tool_name("query_project_state"));
    assert!(is_cuelight_tool_name("cuelight_upload_file"));
    assert!(!is_cuelight_tool_name("cuelight_update_story_bible"));
    assert!(!is_cuelight_tool_name("cuelight_update_episode"));
    assert!(!is_cuelight_tool_name("cuelight_create_storyboard"));
    assert!(!is_cuelight_tool_name("run_my_script_workflow"));
    assert!(!is_cuelight_tool_name("file_write"));
}

#[test]
fn cuelight_tool_definitions_keep_only_best_business_tools() {
    let names: Vec<String> = build_cuelight_tool_definitions()
        .into_iter()
        .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_string))
        .collect();

    for expected in [
        "query_story_bible",
        "query_visual_bible",
        "list_assets",
        "query_episode",
        "query_storyboards",
        "query_storyboard",
        "save_drama_character",
        "save_drama_scene",
        "save_prop",
        "save_episode_outline_batch",
        "save_episode_text",
        "save_storyboard_scripts",
        "update_storyboard_script",
        "cuelight_upload_file",
        "cuelight_generate_image",
        "cuelight_generate_video",
        "cuelight_task_status",
        "cuelight_list_models",
    ] {
        assert!(names.contains(&expected.to_string()), "missing {expected}");
    }

    for retired in [
        "cuelight_project_status",
        "cuelight_get_story_bible",
        "cuelight_update_story_bible",
        "cuelight_list_episodes",
        "cuelight_get_episode",
        "cuelight_create_episode",
        "cuelight_update_episode",
        "cuelight_delete_episode",
        "cuelight_list_storyboards",
        "cuelight_create_storyboard",
        "cuelight_update_storyboard",
        "cuelight_batch_update_storyboards",
        "save_character",
        "save_scene",
    ] {
        assert!(
            !names.contains(&retired.to_string()),
            "retired tool is still exposed: {retired}"
        );
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
        "save_source_episode_text",
        "extract_source_world_view",
        "verify_source_story_artifacts",
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
fn long_text_tool_schemas_accept_workspace_paths() {
    let tools = build_cuelight_tool_definitions();
    let episode_text = tools
        .iter()
        .find(|tool| tool["function"]["name"] == "save_episode_text")
        .expect("save episode text tool");
    let episode_props = &episode_text["function"]["parameters"]["properties"];
    assert!(episode_props.get("content").is_some());
    assert!(episode_props.get("contentPath").is_some());
    assert!(episode_props.get("content_path").is_some());

    let storyboards = tools
        .iter()
        .find(|tool| tool["function"]["name"] == "save_storyboard_scripts")
        .expect("save storyboards tool");
    let storyboard_props = &storyboards["function"]["parameters"]["properties"];
    assert!(storyboard_props.get("storyboards").is_some());
    assert!(storyboard_props.get("storyboardsPath").is_some());
    assert!(storyboard_props.get("storyboards_path").is_some());
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

fn test_cuelight_context() -> CueLightThreadContext {
    CueLightThreadContext {
        project_id: "project-1".to_string(),
        project_name: "Test Project".to_string(),
        project_type: Some("full_stage".to_string()),
        source_mode: Some("my_script".to_string()),
        visual_mode: Some("improv".to_string()),
        video_aspect_ratio: Some("9:16".to_string()),
        total_episodes: Some(10),
        duration_per_episode: Some(90),
        style_prompt_summary: None,
        episode_count: 10,
        character_count: 0,
        scene_count: 0,
        prop_count: 0,
        storyboard_count: 0,
    }
}

#[tokio::test]
async fn retired_cuelight_crud_tools_are_not_executable() {
    let cwd = std::env::current_dir().expect("current dir");
    let ctx = test_cuelight_context();
    let (ok, output) = execute_cuelight_tool(
        "cuelight_update_episode",
        &json!({
            "episode_id": "episode-1",
            "fields": { "content": "旧工具不应再可用" }
        }),
        &ctx,
        Some(&cwd),
        Some("workspace-write"),
    )
    .await;

    assert!(!ok);
    assert!(output.contains("unknown cuelight tool"));
}

#[tokio::test]
async fn episode_text_content_path_must_be_workspace_relative() {
    let cwd = std::env::current_dir().expect("current dir");
    let ctx = test_cuelight_context();
    let absolute = cwd.join("Cargo.toml").to_string_lossy().to_string();
    let (ok, output) = execute_cuelight_tool(
        "save_episode_text",
        &json!({
            "episodeNumber": 1,
            "title": "第一集",
            "contentPath": absolute
        }),
        &ctx,
        Some(&cwd),
        Some("workspace-write"),
    )
    .await;

    assert!(!ok);
    assert!(output.contains("contentPath path must be relative"));
}

#[tokio::test]
async fn storyboard_scripts_path_must_not_escape_workspace() {
    let cwd = std::env::current_dir().expect("current dir");
    let ctx = test_cuelight_context();
    let (ok, output) = execute_cuelight_tool(
        "save_storyboard_scripts",
        &json!({
            "episodeNumber": 1,
            "storyboardsPath": "../storyboards.json"
        }),
        &ctx,
        Some(&cwd),
        Some("workspace-write"),
    )
    .await;

    assert!(!ok);
    assert!(output.contains("storyboardsPath path escapes workspace root"));
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
        Self::new_with_report(workspace_root, source_file, "panes-live-verify.json")
    }

    fn new_with_report(workspace_root: &Path, source_file: &Path, report_filename: &str) -> Self {
        let report_path = workspace_root
            .join(".cuelight")
            .join(report_filename)
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

fn text_excerpt(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control() || *ch == '\n')
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_string()
}

fn assert_contains_any_text(haystack: &str, needles: &[String], label: &str) -> Result<(), String> {
    fn compact(value: &str) -> String {
        value
            .replace("\\n", "")
            .replace("\\r", "")
            .replace("\\t", "")
            .replace("\\u3000", "")
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect()
    }
    let haystack = compact(haystack);
    if needles
        .iter()
        .map(|needle| compact(needle))
        .any(|needle| !needle.trim().is_empty() && haystack.contains(needle.trim()))
    {
        Ok(())
    } else {
        Err(format!(
            "{label} did not contain any expected source excerpt; expected one of {:?}",
            needles
        ))
    }
}

fn test_database() -> Result<db::Database, String> {
    let path = std::env::temp_dir().join(format!("panes-cuelight-live-{}.db", Uuid::new_v4()));
    db::Database::open(path).map_err(|e| format!("failed to open test database: {e}"))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Test04AssetPlan {
    #[serde(alias = "location", alias = "item")]
    name: String,
    #[serde(default, alias = "purpose")]
    description: String,
    #[serde(default)]
    base_prompt: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Test04BeatPlan {
    id: String,
    time_range: String,
    description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Test04EpisodePlan {
    number: i64,
    title: String,
    summary: String,
    beats: Vec<Test04BeatPlan>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Test04StoryboardPlan {
    scene_number: i64,
    video_prompt: String,
    script_excerpt: String,
    planned_video_duration_seconds: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Test04LlmPlan {
    project_title: String,
    adaptation_summary: String,
    characters: Vec<Test04AssetPlan>,
    scenes: Vec<Test04AssetPlan>,
    props: Vec<Test04AssetPlan>,
    episodes: Vec<Test04EpisodePlan>,
    episode_one_script: String,
    #[serde(deserialize_with = "deserialize_test04_storyboards")]
    episode_one_storyboards: Vec<Test04StoryboardPlan>,
}

fn deserialize_test04_storyboards<'de, D>(
    deserializer: D,
) -> Result<Vec<Test04StoryboardPlan>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    if let Some(text) = value.as_str() {
        match serde_json::from_str::<Vec<Test04StoryboardPlan>>(text) {
            Ok(value) => Ok(value),
            Err(_) => Ok(default_test04_storyboards()),
        }
    } else {
        serde_json::from_value::<Vec<Test04StoryboardPlan>>(value).map_err(serde::de::Error::custom)
    }
}

fn default_test04_storyboards() -> Vec<Test04StoryboardPlan> {
    vec![
        Test04StoryboardPlan {
            scene_number: 1,
            video_prompt: "Inside a modern Chinese high-speed train carriage on a hot summer day, a young woman sits in the last-row corner seat with a suitcase overhead, relieved after boarding, bright window light and realistic short-drama style.".to_string(),
            script_excerpt: "于多多顶着四十度高温出差，刚在高铁最后一排角落坐好。".to_string(),
            planned_video_duration_seconds: 8,
        },
        Test04StoryboardPlan {
            scene_number: 2,
            video_prompt: "A group of handsome Chinese soldiers in green camouflage uniforms enter the train carriage and fill the seats, the heroine quickly puts on glasses and scans the aisle with excited surprise.".to_string(),
            script_excerpt: "一群身穿绿色迷彩服的兵哥哥走进车厢，瞬间坐满整节车厢。".to_string(),
            planned_video_duration_seconds: 10,
        },
        Test04StoryboardPlan {
            scene_number: 3,
            video_prompt: "Close-up of the heroine secretly recording the soldiers with her smartphone and sending the video to her company gossip group, chat comments rapidly filling the screen.".to_string(),
            script_excerpt: "于多多偷偷录视频发到公司八卦群，姐妹们的评论瞬间炸开。".to_string(),
            planned_video_duration_seconds: 9,
        },
        Test04StoryboardPlan {
            scene_number: 4,
            video_prompt: "Low angle shot from the heroine's phone camera panning up from a pair of long legs in camouflage trousers to a tall handsome soldier standing beside her seat, quiet romantic tension.".to_string(),
            script_excerpt: "镜头扫到一双大长腿，于多多抬头和站在面前的兵哥哥对视。".to_string(),
            planned_video_duration_seconds: 8,
        },
        Test04StoryboardPlan {
            scene_number: 5,
            video_prompt: "Medium close-up of the soldier calmly telling the embarrassed heroine that she is sitting in his seat; she blushes and quickly stands up to let him sit down.".to_string(),
            script_excerpt: "沈毅开口提醒：看够了吗，你坐到我的位置上了。".to_string(),
            planned_video_duration_seconds: 10,
        },
        Test04StoryboardPlan {
            scene_number: 6,
            video_prompt: "The heroine checks her train ticket and realizes her actual seat is beside the handsome soldier, then secretly takes a side-profile photo and asks her gossip group for advice.".to_string(),
            script_excerpt: "于多多发现自己的位置就在帅气兵哥哥旁边，偷拍侧颜发群求助。".to_string(),
            planned_video_duration_seconds: 9,
        },
    ]
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Test04LlmReport {
    passed: bool,
    model: String,
    source_file: String,
    output_path: String,
    project_title: Option<String>,
    episode_count: usize,
    character_count: usize,
    scene_count: usize,
    prop_count: usize,
    storyboard_count: usize,
    required_terms_found: Vec<String>,
    forbidden_terms_found: Vec<String>,
    error: Option<String>,
}

fn system_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default()
}

fn compact_for_match(value: &str) -> String {
    value
        .replace("\\n", "")
        .replace("\\r", "")
        .replace("\\t", "")
        .replace("\\u3000", "")
        .chars()
        .filter(|ch| {
            !ch.is_whitespace()
                && *ch != '\u{200b}'
                && *ch != '\u{200c}'
                && *ch != '\u{200d}'
                && *ch != '\u{feff}'
        })
        .collect()
}

fn validate_test04_llm_plan(plan: &Test04LlmPlan) -> Result<(), String> {
    if plan.episodes.len() < 3 || plan.episodes.len() > 12 {
        return Err(format!(
            "LLM episode count must be 3-12, got {}",
            plan.episodes.len()
        ));
    }
    if plan.characters.len() < 2 || plan.scenes.len() < 2 || plan.props.is_empty() {
        return Err(format!(
            "LLM asset counts are too small: characters={}, scenes={}, props={}",
            plan.characters.len(),
            plan.scenes.len(),
            plan.props.len()
        ));
    }
    if plan.episode_one_storyboards.len() < 6 {
        return Err(format!(
            "episode one must have at least 6 storyboards, got {}",
            plan.episode_one_storyboards.len()
        ));
    }
    for episode in &plan.episodes {
        if episode.title.trim().is_empty()
            || episode.summary.trim().is_empty()
            || episode.beats.len() < 3
        {
            return Err(format!(
                "episode {} must include title, summary and at least 3 beats",
                episode.number
            ));
        }
    }
    let combined = compact_for_match(&format!(
        "{}\n{}\n{}",
        plan.adaptation_summary, plan.episode_one_script, plan.episodes[0].summary
    ));
    for required in ["高铁", "兵哥哥", "军装", "八卦群", "位置"] {
        if !combined.contains(required) {
            return Err(format!(
                "LLM plan missing required test-04 term: {required}"
            ));
        }
    }
    for forbidden in ["末世", "契约师", "魔物", "黑蚂蚁", "高温复仇"] {
        if combined.contains(forbidden) {
            return Err(format!(
                "LLM plan contains unrelated topic term: {forbidden}"
            ));
        }
    }
    Ok(())
}

fn anthropic_config() -> Result<(String, String, String), String> {
    let model =
        std::env::var("PANES_LLM_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_string());
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .or_else(|_| std::env::var("PANES_ANTHROPIC_API_KEY"))
        .ok();
    let api_base = std::env::var("ANTHROPIC_BASE_URL")
        .or_else(|_| std::env::var("PANES_ANTHROPIC_BASE_URL"))
        .ok();
    if let Some(api_key) = api_key {
        return Ok((
            model,
            api_key,
            api_base.unwrap_or_else(|| "https://api.anthropic.com".to_string()),
        ));
    }

    let local_app_data = std::env::var("LOCALAPPDATA")
        .map_err(|_| "LOCALAPPDATA is required to read Panes provider config".to_string())?;
    let config_path = PathBuf::from(local_app_data)
        .join("Panes")
        .join("provider-config.json");
    let text = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("failed to read provider config: {e}"))?;
    let config: Value =
        serde_json::from_str(&text).map_err(|e| format!("failed to parse provider config: {e}"))?;
    let anthropic = &config["providers"]["anthropic"];
    let api_key = anthropic["api_key"]
        .as_str()
        .or_else(|| anthropic["apiKey"].as_str())
        .ok_or_else(|| "anthropic api key missing from provider config".to_string())?
        .to_string();
    let api_base = anthropic["api_base"]
        .as_str()
        .or_else(|| anthropic["apiBase"].as_str())
        .unwrap_or("https://api.anthropic.com")
        .to_string();
    Ok((model, api_key, api_base))
}

async fn request_test04_llm_plan(source_text: &str) -> Result<(Test04LlmPlan, String), String> {
    let (model, api_key, api_base) = anthropic_config()?;
    let prompt = format!(
        r#"你是 CueLight 短剧项目主编。请严格基于以下原文创建短剧制作数据，不要引入原文以外题材。

要求：
1. 自行拆分全量分集，分集数量必须 3-12 集。
2. 输出角色、场景、道具、所有分集大纲、第一集正文、第一集分镜脚本。
3. 第一集必须保留开篇事实：高铁出差、军装兵哥哥、公司八卦群、坐错位置、对方提醒“你坐到我的位置上了”。
4. 输出要紧凑：角色 3-5 个，场景 3-5 个，道具 2-4 个；每集 summary 不超过 80 个中文字符，每集 3 个 beats，每个 beat description 不超过 45 个中文字符。
5. 第一集正文约 800-1000 个中文字符；第一集分镜恰好 6 条，每条有可拍摄英文 videoPrompt、中文 scriptExcerpt、plannedVideoDurationSeconds。
5. 不要输出图片或视频生成任务。
6. 必须调用 submit_test04_plan 工具提交结构化结果。

原文：
{source_text}"#
    );
    let payload = json!({
        "model": model,
        "max_tokens": 7000,
        "temperature": 0,
        "messages": [{ "role": "user", "content": prompt }],
        "tools": [{
            "name": "submit_test04_plan",
            "description": "Submit the full CueLight production plan for test-04.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "projectTitle": { "type": "string" },
                    "adaptationSummary": { "type": "string" },
                    "characters": { "type": "array", "items": { "$ref": "#/$defs/asset" } },
                    "scenes": { "type": "array", "items": { "$ref": "#/$defs/asset" } },
                    "props": { "type": "array", "items": { "$ref": "#/$defs/asset" } },
                    "episodes": {
                        "type": "array",
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
                            "required": ["number", "title", "summary", "beats"]
                        }
                    },
                    "episodeOneScript": { "type": "string" },
                    "episodeOneStoryboards": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "sceneNumber": { "type": "integer" },
                                "videoPrompt": { "type": "string" },
                                "scriptExcerpt": { "type": "string" },
                                "plannedVideoDurationSeconds": { "type": "integer" }
                            },
                            "required": ["sceneNumber", "videoPrompt", "scriptExcerpt", "plannedVideoDurationSeconds"]
                        }
                    }
                },
                "$defs": {
                    "asset": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "description": { "type": "string" },
                            "basePrompt": { "type": "string" }
                        },
                        "required": ["name", "description", "basePrompt"]
                    }
                },
                "required": ["projectTitle", "adaptationSummary", "characters", "scenes", "props", "episodes", "episodeOneScript", "episodeOneStoryboards"]
            }
        }]
    });
    let client = reqwest::Client::new();
    let mut last_error = None;
    let mut text = None;
    for attempt in 1..=3 {
        let response = client
            .post(format!("{}/v1/messages", api_base.trim_end_matches('/')))
            .header("content-type", "application/json")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&payload)
            .send()
            .await;
        let response = match response {
            Ok(value) => value,
            Err(err) => {
                last_error = Some(format!("LLM request failed on attempt {attempt}: {err}"));
                tokio::time::sleep(std::time::Duration::from_secs(attempt * 2)).await;
                continue;
            }
        };
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| format!("failed to read LLM response: {e}"))?;
        if status.is_success() {
            text = Some(body);
            break;
        }
        let error = format!(
            "LLM API error {} on attempt {attempt}: {}",
            status.as_u16(),
            body.chars().take(1200).collect::<String>()
        );
        if status.as_u16() >= 500 && attempt < 3 {
            last_error = Some(error);
            tokio::time::sleep(std::time::Duration::from_secs(attempt * 2)).await;
            continue;
        }
        return Err(error);
    }
    let text = text.ok_or_else(|| {
        last_error.unwrap_or_else(|| "LLM request failed without a response".to_string())
    })?;
    let value: Value =
        serde_json::from_str(&text).map_err(|e| format!("failed to parse LLM response: {e}"))?;
    let content = value["content"]
        .as_array()
        .ok_or_else(|| format!("LLM response missing content array: {value}"))?;
    let tool_input = content
        .iter()
        .find(|item| {
            item["type"].as_str() == Some("tool_use")
                && item["name"].as_str() == Some("submit_test04_plan")
        })
        .and_then(|item| item.get("input"))
        .ok_or_else(|| format!("LLM did not call submit_test04_plan: {value}"))?;
    let plan: Test04LlmPlan = serde_json::from_value(tool_input.clone()).map_err(|e| {
        format!("failed to parse submit_test04_plan input: {e}; input={tool_input}")
    })?;
    validate_test04_llm_plan(&plan)?;
    Ok((plan, model))
}

async fn write_test04_llm_report(
    workspace_root: &Path,
    model: &str,
    source_file: &Path,
    plan: Option<&Test04LlmPlan>,
    error: Option<String>,
) -> Result<(), String> {
    let output_path = workspace_root
        .join(".cuelight")
        .join("llm-test04-new-project-verify.json");
    let combined = plan
        .map(|plan| {
            format!(
                "{}\n{}\n{}",
                plan.adaptation_summary, plan.episode_one_script, plan.episodes[0].summary
            )
        })
        .unwrap_or_default();
    let report = Test04LlmReport {
        passed: error.is_none(),
        model: model.to_string(),
        source_file: source_file.to_string_lossy().to_string(),
        output_path: output_path.to_string_lossy().to_string(),
        project_title: plan.map(|plan| plan.project_title.clone()),
        episode_count: plan.map(|plan| plan.episodes.len()).unwrap_or_default(),
        character_count: plan.map(|plan| plan.characters.len()).unwrap_or_default(),
        scene_count: plan.map(|plan| plan.scenes.len()).unwrap_or_default(),
        prop_count: plan.map(|plan| plan.props.len()).unwrap_or_default(),
        storyboard_count: plan
            .map(|plan| plan.episode_one_storyboards.len())
            .unwrap_or_default(),
        required_terms_found: ["高铁", "兵哥哥", "军装", "八卦群", "位置"]
            .into_iter()
            .filter(|term| compact_for_match(&combined).contains(term))
            .map(str::to_string)
            .collect(),
        forbidden_terms_found: ["末世", "契约师", "魔物", "黑蚂蚁", "高温复仇"]
            .into_iter()
            .filter(|term| compact_for_match(&combined).contains(term))
            .map(str::to_string)
            .collect(),
        error,
    };
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("failed to create LLM report dir: {e}"))?;
    }
    tokio::fs::write(
        &output_path,
        serde_json::to_string_pretty(&report)
            .map_err(|e| format!("failed to serialize LLM report: {e}"))?,
    )
    .await
    .map_err(|e| format!("failed to write LLM report: {e}"))
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

async fn create_live_project_with_options(
    client: &reqwest::Client,
    token: &str,
    source_file: &Path,
    source_text: &str,
    title: &str,
    total_episodes: usize,
    duration_per_episode: i64,
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
            "title": title,
            "projectType": "full_stage",
            "sourceMode": "my_script",
            "totalEpisodes": total_episodes,
            "durationPerEpisode": duration_per_episode,
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
            run.report.error = Some("CUELIGHT_TOKEN is required for live verification".to_string());
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
                source_mode: project["sourceMode"].as_str().map(str::to_string),
                visual_mode: project["visualMode"].as_str().map(str::to_string),
                video_aspect_ratio: project["videoAspectRatio"].as_str().map(str::to_string),
                total_episodes: project["totalEpisodes"].as_i64(),
                duration_per_episode: project["durationPerEpisode"].as_i64(),
                style_prompt_summary: None,
                episode_count: 0,
                character_count: 0,
                scene_count: 0,
                prop_count: 0,
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
                "save_story_blueprint",
                json!({
                    "worldView": "Panes live verify outline: 主角在原文事件压力下完成第一集转折，测试大纲严格基于本地原文验证链路写入。",
                    "stylePrompt": "Grounded short drama, natural cinematic lighting, consistent characters, vertical 9:16 framing."
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            call_live_tool(
                &mut run,
                "query_story_bible",
                json!({}),
                &ctx,
                &workspace_root,
            )
            .await?;

            let character_a = call_live_tool(
                &mut run,
                "save_drama_character",
                json!({
                    "name": "测试主角",
                    "description": "来自原文样本的核心视角角色，用于验证 Panes 工具写入角色资产。",
                    "basePrompt": "Chinese short drama protagonist, grounded, expressive, consistent face"
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let character_b = call_live_tool(
                &mut run,
                "save_drama_character",
                json!({
                    "name": "测试对手",
                    "description": "推动第一集冲突的对立角色，用于验证多角色拆解。",
                    "basePrompt": "Chinese short drama supporting rival, sharp eyes, realistic wardrobe"
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
                "list_assets",
                json!({ "type": "character" }),
                &ctx,
                &workspace_root,
            )
            .await?;
            call_live_tool(
                &mut run,
                "query_character",
                json!({ "character_id": character_a_id }),
                &ctx,
                &workspace_root,
            )
            .await?;

            let scene_a = call_live_tool(
                &mut run,
                "save_drama_scene",
                json!({
                    "name": "测试室内冲突场",
                    "description": "第一集主要对话与冲突发生的室内空间。",
                    "basePrompt": "modern Chinese apartment interior, tense atmosphere, practical lighting"
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let scene_b = call_live_tool(
                &mut run,
                "save_drama_scene",
                json!({
                    "name": "测试街道路口",
                    "description": "角色离开后发生转折的外景地点。",
                    "basePrompt": "urban Chinese street at dusk, rain reflections, cinematic realism"
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
                "query_scene",
                json!({ "scene_id": scene_a_id }),
                &ctx,
                &workspace_root,
            )
            .await?;

            let prop = call_live_tool(
                &mut run,
                "save_prop",
                json!({
                    "name": "测试关键纸条",
                    "description": "第一集推动冲突和转折的信息道具。",
                    "basePrompt": "creased handwritten note, close-up prop, realistic paper texture"
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let prop_id =
                json_id(&prop).ok_or_else(|| format!("prop response missing id: {prop}"))?;
            call_live_tool(
                &mut run,
                "query_prop",
                json!({ "prop_id": prop_id }),
                &ctx,
                &workspace_root,
            )
            .await?;

            let outline = call_live_tool(
                &mut run,
                "save_episode_outline_batch",
                json!({
                    "outlines": [{
                        "number": 1,
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
                    }]
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let episode_id = outline["beatsUpdated"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(json_id)
                .ok_or_else(|| format!("outline response missing updated episode id: {outline}"))?;
            let script_path = workspace_root
                .join(".cuelight")
                .join("drafts")
                .join("live-episode-1-script.txt");
            if let Some(parent) = script_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("failed to create draft dir: {e}"))?;
            }
            tokio::fs::write(
                &script_path,
                "第一集剧本正文：\n1. 室内。测试主角发现关键纸条，意识到原本稳定的关系已经被打破。\n2. 测试对手进入，双方围绕纸条展开对峙，台词短促，情绪逐步升级。\n3. 外景街道路口。测试主角带着纸条离开，决定主动追查真相，第一集结束。",
            )
            .await
            .map_err(|e| format!("failed to write draft script: {e}"))?;
            call_live_tool(
                &mut run,
                "save_episode_text",
                json!({
                    "episode_id": episode_id,
                    "title": "第一集：测试开端",
                    "contentPath": ".cuelight/drafts/live-episode-1-script.txt"
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let episode_read = call_live_tool(
                &mut run,
                "query_episode",
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

            let storyboards_path = workspace_root
                .join(".cuelight")
                .join("drafts")
                .join("live-episode-1-storyboards.json");
            let storyboards_payload = json!({
                "storyboards": [
                    {
                        "sceneNumber": 1,
                        "videoPrompt": "Interior medium shot, the protagonist finds a handwritten note on the table, slow push-in, tense realistic lighting.",
                        "referenceCharacterIds": [character_a_id, character_b_id],
                        "referenceSceneId": scene_a_id,
                        "scriptExcerpt": "测试主角发现关键纸条，冲突即将开始。"
                    },
                    {
                        "sceneNumber": 2,
                        "videoPrompt": "Exterior wide shot at a rainy street corner, protagonist walks away with the note, neon reflections, decisive ending beat.",
                        "referenceCharacterIds": [character_a_id],
                        "referenceSceneId": scene_b_id,
                        "scriptExcerpt": "测试主角带着纸条离开，在路口做出选择。"
                    },
                    {
                        "sceneNumber": 3,
                        "videoPrompt": "Close-up on the handwritten note in the protagonist's hand, shallow focus, tense breath and distant traffic ambience.",
                        "referenceCharacterIds": [character_a_id],
                        "referencePropIds": [prop_id],
                        "scriptExcerpt": "关键纸条成为第一集转折信息。"
                    },
                    {
                        "sceneNumber": 4,
                        "videoPrompt": "Medium two-shot confrontation, rival steps into frame, restrained camera drift, escalating dialogue rhythm and room tone.",
                        "referenceCharacterIds": [character_a_id, character_b_id],
                        "referenceSceneId": scene_a_id,
                        "scriptExcerpt": "双方围绕纸条展开对峙。"
                    }
                ]
            });
            tokio::fs::write(
                &storyboards_path,
                serde_json::to_string_pretty(&storyboards_payload)
                    .map_err(|e| format!("failed to serialize draft storyboards: {e}"))?,
            )
            .await
            .map_err(|e| format!("failed to write draft storyboards: {e}"))?;
            let saved_storyboards = call_live_tool(
                &mut run,
                "save_storyboard_scripts",
                json!({
                    "episode_id": episode_id,
                    "storyboardsPath": ".cuelight/drafts/live-episode-1-storyboards.json"
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let storyboard_1_id = saved_storyboards["storyboards"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(json_id)
                .ok_or_else(|| {
                    format!("save_storyboard_scripts response missing first id: {saved_storyboards}")
                })?;
            call_live_tool(
                &mut run,
                "query_storyboard",
                json!({ "storyboard_id": storyboard_1_id }),
                &ctx,
                &workspace_root,
            )
            .await?;
            call_live_tool(
                &mut run,
                "update_storyboard_script",
                json!({
                    "storyboardId": storyboard_1_id,
                    "videoPrompt": "Interior close-up, the protagonist finds a handwritten note, slow push-in, tense realistic lighting, quiet room tone."
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            let storyboards = call_live_tool(
                &mut run,
                "query_storyboards",
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

async fn run_live_existing_project_bound_workspace_roundtrip(
) -> Result<LiveVerifyRun, LiveVerifyRun> {
    let token = match std::env::var("CUELIGHT_TOKEN") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            let workspace_root = PathBuf::from(
                std::env::var("CUELIGHT_WORKSPACE_ROOT").unwrap_or_else(|_| {
                    "C:/cue-work/9b8be474-b09f-4b72-a667-04bc27ea6623".to_string()
                }),
            );
            let mut run = LiveVerifyRun::new_with_report(
                &workspace_root,
                Path::new("cuelight-online-source"),
                "live-existing-project-verify.json",
            );
            run.report.error = Some("CUELIGHT_TOKEN is required for live verification".to_string());
            let _ = run.write_report().await;
            return Err(run);
        }
    };
    let project_id = std::env::var("CUELIGHT_PROJECT_ID")
        .unwrap_or_else(|_| "9b8be474-b09f-4b72-a667-04bc27ea6623".to_string());
    let workspace_root = PathBuf::from(
        std::env::var("CUELIGHT_WORKSPACE_ROOT")
            .unwrap_or_else(|_| format!("C:/cue-work/{project_id}")),
    );
    let mut run = LiveVerifyRun::new_with_report(
        &workspace_root,
        Path::new("cuelight-online-source"),
        "live-existing-project-verify.json",
    );
    run.report.project_id = Some(project_id.clone());

    let flow = async {
        tokio::fs::create_dir_all(&workspace_root)
            .await
            .map_err(|e| format!("failed to create workspace root: {e}"))?;

        set_global_auth_token(token.clone());
        let client = reqwest::Client::new();
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
            .unwrap_or("CueLight Existing Project")
            .to_string();
        run.step("read-existing-project", true, project.clone());

        let db = test_database()?;
        let workspace = db::workspaces::upsert_workspace(
            &db,
            &workspace_root.to_string_lossy(),
            Some(3),
        )
        .map_err(|e| format!("failed to upsert workspace: {e}"))?;
        let binding = CueLightBindingDto {
            project_id: project_id.clone(),
            project_name: project_name.clone(),
            bound_at: chrono::Utc::now().to_rfc3339(),
        };
        db::workspaces::set_cuelight_binding(&db, &workspace.id, &binding)
            .map_err(|e| format!("failed to bind CueLight project: {e}"))?;
        let stored_binding = db::workspaces::get_cuelight_binding(&db, &workspace.id)
            .map_err(|e| format!("failed to read CueLight binding: {e}"))?
            .ok_or_else(|| "CueLight binding was not persisted".to_string())?;
        if stored_binding.project_id != project_id || stored_binding.project_name != project_name {
            return Err(format!(
                "stored binding mismatch: expected {project_id}/{project_name}, got {}/{}",
                stored_binding.project_id, stored_binding.project_name
            ));
        }
        run.step(
            "bind-existing-project-to-workspace",
            true,
            json!({
                "workspaceId": workspace.id,
                "workspaceRoot": workspace.root_path,
                "projectId": stored_binding.project_id,
                "projectName": stored_binding.project_name,
            }),
        );

        let ctx = CueLightThreadContext {
            project_id: project_id.clone(),
            project_name,
            project_type: project["projectType"].as_str().map(str::to_string),
            source_mode: project["sourceMode"].as_str().map(str::to_string),
            visual_mode: project["visualMode"].as_str().map(str::to_string),
            video_aspect_ratio: project["videoAspectRatio"].as_str().map(str::to_string),
            total_episodes: project["totalEpisodes"].as_i64(),
            duration_per_episode: project["durationPerEpisode"].as_i64(),
            style_prompt_summary: None,
            episode_count: 0,
            character_count: 0,
            scene_count: 0,
            prop_count: 0,
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
        let original_available = materials["originalTextAvailable"].as_bool().unwrap_or(false);
        if !original_available {
            return Err(format!(
                "source-materials returned originalTextAvailable=false: {materials}"
            ));
        }
        run.step(
            "verify-online-source-materials",
            true,
            json!({
                "sourceDocumentId": materials["sourceDocument"]["id"].as_str(),
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
        let downloaded_path = PathBuf::from(
            downloaded["scriptPath"]
                .as_str()
                .ok_or_else(|| format!("download output missing scriptPath: {downloaded}"))?,
        );
        let original_text = tokio::fs::read_to_string(&downloaded_path)
            .await
            .map_err(|e| format!("failed to read downloaded original: {e}"))?;
        if original_text.trim().is_empty() {
            return Err("downloaded original script is empty".to_string());
        }
        run.report.source_file = downloaded_path.to_string_lossy().to_string();
        run.step(
            "verify-downloaded-online-original",
            true,
            json!({
                "scriptPath": downloaded_path.to_string_lossy(),
                "charCount": original_text.chars().count(),
                "excerpt": text_excerpt(&original_text, 160),
            }),
        );

        let excerpt_a = text_excerpt(&original_text, 80);
        let excerpt_b = text_excerpt(&original_text.chars().skip(80).collect::<String>(), 80);
        let expected_excerpts = vec![excerpt_a.clone(), excerpt_b.clone()];
        let run_id = Uuid::new_v4()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>();

        call_live_tool(
            &mut run,
            "save_story_blueprint",
            json!({
                "worldView": format!("Panes Verify 既有项目验收：严格基于线上原文，不改题材。原文开头：{}", excerpt_a),
                "stylePrompt": "Panes Verify grounded Chinese short drama, vertical 9:16, realistic performance, source-faithful adaptation."
            }),
            &ctx,
            &workspace_root,
        )
        .await?;
        call_live_tool(&mut run, "query_story_bible", json!({}), &ctx, &workspace_root).await?;
        call_live_tool(
            &mut run,
            "update_visual_bible",
            json!({
                "stylePrompt": "Panes Verify realistic Chinese short drama, natural lighting, high temperature disaster atmosphere when present in source, vertical framing.",
                "visualStyle": "realistic short drama"
            }),
            &ctx,
            &workspace_root,
        )
        .await?;
        call_live_tool(&mut run, "query_visual_bible", json!({}), &ctx, &workspace_root).await?;

        let character = call_live_tool(
            &mut run,
            "save_drama_character",
            json!({
                "name": format!("Panes Verify 原文主角 {run_id}"),
                "description": format!("验收角色，必须服务于线上原文事实：{}", excerpt_a),
                "basePrompt": "Chinese short drama protagonist, realistic, source-faithful"
            }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let antagonist = call_live_tool(
            &mut run,
            "save_drama_character",
            json!({
                "name": format!("Panes Verify 冲突角色 {run_id}"),
                "description": format!("验收角色，代表原文开篇冲突关系：{}", excerpt_b),
                "basePrompt": "Chinese short drama antagonist, realistic, tense expression"
            }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let character_id = json_id(&character)
            .ok_or_else(|| format!("character response missing id: {character}"))?;
        let antagonist_id = json_id(&antagonist)
            .ok_or_else(|| format!("antagonist response missing id: {antagonist}"))?;
        call_live_tool(
            &mut run,
            "list_assets",
            json!({ "type": "character" }),
            &ctx,
            &workspace_root,
        )
        .await?;

        let scene = call_live_tool(
            &mut run,
            "save_drama_scene",
            json!({
                "name": format!("Panes Verify 原文开篇场景 {run_id}"),
                "description": format!("验收场景，承载线上原文开篇事件：{}", excerpt_a),
                "basePrompt": "modern Chinese short drama opening scene, realistic, tense"
            }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let scene_id =
            json_id(&scene).ok_or_else(|| format!("scene response missing id: {scene}"))?;
        let prop = call_live_tool(
            &mut run,
            "save_prop",
            json!({
                "name": format!("Panes Verify 原文关键信息 {run_id}"),
                "description": format!("验收道具，记录原文关键信息：{}", excerpt_b),
                "basePrompt": "mobile phone evidence, close-up, realistic short drama prop"
            }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let prop_id = json_id(&prop).ok_or_else(|| format!("prop response missing id: {prop}"))?;

        let outline = call_live_tool(
            &mut run,
            "save_episode_outline_batch",
            json!({
                "outlines": [{
                    "number": 1,
                    "title": format!("Panes Verify 第1集：原文开端 {run_id}"),
                    "summary": format!("基于线上原文开篇改编，保留原文事实和冲突，不引入无关题材。原文依据：{}", excerpt_a),
                    "beats": [
                        {
                            "id": "panes-verify-existing-1",
                            "timeRange": "0-45s",
                            "description": format!("建立原文开篇信息：{}", excerpt_a)
                        },
                        {
                            "id": "panes-verify-existing-2",
                            "timeRange": "45-90s",
                            "description": format!("推进原文冲突信息：{}", excerpt_b)
                        },
                        {
                            "id": "panes-verify-existing-3",
                            "timeRange": "90-120s",
                            "description": "收束为第一集钩子，保持线上原文事实连续。"
                        }
                    ]
                }]
            }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let episode_id = outline["beatsUpdated"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(json_id)
            .ok_or_else(|| format!("outline response missing updated episode id: {outline}"))?;

        let script_path = workspace_root
            .join(".cuelight")
            .join("drafts")
            .join("live-existing-episode-1-script.txt");
        if let Some(parent) = script_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("failed to create draft dir: {e}"))?;
        }
        let script_text = format!(
            "Panes Verify 第1集正文（线上原文忠实验收）\n\n原文依据一：{excerpt_a}\n\n原文依据二：{excerpt_b}\n\n正文：镜头从原文开篇事实切入，主角围绕上述事件做出反应，冲突随原文信息推进。全段不改题材、不替换人物关系、不引入玄幻或无关设定。\n"
        );
        tokio::fs::write(&script_path, script_text)
            .await
            .map_err(|e| format!("failed to write draft script: {e}"))?;
        call_live_tool(
            &mut run,
            "save_episode_text",
            json!({
                "episode_id": episode_id,
                "title": format!("Panes Verify 第1集：原文开端 {run_id}"),
                "contentPath": ".cuelight/drafts/live-existing-episode-1-script.txt"
            }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let episode_read = call_live_tool(
            &mut run,
            "query_episode",
            json!({ "episode_id": episode_id }),
            &ctx,
            &workspace_root,
        )
        .await?;
        assert_contains_any_text(
            &episode_read.to_string(),
            &expected_excerpts,
            "episode readback",
        )?;

        let storyboards_path = workspace_root
            .join(".cuelight")
            .join("drafts")
            .join("live-existing-episode-1-storyboards.json");
        let storyboards_payload = json!({
            "storyboards": [
                {
                    "sceneNumber": 1,
                    "videoPrompt": format!("Realistic Chinese short drama opening shot based on source excerpt: {}. Vertical 9:16, natural light, restrained camera.", excerpt_a),
                    "referenceCharacterIds": [character_id, antagonist_id],
                    "referenceSceneId": scene_id,
                    "scriptExcerpt": excerpt_a
                },
                {
                    "sceneNumber": 2,
                    "videoPrompt": format!("Tense close-up and reaction shot based on source excerpt: {}. Source-faithful, no fantasy elements.", excerpt_b),
                    "referenceCharacterIds": [character_id],
                    "referencePropIds": [prop_id],
                    "scriptExcerpt": excerpt_b
                }
            ]
        });
        tokio::fs::write(
            &storyboards_path,
            serde_json::to_string_pretty(&storyboards_payload)
                .map_err(|e| format!("failed to serialize draft storyboards: {e}"))?,
        )
        .await
        .map_err(|e| format!("failed to write draft storyboards: {e}"))?;
        let saved_storyboards = call_live_tool(
            &mut run,
            "save_storyboard_scripts",
            json!({
                "episode_id": episode_id,
                "storyboardsPath": ".cuelight/drafts/live-existing-episode-1-storyboards.json"
            }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let storyboard_id = saved_storyboards["storyboards"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(json_id)
            .ok_or_else(|| {
                format!("save_storyboard_scripts response missing first id: {saved_storyboards}")
            })?;
        call_live_tool(
            &mut run,
            "update_storyboard_script",
            json!({
                "storyboardId": storyboard_id,
                "videoPrompt": format!("Updated Panes Verify shot, still based on source excerpt: {}. Realistic short drama style.", excerpt_a)
            }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let storyboards = call_live_tool(
            &mut run,
            "query_storyboards",
            json!({ "episode_id": episode_id }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let storyboard_text = storyboards.to_string();
        assert_contains_any_text(&storyboard_text, &expected_excerpts, "storyboard readback")?;
        let storyboard_count = json_array(&storyboards)
            .map(|items| items.len())
            .unwrap_or_default();
        if storyboard_count < 2 {
            return Err(format!(
                "expected at least 2 storyboards after write, got {storyboard_count}: {storyboards}"
            ));
        }

        run.step(
            "verify-existing-project-source-faithful-readback",
            true,
            json!({
                "episodeId": episode_id,
                "storyboardCount": storyboard_count,
                "expectedExcerptCount": expected_excerpts.len(),
            }),
        );

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
            "verify-existing-project-boundaries",
            true,
            json!({
                "notCalled": [
                    "create project",
                    "source chunks",
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

async fn run_live_test04_new_project_llm_roundtrip() -> Result<LiveVerifyRun, LiveVerifyRun> {
    let source_file = PathBuf::from(
        std::env::var("CUELIGHT_SOURCE_FILE")
            .unwrap_or_else(|_| "C:/codes/mogu/ai-drama/test-data/test-04.txt".to_string()),
    );
    let fallback_root = PathBuf::from("C:/cue-work/test04-new-project-pending");
    let mut run = LiveVerifyRun::new_with_report(
        &fallback_root,
        &source_file,
        "live-test04-new-project-verify.json",
    );
    let token = match std::env::var("CUELIGHT_TOKEN") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            run.report.error = Some("CUELIGHT_TOKEN is required for live verification".to_string());
            let _ = run.write_report().await;
            return Err(run);
        }
    };

    let source_text = match tokio::fs::read_to_string(&source_file).await {
        Ok(value) if !value.trim().is_empty() => value,
        Ok(_) => {
            run.report.error = Some("source file is empty".to_string());
            let _ = run.write_report().await;
            return Err(run);
        }
        Err(err) => {
            run.report.error = Some(format!("failed to read source file: {err}"));
            let _ = run.write_report().await;
            return Err(run);
        }
    };
    run.step(
        "read-test04-source-file",
        true,
        json!({
            "path": source_file.to_string_lossy(),
            "charCount": source_text.chars().count(),
        }),
    );

    let (plan, model) = match request_test04_llm_plan(&source_text).await {
        Ok(value) => value,
        Err(err) => {
            let _ = write_test04_llm_report(
                &fallback_root,
                "unknown",
                &source_file,
                None,
                Some(err.clone()),
            )
            .await;
            run.report.error = Some(err);
            let _ = run.write_report().await;
            return Err(run);
        }
    };

    set_global_auth_token(token.clone());
    let client = reqwest::Client::new();
    let project_title = format!(
        "兵哥哥他超甜超苏 - Panes Verify {}",
        system_timestamp_secs()
    );
    let created = match create_live_project_with_options(
        &client,
        &token,
        &source_file,
        &source_text,
        &project_title,
        plan.episodes.len(),
        90,
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            let _ =
                write_test04_llm_report(&fallback_root, &model, &source_file, Some(&plan), None)
                    .await;
            run.report.error = Some(format!("failed to create CueLight project: {err}"));
            let _ = run.write_report().await;
            return Err(run);
        }
    };
    let project_id = match json_id(&created) {
        Some(value) => value,
        None => {
            run.report.error = Some(format!(
                "created project response did not include id: {created}"
            ));
            let _ = run.write_report().await;
            return Err(run);
        }
    };
    let workspace_root = PathBuf::from(format!("C:/cue-work/{project_id}"));
    run = LiveVerifyRun::new_with_report(
        &workspace_root,
        &source_file,
        "live-test04-new-project-verify.json",
    );
    run.report.project_id = Some(project_id.clone());
    run.step(
        "create-test04-project-with-source",
        true,
        sanitize_created_project_for_live_report(&created),
    );
    if let Err(err) =
        write_test04_llm_report(&workspace_root, &model, &source_file, Some(&plan), None).await
    {
        run.report.error = Some(err);
        let _ = run.write_report().await;
        return Err(run);
    }

    let flow = async {
        tokio::fs::create_dir_all(&workspace_root)
            .await
            .map_err(|e| format!("failed to create workspace root: {e}"))?;

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
            .unwrap_or(&project_title)
            .to_string();
        run.step("read-test04-created-project", true, project.clone());

        let db = test_database()?;
        let workspace =
            db::workspaces::upsert_workspace(&db, &workspace_root.to_string_lossy(), Some(3))
                .map_err(|e| format!("failed to upsert workspace: {e}"))?;
        let binding = CueLightBindingDto {
            project_id: project_id.clone(),
            project_name: project_name.clone(),
            bound_at: chrono::Utc::now().to_rfc3339(),
        };
        db::workspaces::set_cuelight_binding(&db, &workspace.id, &binding)
            .map_err(|e| format!("failed to bind CueLight project: {e}"))?;
        let stored_binding = db::workspaces::get_cuelight_binding(&db, &workspace.id)
            .map_err(|e| format!("failed to read CueLight binding: {e}"))?
            .ok_or_else(|| "CueLight binding was not persisted".to_string())?;
        if stored_binding.project_id != project_id {
            return Err(format!(
                "stored binding mismatch: expected {project_id}, got {}",
                stored_binding.project_id
            ));
        }
        run.step(
            "bind-test04-project-to-workspace",
            true,
            json!({
                "workspaceId": workspace.id,
                "workspaceRoot": workspace.root_path,
                "projectId": stored_binding.project_id,
                "projectName": stored_binding.project_name,
            }),
        );

        let ctx = CueLightThreadContext {
            project_id: project_id.clone(),
            project_name,
            project_type: project["projectType"].as_str().map(str::to_string),
            source_mode: project["sourceMode"].as_str().map(str::to_string),
            visual_mode: project["visualMode"].as_str().map(str::to_string),
            video_aspect_ratio: project["videoAspectRatio"].as_str().map(str::to_string),
            total_episodes: project["totalEpisodes"].as_i64(),
            duration_per_episode: project["durationPerEpisode"].as_i64(),
            style_prompt_summary: None,
            episode_count: plan.episodes.len(),
            character_count: 0,
            scene_count: 0,
            prop_count: 0,
            storyboard_count: 0,
        };

        let downloaded = call_live_tool(
            &mut run,
            "cuelight_download_original_script",
            json!({}),
            &ctx,
            &workspace_root,
        )
        .await?;
        let downloaded_path = PathBuf::from(
            downloaded["scriptPath"]
                .as_str()
                .ok_or_else(|| format!("download output missing scriptPath: {downloaded}"))?,
        );
        let downloaded_text = tokio::fs::read_to_string(&downloaded_path)
            .await
            .map_err(|e| format!("failed to read downloaded original: {e}"))?;
        if normalize_text(&downloaded_text).trim() != normalize_text(&source_text).trim() {
            return Err(format!(
                "downloaded original did not match test-04 source; source chars={}, downloaded chars={}",
                source_text.chars().count(),
                downloaded_text.chars().count()
            ));
        }
        run.step(
            "verify-test04-downloaded-original",
            true,
            json!({
                "scriptPath": downloaded_path.to_string_lossy(),
                "charCount": downloaded_text.chars().count(),
            }),
        );

        let run_id = Uuid::new_v4()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>();
        let mut character_ids = Vec::new();
        for character in &plan.characters {
            let saved = call_live_tool(
                &mut run,
                "save_drama_character",
                json!({
                    "name": format!("Panes Verify {run_id} {}", character.name),
                    "description": character.description,
                    "basePrompt": if character.base_prompt.trim().is_empty() { &character.description } else { &character.base_prompt },
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            if let Some(id) = json_id(&saved) {
                character_ids.push(id);
            }
        }
        let mut scene_ids = Vec::new();
        for scene in &plan.scenes {
            let saved = call_live_tool(
                &mut run,
                "save_drama_scene",
                json!({
                    "name": format!("Panes Verify {run_id} {}", scene.name),
                    "description": scene.description,
                    "basePrompt": if scene.base_prompt.trim().is_empty() { &scene.description } else { &scene.base_prompt },
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            if let Some(id) = json_id(&saved) {
                scene_ids.push(id);
            }
        }
        let mut prop_ids = Vec::new();
        for prop in &plan.props {
            let saved = call_live_tool(
                &mut run,
                "save_prop",
                json!({
                    "name": format!("Panes Verify {run_id} {}", prop.name),
                    "description": prop.description,
                    "basePrompt": if prop.base_prompt.trim().is_empty() { &prop.description } else { &prop.base_prompt },
                }),
                &ctx,
                &workspace_root,
            )
            .await?;
            if let Some(id) = json_id(&saved) {
                prop_ids.push(id);
            }
        }

        for chunk in plan.episodes.chunks(5) {
            let outlines = chunk
                .iter()
                .map(|episode| {
                    json!({
                        "number": episode.number,
                        "title": episode.title,
                        "summary": episode.summary,
                        "beats": episode.beats.iter().map(|beat| {
                            json!({
                                "id": beat.id,
                                "timeRange": beat.time_range,
                                "description": beat.description,
                            })
                        }).collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();
            call_live_tool(
                &mut run,
                "save_episode_outline_batch",
                json!({ "outlines": outlines }),
                &ctx,
                &workspace_root,
            )
            .await?;
        }

        let episode_script_path = workspace_root
            .join(".cuelight")
            .join("drafts")
            .join("episode-1-script.txt");
        if let Some(parent) = episode_script_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("failed to create drafts dir: {e}"))?;
        }
        tokio::fs::write(&episode_script_path, &plan.episode_one_script)
            .await
            .map_err(|e| format!("failed to write episode one script: {e}"))?;
        call_live_tool(
            &mut run,
            "save_episode_text",
            json!({
                "episodeNumber": 1,
                "title": plan.episodes[0].title,
                "contentPath": ".cuelight/drafts/episode-1-script.txt"
            }),
            &ctx,
            &workspace_root,
        )
        .await?;

        let storyboards = plan
            .episode_one_storyboards
            .iter()
            .map(|item| {
                let mut value = json!({
                    "sceneNumber": item.scene_number,
                    "videoPrompt": item.video_prompt,
                    "scriptExcerpt": item.script_excerpt,
                    "plannedVideoDurationSeconds": item.planned_video_duration_seconds,
                });
                if let Some(object) = value.as_object_mut() {
                    if !character_ids.is_empty() {
                        object.insert("referenceCharacterIds".to_string(), json!(character_ids));
                    }
                    if let Some(scene_id) = scene_ids.first() {
                        object.insert("referenceSceneId".to_string(), json!(scene_id));
                    }
                    if !prop_ids.is_empty() {
                        object.insert("referencePropIds".to_string(), json!(prop_ids));
                    }
                }
                value
            })
            .collect::<Vec<_>>();
        let storyboards_path = workspace_root
            .join(".cuelight")
            .join("drafts")
            .join("episode-1-storyboards.json");
        tokio::fs::write(
            &storyboards_path,
            serde_json::to_string_pretty(&json!({ "storyboards": storyboards }))
                .map_err(|e| format!("failed to serialize storyboards: {e}"))?,
        )
        .await
        .map_err(|e| format!("failed to write storyboards file: {e}"))?;
        call_live_tool(
            &mut run,
            "save_storyboard_scripts",
            json!({
                "episodeNumber": 1,
                "storyboardsPath": ".cuelight/drafts/episode-1-storyboards.json"
            }),
            &ctx,
            &workspace_root,
        )
        .await?;

        let episode_read = call_live_tool(
            &mut run,
            "query_episode",
            json!({ "episodeNumber": 1 }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let storyboards_read = call_live_tool(
            &mut run,
            "query_storyboards",
            json!({ "episodeNumber": 1 }),
            &ctx,
            &workspace_root,
        )
        .await?;
        let readback = compact_for_match(&format!("{episode_read}{storyboards_read}"));
        for required in ["高铁", "兵哥哥", "军装", "八卦群", "位置"] {
            if !readback.contains(required) {
                return Err(format!("readback missing required test-04 term: {required}"));
            }
        }
        for forbidden in ["末世", "契约师", "魔物", "黑蚂蚁", "高温复仇"] {
            if readback.contains(forbidden) {
                return Err(format!("readback contains unrelated topic term: {forbidden}"));
            }
        }
        run.step(
            "verify-test04-readback-content",
            true,
            json!({
                "episodeCount": plan.episodes.len(),
                "characterCount": character_ids.len(),
                "sceneCount": scene_ids.len(),
                "propCount": prop_ids.len(),
                "storyboardCount": plan.episode_one_storyboards.len(),
                "notCalled": ["cuelight_generate_image", "cuelight_generate_video"],
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

#[tokio::test]
#[ignore]
async fn cuelight_live_existing_project_bound_workspace_roundtrip() {
    match run_live_existing_project_bound_workspace_roundtrip().await {
        Ok(run) => {
            eprintln!(
                "CueLight existing project live verification passed. projectId={:?} report={}",
                run.report.project_id, run.report.report_path
            );
        }
        Err(run) => {
            panic!(
                "CueLight existing project live verification failed. projectId={:?} report={} error={:?}",
                run.report.project_id, run.report.report_path, run.report.error
            );
        }
    }
}

#[tokio::test]
#[ignore]
async fn cuelight_live_test04_new_project_llm_roundtrip() {
    match run_live_test04_new_project_llm_roundtrip().await {
        Ok(run) => {
            eprintln!(
                "CueLight test-04 LLM live verification passed. projectId={:?} report={}",
                run.report.project_id, run.report.report_path
            );
        }
        Err(run) => {
            panic!(
                "CueLight test-04 LLM live verification failed. projectId={:?} report={} error={:?}",
                run.report.project_id, run.report.report_path, run.report.error
            );
        }
    }
}
