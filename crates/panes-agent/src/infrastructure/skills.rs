use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::domain::skills::{PluginManifest, SkillDefinition, SkillSource};

const AGENTS_DIR: &str = ".agents";

pub fn discover_skills(cwd: &Path) -> Vec<SkillDefinition> {
    discover_skills_with_home(cwd, home_dir().as_deref())
}

fn discover_skills_with_home(cwd: &Path, home: Option<&Path>) -> Vec<SkillDefinition> {
    let mut skills = skill_roots(cwd, home)
        .into_iter()
        .flat_map(|(root, source)| read_skill_dir(&root, source))
        .collect::<Vec<_>>();
    for plugin in discover_plugins_with_home(cwd, home) {
        for root in &plugin.skills {
            skills.extend(read_skill_dir(
                &PathBuf::from(root),
                SkillSource::Plugin {
                    plugin_id: plugin.id.clone(),
                },
            ));
        }
    }
    skills
}

pub fn discover_plugins(cwd: &Path) -> Vec<PluginManifest> {
    discover_plugins_with_home(cwd, home_dir().as_deref())
}

fn discover_plugins_with_home(cwd: &Path, home: Option<&Path>) -> Vec<PluginManifest> {
    plugin_roots(cwd, home)
        .into_iter()
        .flat_map(|root| read_plugin_dir(&root))
        .collect()
}

fn skill_roots(cwd: &Path, home: Option<&Path>) -> Vec<(PathBuf, SkillSource)> {
    let mut roots = Vec::new();
    if let Some(home) = home {
        roots.push((home.join(AGENTS_DIR).join("skills"), SkillSource::User));
    }
    roots.push((cwd.join(AGENTS_DIR).join("skills"), SkillSource::Workspace));
    roots
}

fn plugin_roots(cwd: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = home {
        roots.push(home.join(AGENTS_DIR).join("plugins"));
    }
    roots.push(cwd.join(AGENTS_DIR).join("plugins"));
    roots
}

fn read_skill_dir(root: &Path, source: SkillSource) -> Vec<SkillDefinition> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let skill_md = if path.is_dir() {
                path.join("SKILL.md")
            } else {
                path.clone()
            };
            if skill_md.extension().and_then(|value| value.to_str()) != Some("md") {
                return None;
            }
            let prompt = fs::read_to_string(&skill_md).ok()?;
            let name = skill_md
                .parent()
                .and_then(|parent| parent.file_name())
                .or_else(|| skill_md.file_stem())
                .and_then(|value| value.to_str())
                .unwrap_or("skill")
                .to_string();
            let description = prompt
                .lines()
                .find_map(|line| line.strip_prefix("description:"))
                .map(str::trim)
                .map(str::to_string);
            Some(SkillDefinition {
                name,
                path: skill_md.to_string_lossy().replace('\\', "/"),
                description,
                prompt,
                source: source.clone(),
            })
        })
        .collect()
}

fn read_plugin_dir(root: &Path) -> Vec<PluginManifest> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            read_plugin_manifest(&path)
        })
        .collect()
}

fn read_plugin_manifest(path: &Path) -> Option<PluginManifest> {
    let value = if path.join("plugin.json").is_file() {
        let manifest = path.join("plugin.json");
        let raw = fs::read_to_string(&manifest).ok()?;
        serde_json::from_str::<serde_json::Value>(&raw).ok()?
    } else if path.join("plugin.toml").is_file() {
        let manifest = path.join("plugin.toml");
        let raw = fs::read_to_string(&manifest).ok()?;
        let value = raw.parse::<toml::Value>().ok()?;
        serde_json::to_value(value).ok()?
    } else {
        return None;
    };
    let id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("name").and_then(serde_json::Value::as_str))
        .or_else(|| path.file_name().and_then(|value| value.to_str()))
        .unwrap_or("plugin")
        .to_string();
    Some(PluginManifest {
        id,
        path: path.to_string_lossy().replace('\\', "/"),
        name: value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        description: value
            .get("description")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        commands: string_paths(path, &value, "commands", "commands"),
        agents: string_paths(path, &value, "agents", "agents"),
        skills: plugin_skill_paths(path, &value),
        hooks: value.get("hooks").cloned(),
        mcp_servers: array_or_object_values(&value, "mcp_servers", "mcpServers"),
        lsp_servers: array_or_object_values(&value, "lsp_servers", "lspServers"),
        capabilities: value
            .get("capabilities")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn plugin_skill_paths(root: &Path, value: &serde_json::Value) -> Vec<String> {
    let mut paths = string_paths(root, value, "skills", "skills");
    let default = root.join("skills");
    if default.is_dir() {
        paths.push(default.to_string_lossy().replace('\\', "/"));
    }
    paths
}

fn string_paths(root: &Path, value: &serde_json::Value, snake: &str, camel: &str) -> Vec<String> {
    value
        .get(snake)
        .or_else(|| value.get(camel))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|path| root.join(path).to_string_lossy().replace('\\', "/"))
                .collect()
        })
        .unwrap_or_default()
}

