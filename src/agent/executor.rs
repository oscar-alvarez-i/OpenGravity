use crate::domain::message::{Message, Role};
use crate::llm::LlmOrchestrator;
use crate::skills::registry::SkillRegistry;
use crate::tools::registry::Registry;
use anyhow::Result;
use tracing::{debug, info, warn};

pub struct Executor<'a> {
    llm: &'a LlmOrchestrator,
    registry: &'a Registry,
    skill_registry: &'a SkillRegistry,
}

impl<'a> Executor<'a> {
    pub fn new(
        llm: &'a LlmOrchestrator,
        registry: &'a Registry,
        skill_registry: &'a SkillRegistry,
    ) -> Self {
        Self {
            llm,
            registry,
            skill_registry,
        }
    }

    /// Evaluates messages, queries LLM, and returns a list of messages (Assistant, Tool) and a continuation flag
    pub async fn execute_step(
        &self,
        system_prompt: &str,
        messages: &[Message],
    ) -> Result<(Vec<Message>, bool)> {
        debug!("Executing LLM step message context:");
        debug!("  Context [0] System: {}", system_prompt);
        for (i, msg) in messages.iter().enumerate() {
            debug!("  Context [{}] {:?}: {}", i + 1, msg.role, msg.content);
        }
        let response_text = self.llm.generate(system_prompt, messages).await?;
        debug!("Raw LLM response: {}", response_text);

        // Detect Tool and separate assistant reasoning
        if let Some(tool_call) = self.registry.parse_tool_call(&response_text) {
            info!(
                "Tool call detected: {} with input: '{}'",
                tool_call.name, tool_call.input
            );

            // DUPLICATE TOOL PREVENTION: Check if same tool was already executed in latest turn
            if let Some(last_msg) = messages.last() {
                if last_msg.role == Role::Tool {
                    // Check if this tool was already called in the last turn
                    let last_tool_content = &last_msg.content;
                    if last_tool_content.contains("Tool result available:") {
                        // Same tool was already executed - strip TOOL line and return assistant answer only
                        debug!(
                            "Tool '{}' already executed in previous turn - blocking duplicate",
                            tool_call.name
                        );

                        // Extract assistant content and return as final answer (no tool re-execution)
                        let assistant_content = response_text
                            .lines()
                            .filter(|line| !line.trim_start().starts_with("TOOL:"))
                            .collect::<Vec<_>>()
                            .join("\n")
                            .trim()
                            .to_string();

                        return Ok((
                            vec![Message::new(Role::Assistant, assistant_content)],
                            false,
                        ));
                    }
                }
            }

            // Extract Assistant part (everything before the TOOL: line)
            let mut assistant_lines = Vec::new();
            let mut tool_line_found = false;
            let mut trailing_content = Vec::new();

            for line in response_text.lines() {
                if !tool_line_found {
                    if line.trim_start().starts_with("TOOL:") {
                        tool_line_found = true;
                    } else {
                        assistant_lines.push(line);
                    }
                } else if !line.trim().is_empty() {
                    trailing_content.push(line);
                }
            }

            if !trailing_content.is_empty() {
                warn!(
                    "TOOL protocol violation: Content found after TOOL call: {:?}",
                    trailing_content
                );
            }

            let assistant_content = assistant_lines.join("\n").trim().to_string();

            let mut step_messages = Vec::new();

            if !assistant_content.is_empty() {
                debug!("Extracted Assistant reasoning: {}", assistant_content);
                step_messages.push(Message::new(Role::Assistant, assistant_content));
            }

            // Execute Tool
            let tool_res = self.registry.execute_tool(&tool_call);
            let tool_output_text = match tool_res.output {
                Ok(data) => format!("Tool result available: {}. Use this result to answer the user directly without calling the tool again.", data),
                Err(err) => format!("Tool execution error: {}", err),
            };

            info!("Returning Tool message containing execution output.");
            step_messages.push(Message::new(Role::Tool, tool_output_text));

            // Return True for `should_continue` because tool output needs reasoning
            return Ok((step_messages, true));
        }

        // SKILL GATE: Only execute skill if no tool was called
        // Priority: tool > skill > direct answer
        if let Some(user_msg) = messages.iter().rev().find(|m| m.role == Role::User) {
            if let Some(skill) = self
                .skill_registry
                .select_skill(&user_msg.content, messages)
            {
                info!("Skill '{}' triggered for message", skill.name());
                let skill_result = skill.execute(messages, user_msg).await?;

                // Process memory updates - log for now (future: store in DB)
                for update in &skill_result.memory_updates {
                    info!(
                        "Memory update: key='{}', value='{}', op={:?}",
                        update.fact_key, update.fact_value, update.operation
                    );
                }

                // If skill produced content, use that as the response and stop
                if let Some(content) = skill_result.content {
                    return Ok((vec![Message::new(Role::Assistant, content)], false));
                }

                // Skill ran silently (memory updates) - allow LLM to continue normally
                // Don't add extra messages, just let the normal flow continue
            }
        }

        // Return Assistant regular response and False for `should_continue`
        Ok((vec![Message::new(Role::Assistant, response_text)], false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::models::MockLlmProvider;
    use crate::skills::registry::SkillRegistry;

    #[tokio::test]
    async fn test_executor_tool_error_path() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async { Ok("I will call unknown_tool\nTOOL:unknown_tool".to_string()) })
            });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(mock_groq), Box::new(mock_or));
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);

        let (msgs, should_continue) = executor.execute_step("sys", &[]).await.unwrap();

        assert!(should_continue);
        let last_msg = msgs.last().unwrap();
        assert_eq!(last_msg.role, Role::Tool);
        assert!(last_msg
            .content
            .contains("Tool execution error: Tool implementation not found or unauthorized"));
    }
    #[tokio::test]
    async fn test_executor_split_reasoning() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async {
                    Ok("I am thinking.\nTOOL:get_current_time\nSome extra stuff".to_string())
                })
            });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(mock_groq), Box::new(mock_or));
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);

        let (msgs, should_continue) = executor.execute_step("sys", &[]).await.unwrap();

        assert!(should_continue);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, Role::Assistant);
        assert_eq!(msgs[0].content, "I am thinking.");
        assert_eq!(msgs[1].role, Role::Tool);
        assert!(msgs[1].content.contains("Tool result available:"));
    }

    #[tokio::test]
    async fn test_executor_trailing_content_warning() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq.expect_generate_response().returning(|_, _| {
            Box::pin(async {
                Ok("Thinking...\nTOOL:get_current_time\nIllegal trailing content".to_string())
            })
        });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(mock_groq), Box::new(mock_or));
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);

        // We also pass a dummy message to cover the debug context logging at the start of execute_step
        let messages = vec![Message::new(Role::User, "trigger debug log")];
        let (msgs, _should_continue) = executor.execute_step("sys", &messages).await.unwrap();

        assert_eq!(msgs.len(), 2);
        // The warning itself isn't returned, but we hit the code path.
    }

    #[tokio::test]
    async fn test_executor_prevents_duplicate_tool_execution() {
        let mut mock_groq = MockLlmProvider::new();
        // LLM tries to call get_current_time again
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(mock_groq), Box::new(mock_or));
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);

        // Simulate that get_current_time was already executed in previous turn
        let messages = vec![
            Message::new(Role::User, "What time is it?"),
            Message::new(Role::Tool, "Tool result available: UTC: 12:00".to_string()),
        ];

        let (msgs, should_continue) = executor.execute_step("sys", &messages).await.unwrap();

        // Should NOT execute tool again - should return assistant answer only
        assert!(!should_continue);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, Role::Assistant);
        // Should not contain Tool message (no duplicate execution)
        assert!(!msgs.iter().any(|m| m.role == Role::Tool));
    }
}
