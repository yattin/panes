use std::time::Duration;

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

const MODEL_STREAM_MAX_RETRIES: u32 = 5;
const MODEL_STREAM_RETRY_BASE_DELAY_MS: u64 = 100;

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

            let mut attempt = 0;
            let (turn_index, used_tool, stream_usage) = loop {
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
                budget_usage.turn_count = budget_usage.turn_count.saturating_add(1);
                metrics.model_turn_count = metrics.model_turn_count.saturating_add(1);

                let mut stream = match self.model.stream(request).await {
                    Ok(stream) => stream,
                    Err(error) => {
                        if attempt < MODEL_STREAM_MAX_RETRIES {
                            self.retry_model_turn(
                                turn_index,
                                attempt,
                                &error.to_string(),
                                &cancellation,
                            )
                            .await?;
                            attempt += 1;
                            continue;
                        }
                        return Err(error);
                    }
                };
                let mut used_tool = false;
                let mut stream_usage = TokenUsage::default();
                let mut emitted_output = false;
                let mut stream_error = None;

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
                            emitted_output = true;
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
                            emitted_output = true;
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
                            emitted_output = true;
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
                        ModelStreamEvent::Error(message) => {
                            stream_error = Some(anyhow::anyhow!(message));
                            break;
                        }
                        ModelStreamEvent::Done => break,
                    }
                }

                if let Some(error) = stream_error {
                    if !emitted_output && attempt < MODEL_STREAM_MAX_RETRIES {
                        self.retry_model_turn(
                            turn_index,
                            attempt,
                            &error.to_string(),
                            &cancellation,
                        )
                        .await?;
                        attempt += 1;
                        continue;
                    }
                    return Err(error);
                }

                break (turn_index, used_tool, stream_usage);
            };
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

    async fn retry_model_turn(
        &self,
        turn_index: u32,
        attempt: u32,
        error: &str,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<()> {
        let delay = model_stream_retry_delay(attempt);
        self.events
            .emit(AgentEvent::TranscriptEntry {
                entry_type: "model_turn_retry".to_string(),
                data: serde_json::json!({
                    "turn_index": turn_index,
                    "retry": attempt + 1,
                    "max_retries": MODEL_STREAM_MAX_RETRIES,
                    "retry_after_ms": delay.as_millis(),
                    "error": error,
                }),
            })
            .await?;
        self.events
            .emit(AgentEvent::ModelTurnCompleted {
                turn_index,
                used_tool: false,
                token_usage: None,
            })
            .await?;
        tokio::select! {
            _ = tokio::time::sleep(delay) => Ok(()),
            _ = cancellation.cancelled() => anyhow::bail!("agent turn cancelled"),
        }
    }
}

fn model_stream_retry_delay(attempt: u32) -> Duration {
    let multiplier = 1_u64 << attempt.min(5);
    Duration::from_millis(MODEL_STREAM_RETRY_BASE_DELAY_MS.saturating_mul(multiplier))
}
