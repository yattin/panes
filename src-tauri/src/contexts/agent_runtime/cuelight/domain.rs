use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::CueLightBindingDto;

use super::infrastructure::auth::get_global_auth_token;
use super::infrastructure::CUELIGHT_SERVER_URL;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CueLightThreadContext {
    pub project_id: String,
    pub project_name: String,
    pub project_type: Option<String>,
    pub source_mode: Option<String>,
    pub visual_mode: Option<String>,
    pub video_aspect_ratio: Option<String>,
    pub total_episodes: Option<i64>,
    pub duration_per_episode: Option<i64>,
    pub style_prompt_summary: Option<String>,
    pub episode_count: usize,
    pub character_count: usize,
    pub scene_count: usize,
    pub prop_count: usize,
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
        let characters = project["characters"]
            .as_array()
            .or_else(|| project["bible"]["characters"].as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);
        let scenes = project["scenes"]
            .as_array()
            .or_else(|| project["bible"]["scenes"].as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);
        let props = project["props"]
            .as_array()
            .or_else(|| project["bible"]["props"].as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        Ok(Self {
            project_id: binding.project_id.clone(),
            project_name: binding.project_name.clone(),
            project_type: project["projectType"].as_str().map(|s| s.to_string()),
            source_mode: project["sourceMode"].as_str().map(|s| s.to_string()),
            visual_mode: project["visualMode"].as_str().map(|s| s.to_string()),
            video_aspect_ratio: project["videoAspectRatio"].as_str().map(|s| s.to_string()),
            total_episodes: project["totalEpisodes"].as_i64(),
            duration_per_episode: project["durationPerEpisode"].as_i64(),
            style_prompt_summary: project["bible"]["stylePrompt"]
                .as_str()
                .or_else(|| project["stylePrompt"].as_str())
                .map(|s| s.chars().take(160).collect()),
            episode_count: episodes,
            character_count: characters,
            scene_count: scenes,
            prop_count: props,
            storyboard_count: storyboards,
        })
    }
}
