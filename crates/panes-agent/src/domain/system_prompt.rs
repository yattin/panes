use crate::interfaces::SystemContext;

const BASE_SYSTEM_PROMPT: &str = r#"You are Claurst, the native agent runtime inside Panes. You are a general-purpose software and project execution agent, with built-in CueLight AI film production enhancement for story, visual design, assets, storyboards, and media generation workflows.

## Core identity
- Work as a pragmatic senior engineer and project operator. You can investigate code, edit files, run commands, use skills/plugins, call tools, and help users move work forward end to end.
- CueLight is part of your base identity, not a separate persona. For non-film tasks, behave as a general agent. For film, video, or CueLight tasks, actively apply CueLight domain knowledge, Chinese production terminology, and available CueLight tools when present.
- Treat the current workspace as the source of truth. Do not invent files, APIs, behavior, command output, project state, or CueLight data when it can be inspected with tools.

## Capabilities
- Read, search, create, and edit workspace files while matching the existing project style.
- Run shell commands and tests when they are useful for diagnosis or verification.
- Use dedicated tools for structured operations, including skills, plugins, project tools, filesystem tools, search tools, and CueLight tools.
- Use memory files, available skill/plugin catalogs, structured output contracts, and runtime context when they are injected below.

## Working method
- Understand before acting: inspect relevant files, project state, and prior context before making changes or giving firm conclusions.
- Keep changes focused: implement only what the user asked for, prefer existing patterns, and avoid unrelated refactors.
- Verify when appropriate: run targeted tests/checks or otherwise inspect results before reporting completion.
- Communicate clearly: be concise, state important assumptions, surface blockers, and summarize changed behavior plus verification.
- For ambiguous or high-impact choices, make a reasonable conservative choice when safe; ask the user when guessing would risk data loss, broad rewrites, or wrong business outcomes.

## Tool use
- Prefer purpose-built read/search/edit tools over ad hoc shell commands when available.
- For text search, prefer fast structured search such as `rg`; for independent reads or searches, parallelize where the runtime supports it.
- Before editing, read the surrounding code and understand local conventions.
- Never claim a tool result, command output, file content, or CueLight project state unless it came from actual context or a tool call.
- When a business-specific appendix names domain tools or workflows, use it to decide when to call those tools; the base agent rules still apply.

## Safety and workspace protection
- Preserve user work. Never revert, overwrite, or delete unrelated changes unless the user explicitly asks.
- Avoid destructive operations unless they are clearly requested and scoped. Explain risk before actions that are hard to undo.
- Do not expose, create, or commit secrets, credentials, API keys, or private tokens.
- Do not expand the task beyond the user's request. If you notice adjacent cleanup, mention it only when it materially affects the requested work.

## CueLight enhancement
- Use CueLight as an AI film production layer for story design, visual design, character/scene/prop assets, episodes, storyboards, image generation, video generation, and task tracking.
- When a CueLight project appendix is present, treat it as authoritative for the current project, tool availability, and production workflow.
- Use Chinese with the user for CueLight workflows unless the user asks otherwise. Keep generation prompts, technical fields, and API parameters in the language required by the workflow."#;

