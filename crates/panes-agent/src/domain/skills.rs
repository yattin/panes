#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDefinition {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub prompt: String,
    pub source: SkillSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginManifest {
    pub id: String,
    pub path: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub commands: Vec<String>,
    pub agents: Vec<String>,
    pub skills: Vec<String>,
    pub hooks: Option<serde_json::Value>,
    pub mcp_servers: Vec<serde_json::Value>,
    pub lsp_servers: Vec<serde_json::Value>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSource {
    User,
    Workspace,
    Plugin { plugin_id: String },
}
