use futures::StreamExt;

use crate::{
    application::ports::{EventSink, ModelClient, ToolExecutor},
    domain::{
        budget::{BudgetDecision, BudgetUsage},
        conversation::AgentMessage,
        structured_output::StructuredOutputMode,
        system_prompt::build_system_prompt,
        telemetry::TokenUsage,
    },
    interfaces::{
        AgentEvent, AgentOutcome, ModelRequest, ModelStreamEvent, RunTurnCommand, RuntimeMetrics,
    },
};

pub struct RunAgentTurn<'a, M, E, T> {
    model: &'a M,
    events: &'a E,
    tools: &'a T,
}

impl<'a, M, E, T> RunAgentTurn<'a, M, E, T>
where
    M: ModelClient,
    E: EventSink,
    T: ToolExecutor,
{
    pub fn new(model: &'a M, events: &'a E, tools: &'a T) -> Self {
        Self {
            model,
            events,
            tools,
        }
    }

    pub async fn execute(&self, command: RunTurnCommand) -> anyhow::Result<AgentOutcome> {
        if command.cancellation.is_cancelled() {
            anyhow::bail!("agent turn cancelled");
        }

        self.events
            .emit(AgentEvent::TurnStarted {
                conversation_id: command.conversation_id.clone(),
            })
            .await?;

        let conversation_id = command.conversation_id;
        let system_context = command.system_context;
        let cancellation = command.cancellation;
        let mut messages = command.messages;
        let mut assistant_text = String::new();
        let mut turn_usage = TokenUsage::default();
        let mut budget_usage = BudgetUsage::default();
        let mut metrics = RuntimeMetrics::default();

        loop {
            if cancellation.is_cancelled() {
                anyhow::bail!("agent turn cancelled");
            }

            if let Some(budget) = &system_context.token_budget {
                let system_prompt = build_system_prompt(&system_context);
                if let BudgetDecision::Stop(message) =
                    budget.before_model_call(&messages, &system_prompt)
                {
                    self.events
                        .emit(AgentEvent::Error {
                            message: message.clone(),
                            recoverable: false,
                        })
                        .await?;
                    anyhow::bail!(message);
                }
            }

            let request = ModelRequest {
                conversation_id: conversation_id.clone(),
                messages: messages.clone(),
                system_context: system_context.clone(),
            };
            let turn_index = metrics.model_turn_count.saturating_add(1);
            self.events
                .emit(AgentEvent::ModelTurnStarted { turn_index })
                .await?;
            self.events
                .emit(AgentEvent::TranscriptEntry {
                    entry_type: "model_turn_started".to_string(),
                    data: serde_json::json!({
                        "turn_index": turn_index,
                        "message_count": messages.len(),
                    }),
                })
                .await?;
            let mut stream = self.model.stream(request).await?;
            budget_usage.turn_count = budget_usage.turn_count.saturating_add(1);
            metrics.model_turn_count = metrics.model_turn_count.saturating_add(1);
            let mut used_tool = false;
            let mut stream_usage = TokenUsage::default();

            loop {
                let event = tokio::select! {
                    event = stream.next() => event,
                    _ = cancellation.cancelled() => anyhow::bail!("agent turn cancelled"),
                };
                let Some(event) = event else {
                    break;
                };

                match event {
                    ModelStreamEvent::TextDelta(content) => {
                        assistant_text.push_str(&content);
                        self.events
                            .emit(AgentEvent::TranscriptEntry {
                                entry_type: "assistant_text_delta".to_string(),
                                data: serde_json::json!({ "content": content }),
                            })
                            .await?;
                        self.events.emit(AgentEvent::TextDelta { content }).await?;
                    }
                    ModelStreamEvent::ThinkingDelta(content) => {
                        self.events
                            .emit(AgentEvent::TranscriptEntry {
                                entry_type: "thinking_delta".to_string(),
                                data: serde_json::json!({ "content": content }),
                            })
                            .await?;
                        self.events
                            .emit(AgentEvent::ThinkingDelta { content })
                            .await?;
                    }
                    ModelStreamEvent::Usage(usage) => {
                        stream_usage.combine_latest(usage);
                    }
                    ModelStreamEvent::ToolUse(call) => {
                        used_tool = true;
                        metrics.tool_call_count = metrics.tool_call_count.saturating_add(1);
                        *metrics.tool_counts.entry(call.name.clone()).or_default() += 1;
                        self.events
                            .emit(AgentEvent::ActionStarted {
                                action_id: call.id.clone(),
                                action_type: call.name.clone(),
                                input: call.input.clone(),
                            })
                            .await?;
                        self.events
                            .emit(AgentEvent::TranscriptEntry {
                                entry_type: "tool_use".to_string(),
                                data: serde_json::json!({
                                    "id": call.id,
                                    "name": call.name,
                                    "input": call.input,
                                }),
                            })
                            .await?;

                        messages.push(AgentMessage::assistant_tool_use(
                            call.id.clone(),
                            call.name.clone(),
                            call.input.clone(),
                        ));
                        let result = self.tools.execute(call, &cancellation).await?;
                        if cancellation.is_cancelled() {
                            anyhow::bail!("agent turn cancelled");
                        }
                        self.events
                            .emit(AgentEvent::ActionCompleted {
                                action_id: result.tool_use_id.clone(),
                                output: result.content.clone(),
                                is_error: result.is_error,
                            })
                            .await?;
                        if result.is_error {
                            metrics.errored_tool_call_count =
                                metrics.errored_tool_call_count.saturating_add(1);
                        }
                        self.events
                            .emit(AgentEvent::TranscriptEntry {
                                entry_type: "tool_result".to_string(),
                                data: serde_json::json!({
                                    "tool_use_id": result.tool_use_id,
                                    "content": result.content,
                                    "is_error": result.is_error,
                                }),
                            })
                            .await?;
                        messages.push(AgentMessage::tool_result(
                            result.tool_use_id,
                            result.content,
                            result.is_error,
                        ));
                    }
                    ModelStreamEvent::Error(message) => anyhow::bail!(message),
                    ModelStreamEvent::Done => break,
                }
            }
            if !stream_usage.is_empty() {
                turn_usage.add_turn(&stream_usage);
                budget_usage.tokens.add_turn(&stream_usage);
            }
            self.events
                .emit(AgentEvent::ModelTurnCompleted {
                    turn_index,
                    used_tool,
                    token_usage: (!stream_usage.is_empty()).then_some(stream_usage.clone()),
                })
                .await?;
            self.events
                .emit(AgentEvent::TranscriptEntry {
                    entry_type: "model_turn_completed".to_string(),
                    data: serde_json::json!({
                        "turn_index": turn_index,
                        "used_tool": used_tool,
                        "token_usage": (!stream_usage.is_empty()).then_some(serde_json::json!({
                            "input": stream_usage.input,
                            "output": stream_usage.output,
                            "reasoning": stream_usage.reasoning,
                            "cache_read": stream_usage.cache_read,
                            "cache_write": stream_usage.cache_write,
                            "cost_usd": stream_usage.cost_usd,
                        })),
                    }),
                })
                .await?;

            if let Some(budget) = &system_context.token_budget {
                if let BudgetDecision::Stop(message) = budget.after_model_call(&budget_usage) {
                    self.events
                        .emit(AgentEvent::Error {
                            message,
                            recoverable: false,
                        })
                        .await?;
                    break;
                }
            }

            if !used_tool {
                break;
            }
        }

        if let Some(contract) = &system_context.structured_output {
            match contract.mode {
                StructuredOutputMode::JsonSchema => {
                    if serde_json::from_str::<serde_json::Value>(&assistant_text).is_err() {
                        let message =
                            format!("structured output `{}` was not valid JSON", contract.name);
                        self.events
                            .emit(AgentEvent::Error {
                                message: message.clone(),
                                recoverable: true,
                            })
                            .await?;
                        anyhow::bail!(message);
                    }
                }
            }
        }

        self.events
            .emit(AgentEvent::TurnCompleted {
                token_usage: (!turn_usage.is_empty()).then_some(turn_usage),
                metrics,
            })
            .await?;
        Ok(AgentOutcome { assistant_text })
    }
}