pub fn build_system_prompt(context: &SystemContext) -> String {
    let base = context
        .custom_system_prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(BASE_SYSTEM_PROMPT);
    let mut parts = Vec::new();

    if let Some(agent_prompt) = context
        .agent_profile
        .as_ref()
        .and_then(|profile| profile.prompt_prefix.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(agent_prompt.to_string());
    }

    parts.push(base.to_string());

    if let Some(working_directory) = context
        .working_directory
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!(
            "Runtime context:\n- Working directory: {working_directory}"
        ));
    }

    if !context.disable_memory_files {
        let memory = context
            .memory_fragments
            .iter()
            .filter(|fragment| !fragment.content.trim().is_empty())
            .map(|fragment| format!("Source: {}\n{}", fragment.source, fragment.content.trim()))
            .collect::<Vec<_>>();
        if !memory.is_empty() {
            parts.push(format!("Memory files:\n{}", memory.join("\n\n")));
        }
    }

    if let Some(contract) = &context.structured_output {
        parts.push(format!(
            "Structured output:\nReturn the final answer as JSON matching schema `{}`.",
            contract.name
        ));
    }

    if !context.skill_catalog.is_empty() {
        let skills = context
            .skill_catalog
            .iter()
            .map(|skill| {
                let description = skill
                    .description
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("No description.");
                format!("- {}: {} ({})", skill.name, description, skill.path)
            })
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!(
            "Available skills:\n{skills}\nUse the `skill` tool to load full instructions for a named skill when it is relevant."
        ));
    }

    if !context.plugin_catalog.is_empty() {
        let plugins = context
            .plugin_catalog
            .iter()
            .map(|plugin| {
                let description = plugin
                    .description
                    .as_deref()
                    .or(plugin.name.as_deref())
                    .unwrap_or("No description.");
                format!("- {}: {} ({})", plugin.id, description, plugin.path)
            })
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!("Available plugins:\n{plugins}"));
    }

    if let Some(append_system_prompt) = context
        .append_system_prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("Business appendix:\n{append_system_prompt}"));
    }

    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_prompt_contains_native_agent_baseline() {
        let prompt = build_system_prompt(&SystemContext {
            working_directory: None,
            custom_system_prompt: None,
            memory_fragments: Vec::new(),
            append_system_prompt: None,
            disable_memory_files: false,
            provider: None,
            token_budget: None,
            structured_output: None,
            agent_profile: None,
            skill_catalog: Vec::new(),
            plugin_catalog: Vec::new(),
            agent_depth: 0,
            allow_nested_agents: false,
        });

        assert!(prompt.contains("Claurst"));
        assert!(prompt.contains("native agent runtime inside Panes"));
        assert!(prompt.contains("general-purpose software and project execution agent"));
        assert!(prompt.contains("CueLight AI film production enhancement"));
        assert!(prompt.contains("Read, search, create, and edit workspace files"));
        assert!(prompt.contains("Use dedicated tools for structured operations"));
        assert!(prompt.contains("Preserve user work"));
        assert!(prompt.contains("Verify when appropriate"));
    }

    #[test]
    fn appends_working_directory_and_business_prompt() {
        let prompt = build_system_prompt(&SystemContext {
            working_directory: Some("C:/codes/panes".to_string()),
            custom_system_prompt: None,
            memory_fragments: Vec::new(),
            append_system_prompt: Some("CueLight business rules only.".to_string()),
            disable_memory_files: false,
            provider: None,
            token_budget: None,
            structured_output: None,
            agent_profile: None,
            skill_catalog: Vec::new(),
            plugin_catalog: Vec::new(),
            agent_depth: 0,
            allow_nested_agents: false,
        });

        assert!(prompt.contains("Working directory: C:/codes/panes"));
        assert!(prompt.contains("Business appendix:"));
        assert!(prompt.ends_with("CueLight business rules only."));
    }

    #[test]
    fn preserves_dynamic_context_sections() {
        let prompt = build_system_prompt(&SystemContext {
            working_directory: Some("C:/codes/panes".to_string()),
            custom_system_prompt: None,
            memory_fragments: vec![crate::domain::memory::MemoryFragment {
                source: "AGENTS.md".to_string(),
                content: "Project rule.".to_string(),
            }],
            append_system_prompt: Some("CueLight appendix.".to_string()),
            disable_memory_files: false,
            provider: None,
            token_budget: None,
            structured_output: Some(
                crate::domain::structured_output::StructuredOutputContract::json_schema(
                    "answer",
                    serde_json::json!({ "type": "object" }),
                ),
            ),
            agent_profile: None,
            skill_catalog: vec![crate::domain::skills::SkillDefinition {
                name: "prototype".to_string(),
                path: "C:/skills/prototype/SKILL.md".to_string(),
                description: Some("Build a prototype".to_string()),
                prompt: "Prototype instructions".to_string(),
                source: crate::domain::skills::SkillSource::User,
            }],
            plugin_catalog: vec![crate::domain::skills::PluginManifest {
                id: "film-tools".to_string(),
                path: "C:/plugins/film-tools".to_string(),
                name: Some("Film Tools".to_string()),
                description: Some("CueLight helpers".to_string()),
                commands: Vec::new(),
                agents: Vec::new(),
                skills: Vec::new(),
                hooks: None,
                mcp_servers: Vec::new(),
                lsp_servers: Vec::new(),
                capabilities: Vec::new(),
            }],
            agent_depth: 0,
            allow_nested_agents: false,
        });

        let working_directory = prompt.find("Runtime context:").unwrap();
        let memory = prompt.find("Memory files:").unwrap();
        let structured_output = prompt.find("Structured output:").unwrap();
        let skills = prompt.find("Available skills:").unwrap();
        let plugins = prompt.find("Available plugins:").unwrap();
        let appendix = prompt.find("Business appendix:").unwrap();

        assert!(working_directory < memory);
        assert!(memory < structured_output);
        assert!(structured_output < skills);
        assert!(skills < plugins);
        assert!(plugins < appendix);
        assert!(prompt.contains("prototype: Build a prototype"));
        assert!(prompt.contains("film-tools: CueLight helpers"));
    }

    #[test]
    fn ignores_blank_context_fragments() {
        let prompt = build_system_prompt(&SystemContext {
            working_directory: Some("   ".to_string()),
            custom_system_prompt: None,
            memory_fragments: Vec::new(),
            append_system_prompt: Some("\n\t".to_string()),
            disable_memory_files: false,
            provider: None,
            token_budget: None,
            structured_output: None,
            agent_profile: None,
            skill_catalog: Vec::new(),
            plugin_catalog: Vec::new(),
            agent_depth: 0,
            allow_nested_agents: false,
        });

        assert!(!prompt.contains("Runtime context:"));
        assert!(!prompt.contains("Business appendix:"));
    }

    #[test]
    fn custom_prompt_replaces_default_but_keeps_memory_and_appendix() {
        let prompt = build_system_prompt(&SystemContext {
            working_directory: None,
            custom_system_prompt: Some("Custom base.".to_string()),
            memory_fragments: vec![crate::domain::memory::MemoryFragment {
                source: "AGENTS.md".to_string(),
                content: "Project rule.".to_string(),
            }],
            append_system_prompt: Some("Business rule.".to_string()),
            disable_memory_files: false,
            provider: None,
            token_budget: None,
            structured_output: None,
            agent_profile: None,
            skill_catalog: Vec::new(),
            plugin_catalog: Vec::new(),
            agent_depth: 0,
            allow_nested_agents: false,
        });

        assert!(prompt.contains("Custom base."));
        assert!(prompt.contains("Project rule."));
        assert!(prompt.contains("Business rule."));
        assert!(!prompt.contains("native agent runtime inside Panes"));
    }
}
