#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageContent {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMessage {
    pub role: Role,
    pub content: Vec<MessageContent>,
}

impl AgentMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![MessageContent::Text(content.into())],
        }
    }

    pub fn assistant_tool_use(
        id: impl Into<String>,
        name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![MessageContent::ToolUse {
                id: id.into(),
                name: name.into(),
                input,
            }],
        }
    }

    pub fn tool_result(
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            role: Role::User,
            content: vec![MessageContent::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: content.into(),
                is_error,
            }],
        }
    }
}
