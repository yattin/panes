//! REPL Module - Interactive Read-Eval-Print Loop
//!
//! Beautiful REPL interface matching the original Claude Code aesthetic

use crate::api::{ApiClient, ChatMessage, ToolDefinition};
use crate::cli::ui;
use crate::mcp::ToolRegistry;
use crate::state::AppState;
use colored::Colorize;
use std::io::{self, BufRead, Write};
use std::sync::Arc;

pub struct Repl {
    state: AppState,
    conversation_history: Vec<ChatMessage>,
    tool_registry: Arc<ToolRegistry>,
}

impl Repl {
    pub fn new(state: AppState) -> Self {
        ui::init_terminal();
        let tool_registry = Arc::new(ToolRegistry::new());

        register_builtin_tools_sync(tool_registry.clone());

        Self {
            state,
            conversation_history: Vec::new(),
            tool_registry,
        }
    }

    pub fn start(&mut self, initial_prompt: Option<String>) -> anyhow::Result<()> {
        ui::print_welcome();

        if let Some(prompt) = initial_prompt {
            self.process_input(&prompt)?;
        }

        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            ui::print_prompt();
            stdout.flush()?;

            let mut input = String::new();
            stdin.lock().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            match input {
                "exit" | "quit" | ".exit" | ":q" => {
                    println!(
                        "\n  {} {}\n",
                        "👋".yellow(),
                        "Goodbye!".truecolor(255, 140, 66).bold()
                    );
                    break;
                }
                "help" | ".help" | ":h" => ui::print_help(),
                "status" | ".status" => self.print_status(),
                "clear" | ".clear" | ":c" => ui::clear_screen(),
                "history" | ".history" => self.print_history(),
                "reset" | ".reset" => self.reset_conversation(),
                "config" | ".config" => self.print_config(),
                _ => self.process_input(input)?,
            }
        }

