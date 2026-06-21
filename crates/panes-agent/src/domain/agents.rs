#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentAccessLevel {
    Full,
    ReadOnly,
    SearchOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProfile {
    pub name: String,
    pub prompt_prefix: Option<String>,
    pub model: Option<String>,
    pub access: AgentAccessLevel,
    pub max_turns: Option<u32>,
}

impl AgentProfile {
    pub fn build() -> Self {
        Self {
            name: "build".to_string(),
            prompt_prefix: Some(
                "You are the build agent. Implement requested changes completely and correctly."
                    .to_string(),
            ),
            model: None,
            access: AgentAccessLevel::Full,
            max_turns: None,
        }
    }

    pub fn plan() -> Self {
        Self {
            name: "plan".to_string(),
            prompt_prefix: Some("You are the plan agent. You can read files and analyze code but cannot write files or execute commands.".to_string()),
            model: None,
            access: AgentAccessLevel::ReadOnly,
            max_turns: Some(20),
        }
    }

    pub fn explore() -> Self {
        Self {
            name: "explore".to_string(),
            prompt_prefix: Some(
                "You are the explore agent. Search and read files to answer questions quickly."
                    .to_string(),
            ),
            model: None,
            access: AgentAccessLevel::SearchOnly,
            max_turns: Some(15),
        }
    }
}
