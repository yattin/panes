//! Panes-owned agent runtime for the claurst-native engine.
//!
//! This crate is a clean-room implementation of the runtime behavior needed by
//! Panes. Behavior is inspired by claurst (GPL-3.0 by kuberwastaken), but this
//! crate does not link to or derive source code from claurst.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

pub use domain::telemetry::TokenUsage;
pub use domain::tools::{ToolCall, ToolResult, ToolSpec};
pub use interfaces::{
    AgentEvent, AgentOutcome, AgentRuntime, AgentRuntimePorts, AgentText, ModelRequest,
    ModelStreamEvent, RunTurnCommand, RuntimeMetrics, SystemContext,
};
