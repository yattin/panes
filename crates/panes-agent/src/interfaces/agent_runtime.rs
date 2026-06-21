use crate::{
    application::ports::{EventSink, ModelClient},
    application::run_agent_turn::RunAgentTurn,
    domain::{
        agents::AgentProfile,
        budget::TokenBudget,
        conversation::AgentMessage,
        memory::MemoryFragment,
        provider::ProviderProfile,
        skills::{PluginManifest, SkillDefinition},
        structured_output::StructuredOutputContract,
        telemetry::TokenUsage,
        tools::ToolCall,
    },
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentText(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct SystemContext {
    pub working_directory: Option<String>,
    pub custom_system_prompt: Option<String>,
    pub memory_fragments: Vec<MemoryFragment>,
    pub append_system_prompt: Option<String>,
    pub disable_memory_files: bool,
    pub provider: Option<ProviderProfile>,
    pub token_budget: Option<TokenBudget>,
    pub structured_output: Option<StructuredOutputContract>,
    pub agent_profile: Option<AgentProfile>,
    pub skill_catalog: Vec<SkillDefinition>,
    pub plugin_catalog: Vec<PluginManifest>,
    pub agent_depth: u32,
    pub allow_nested_agents: bool,
}

impl SystemContext {
    pub fn new(working_directory: Option<String>) -> Self {
        Self {
            working_directory,
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
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunTurnCommand {
    pub conversation_id: String,
    pub messages: Vec<AgentMessage>,
    pub system_context: SystemContext,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelRequest {
    pub conversation_id: String,
    pub messages: Vec<AgentMessage>,
    pub system_context: SystemContext,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelStreamEvent {
    TextDelta(String),
    ThinkingDelta(String),
    ToolUse(ToolCall),
    Usage(TokenUsage),
    Error(String),
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeMetrics {
    pub model_turn_count: u32,
    pub tool_call_count: u32,
    pub errored_tool_call_count: u32,
    pub tool_counts: std::collections::BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    TurnStarted {
        conversation_id: String,
    },
    TextDelta {
        content: String,
    },
    ThinkingDelta {
        content: String,
    },
    ActionStarted {
        action_id: String,
        action_type: String,
        input: serde_json::Value,
    },
    ActionCompleted {
        action_id: String,
        output: String,
        is_error: bool,
    },
    ModelTurnStarted {
        turn_index: u32,
    },
    ModelTurnCompleted {
        turn_index: u32,
        used_tool: bool,
        token_usage: Option<TokenUsage>,
    },
    TranscriptEntry {
        entry_type: String,
        data: serde_json::Value,
    },
    Error {
        message: String,
        recoverable: bool,
    },
    TurnCompleted {
        token_usage: Option<TokenUsage>,
        metrics: RuntimeMetrics,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOutcome {
    pub assistant_text: String,
}

pub trait AgentRuntimePorts: Send + Sync {
    type Model: ModelClient;
    type Events: EventSink;
    type Tools: crate::application::ports::ToolExecutor;

    fn model(&self) -> &Self::Model;
    fn events(&self) -> &Self::Events;
    fn tools(&self) -> &Self::Tools;
}

pub struct AgentRuntime<P> {
    ports: P,
}

impl<P> AgentRuntime<P>
where
    P: AgentRuntimePorts,
{
    pub fn new(ports: P) -> Self {
        Self { ports }
    }

    pub async fn run_turn(&self, command: RunTurnCommand) -> anyhow::Result<AgentOutcome> {
        RunAgentTurn::new(self.ports.model(), self.ports.events(), self.ports.tools())
            .execute(command)
            .await
    }
}
