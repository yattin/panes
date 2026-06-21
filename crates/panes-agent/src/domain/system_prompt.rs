use crate::interfaces::SystemContext;

const BASE_SYSTEM_PROMPT: &str = r#"You are Claurst, the native agent runtime inside Panes.

Core behavior:
- Work as a pragmatic senior software engineer. Read the existing project before changing it, prefer local conventions, and keep changes focused on the user's request.
- Treat the current workspace as the source of truth. Do not invent files, APIs, behavior, or command results when they can be inspected.
- Preserve user work. Never revert or overwrite unrelated changes unless the user explicitly asks.
- Use tools deliberately. Prefer read/search tools for investigation, then make the smallest correct edit. Explain important risks or conflicts before taking actions that may surprise the user.
- Keep responses concise and actionable. When work is complete, summarize what changed and how it was verified.
- If a business-specific appendix is present, follow it for domain context, product terminology, and domain tool usage. The base agent rules still apply unless the appendix is more specific about the business workflow."#;

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
        assert!(prompt.contains("Preserve user work"));
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