fn array_or_object_values(
    value: &serde_json::Value,
    snake: &str,
    camel: &str,
) -> Vec<serde_json::Value> {
    match value.get(snake).or_else(|| value.get(camel)) {
        Some(serde_json::Value::Array(items)) => items.clone(),
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .map(|(name, config)| {
                let mut config = config.clone();
                if let serde_json::Value::Object(object) = &mut config {
                    object
                        .entry("name".to_string())
                        .or_insert_with(|| serde_json::json!(name));
                }
                config
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn discovers_workspace_plugins_and_plugin_skills() {
        let root = temp_dir("panes-agent-skills");
        let home = root.join("home");
        let workspace = root.join("workspace");
        let plugin = workspace.join(".agents/plugins/review-tools");
        fs::create_dir_all(plugin.join("skills/review")).expect("plugin skill dir");
        fs::write(
            plugin.join("plugin.toml"),
            r#"
name = "review-tools"
description = "Review helpers"
commands = ["commands/review.md"]
capabilities = ["read_files"]

[[mcp_servers]]
name = "docs"
command = "docs-mcp"
"#,
        )
        .expect("plugin manifest");
        fs::write(
            plugin.join("skills/review/SKILL.md"),
            "description: Review code\nCheck correctness.",
        )
        .expect("skill");
        fs::create_dir_all(workspace.join(".agents/skills/local")).expect("workspace skill dir");
        fs::write(
            workspace.join(".agents/skills/local/SKILL.md"),
            "description: Local workflow\nUse local rules.",
        )
        .expect("workspace skill");
        fs::create_dir_all(home.join(".agents/skills/user")).expect("user skill dir");
        fs::write(
            home.join(".agents/skills/user/SKILL.md"),
            "description: User workflow\nUse user rules.",
        )
        .expect("user skill");

        let plugins = discover_plugins_with_home(&workspace, Some(&home));
        assert!(plugins.iter().any(|plugin| {
            plugin.id == "review-tools"
                && plugin.description.as_deref() == Some("Review helpers")
                && plugin.capabilities == vec!["read_files"]
                && plugin.mcp_servers.len() == 1
        }));

        let skills = discover_skills_with_home(&workspace, Some(&home));
        assert!(skills.iter().any(|skill| {
            skill.name == "user"
                && skill.description.as_deref() == Some("User workflow")
                && matches!(skill.source, SkillSource::User)
        }));
        assert!(skills.iter().any(|skill| {
            skill.name == "local"
                && skill.description.as_deref() == Some("Local workflow")
                && matches!(skill.source, SkillSource::Workspace)
        }));
        assert!(skills.iter().any(|skill| {
            skill.name == "review"
                && skill.description.as_deref() == Some("Review code")
                && matches!(skill.source, SkillSource::Plugin { ref plugin_id } if plugin_id == "review-tools")
        }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ignores_legacy_claurst_skills_and_plugins() {
        let root = temp_dir("panes-agent-legacy-skills");
        let home = root.join("home");
        let workspace = root.join("workspace");
        fs::create_dir_all(home.join(".claurst/skills/legacy-user")).expect("legacy user skill");
        fs::write(
            home.join(".claurst/skills/legacy-user/SKILL.md"),
            "description: legacy user",
        )
        .expect("legacy user skill file");
        fs::create_dir_all(workspace.join(".claurst/plugins/legacy-plugin/skills/legacy"))
            .expect("legacy plugin skill");
        fs::write(
            workspace.join(".claurst/plugins/legacy-plugin/plugin.toml"),
            r#"name = "legacy-plugin""#,
        )
        .expect("legacy plugin manifest");
        fs::write(
            workspace.join(".claurst/plugins/legacy-plugin/skills/legacy/SKILL.md"),
            "description: legacy plugin",
        )
        .expect("legacy plugin skill file");

        let plugins = discover_plugins_with_home(&workspace, Some(&home));
        let skills = discover_skills_with_home(&workspace, Some(&home));

        assert!(plugins.is_empty());
        assert!(skills.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }
}