        Ok(())
    }

    fn process_input(&mut self, input: &str) -> anyhow::Result<()> {
        // Show user message with styling
        ui::print_user_message(input);

        let client = ApiClient::new(self.state.settings.clone());

        if client.get_api_key().is_none() {
            ui::print_error(
                "API key not configured\n\nSet it with:\n  claude-code config set api_key \"your-api-key\"",
            );
            return Ok(());
        }

        self.conversation_history.push(ChatMessage::user(input));

        // 获取工具定义
        let tools = self.get_tool_definitions();

        // 工具调用循环
        loop {
            // Show typing indicator
            ui::print_typing_indicator();

            let messages = self.conversation_history.clone();
            let response = match block_on_mcp(client.chat(
                messages,
                if tools.is_empty() {
                    None
                } else {
                    Some(tools.clone())
                },
            )) {
                Ok(response) => response,
                Err(e) => {
                    ui::print_error(&format!("Request failed: {}", e));
                    return Ok(());
                }
            };

            let Some(choice) = response.choices.first() else {
                break;
            };
            let message = &choice.message;

            if let Some(calls) = message.tool_calls.clone().filter(|calls| !calls.is_empty()) {
                println!();
                for call in &calls {
                    println!(
                        "  {} Executing tool: {}",
                        "🔧".truecolor(255, 200, 100),
                        call.function.name.cyan().bold()
                    );
                }
                println!();

                self.conversation_history.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: message.content.clone(),
                    tool_calls: Some(calls.clone()),
                    tool_call_id: None,
                });

                for call in &calls {
                    let args: serde_json::Value = serde_json::from_str(&call.function.arguments)
                        .unwrap_or(serde_json::json!({}));
                    let result = self.execute_tool(&call.function.name, args);
                    self.conversation_history
                        .push(ChatMessage::tool(&call.id, result));
                }

                continue;
            }

            if let Some(content) = message.content.as_deref() {
                ui::print_claude_message(content);
                self.conversation_history
                    .push(ChatMessage::assistant(content.to_string()));

                if let Some(usage) = response.usage.as_ref() {
                    let total = usage.prompt_tokens + usage.completion_tokens;
                    println!(
                        "  {} {} prompt · {} generated · {} total",
                        "◦".truecolor(100, 100, 100),
                        usage.prompt_tokens.to_string().truecolor(150, 150, 150),
                        usage.completion_tokens.to_string().truecolor(150, 150, 150),
                        total.to_string().truecolor(180, 180, 180)
                    );
                }
            }

            // 退出循环
            break;
        }

        Ok(())
    }

    /// 获取 MCP 工具定义（转换为 API 格式）
    fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        block_on_mcp(async {
            let tools = self.tool_registry.list().await;
            tools
                .into_iter()
                .map(|t| ToolDefinition::new(t.name, t.description, t.input_schema))
                .collect()
        })
    }

    /// 执行工具调用
    fn execute_tool(&self, name: &str, args: serde_json::Value) -> String {
        let name = name.to_string();
        let registry = self.tool_registry.clone();

        block_on_mcp(async {
            match registry.execute(&name, args).await {
                Ok(result) => {
                    // 打印工具结果摘要
                    if let Some(success) = result.get("success").and_then(|s| s.as_bool()) {
                        if success {
                            println!("  {} Tool succeeded", "✓".green());
                        } else {
                            println!("  {} Tool failed", "✗".red());
                        }
                    }
                    result.to_string()
                }
                Err(e) => {
                    println!("  {} Tool error: {}", "✗".red(), e);
                    serde_json::json!({"error": e.to_string()}).to_string()
                }
            }
        })
    }

    fn print_status(&self) {
        let status = ui::StatusInfo {
            model: self.state.settings.model.clone(),
            api_base: self.state.settings.api.base_url.clone(),
            max_tokens: self.state.settings.api.max_tokens.to_string(),
            timeout: self.state.settings.api.timeout,
            streaming: self.state.settings.api.streaming,
            message_count: self.conversation_history.len(),
            api_key_set: self.state.settings.api.get_api_key().is_some(),
        };
        ui::print_status(&status);
    }

    fn print_history(&self) {
        println!();
        if self.conversation_history.is_empty() {
            println!(
                "  {} {}",
                "◦".truecolor(100, 100, 100),
                "No conversation history".bright_black()
            );
        } else {
            println!(
                "  {} {}",
                "◦".truecolor(147, 112, 219),
                format!(
                    "Conversation history ({} messages)",
                    self.conversation_history.len()
                )
                .truecolor(147, 112, 219)
                .bold()
            );
            println!();

            for (i, msg) in self.conversation_history.iter().enumerate() {
                let (_icon, _color) = match msg.role.as_str() {
                    "user" => ("●", "truecolor(255, 140, 66)"),
                    "assistant" => ("●", "truecolor(147, 112, 219)"),
                    _ => ("●", "bright_black"),
                };

                let role_label = match msg.role.as_str() {
                    "user" => "You".truecolor(255, 180, 100),
                    "assistant" => "Claude".truecolor(200, 150, 255),
                    _ => "Unknown".bright_black(),
                };

                let content = msg.content.as_deref().unwrap_or("");
                let preview: String = content.chars().take(50).collect();
                let suffix = if content.len() > 50 { "..." } else { "" };

                println!(
                    "  {}. {}  {}{}",
                    (i + 1).to_string().truecolor(100, 100, 100),
                    role_label,
                    preview.bright_white(),
                    suffix.bright_black()
                );
            }
        }
        println!();
    }

    fn print_config(&self) {
        println!();
        println!(
            "  {} {}",
            "⚙".truecolor(147, 112, 219),
            "Configuration".truecolor(147, 112, 219).bold()
        );
        println!();

        match serde_json::to_string_pretty(&self.state.settings) {
            Ok(json) => {
                for line in json.lines() {
                    println!("  {}", line.bright_white());
                }
            }
            Err(_) => {
                ui::print_error("Failed to serialize configuration");
            }
        }
        println!();
    }

    fn reset_conversation(&mut self) {
        self.conversation_history.clear();
        ui::print_success("Conversation reset");
        println!();
    }
}

fn register_builtin_tools_sync(tool_registry: Arc<ToolRegistry>) {
    block_on_mcp(async {
        tool_registry.register_builtin_tools().await;
    });
}

fn block_on_mcp<F: std::future::Future>(future: F) -> F::Output {
    futures::executor::block_on(future)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repl_creation() {
        let state = AppState::default();
        let repl = Repl::new(state);
        assert!(repl.conversation_history.is_empty());
    }
}
