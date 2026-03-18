use crate::domain::message::{Message, Role};
use crate::llm::LlmOrchestrator;
use crate::skills::planner::{Plan, PlanStep, Planner as SkillPlanner};
use crate::skills::registry::SkillRegistry;
use crate::tools::registry::Registry;
use anyhow::Result;
use tracing::{debug, info, warn};

pub struct Executor<'a> {
    llm: &'a LlmOrchestrator,
    registry: &'a Registry,
    skill_registry: &'a SkillRegistry,
    planner: SkillPlanner,
    pending_plan: Option<Plan>,
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
            planner: SkillPlanner::new(),
            pending_plan: None,
        }
    }

    pub fn has_pending_plan(&self) -> bool {
        self.pending_plan.is_some()
    }

    pub fn take_pending_plan(&mut self) -> Option<Plan> {
        self.pending_plan.take()
    }

    fn set_pending_plan(&mut self, plan: Plan) {
        let remaining: Vec<PlanStep> = plan.remaining_steps();
        let remaining_len = remaining.len();
        if !remaining.is_empty() {
            self.pending_plan = Some(Plan { steps: remaining });
            debug!("Pending plan set with {} remaining steps", remaining_len);
        } else {
            self.pending_plan = None;
        }
    }

    fn clear_pending_plan(&mut self) {
        self.pending_plan = None;
        debug!("Pending plan cleared");
    }

    /// Evaluates messages, queries LLM, and returns a list of messages (Assistant, Tool) and a continuation flag
    /// Priority: skill > pending_plan > planner > llm > tool handling
    pub async fn execute_step(
        &mut self,
        system_prompt: &str,
        messages: &[Message],
    ) -> Result<(Vec<Message>, bool)> {
        // 1. CHECK PENDING PLAN FIRST (before LLM)
        // Critical: prevents re-planning on same user message
        if let Some(plan) = self.pending_plan.take() {
            if let Some(first_step) = plan.first_step() {
                info!("Executing pending plan step: {:?}", first_step);

                match first_step {
                    PlanStep::Tool(tool_name) => {
                        let tool_call = crate::domain::tool::ToolCall {
                            name: tool_name.clone(),
                            input: String::new(),
                        };

                        let tool_res = self.registry.execute_tool(&tool_call);
                        let tool_output_text = match tool_res.output {
                            Ok(data) => format!(
                                "Tool result available: {}. Use this result to answer the user directly.",
                                data
                            ),
                            Err(err) => format!("Tool execution error: {}", err),
                        };

                        // Set remaining steps as pending
                        self.set_pending_plan(plan);

                        return Ok((vec![Message::new(Role::Tool, tool_output_text)], true));
                    }
                    PlanStep::Direct(content) => {
                        let normalized = self.planner.normalize_direct_step(content);

                        // Set remaining steps as pending
                        self.set_pending_plan(plan);

                        return Ok((vec![Message::new(Role::Assistant, normalized)], true));
                    }
                }
            }
        }

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
                    let last_tool_content = &last_msg.content;
                    if last_tool_content.contains("Tool result available:") {
                        debug!(
                            "Tool '{}' already executed in previous turn - blocking duplicate",
                            tool_call.name
                        );

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
        // Priority: skill > pending_plan > planner > direct answer
        if let Some(user_msg) = messages.iter().rev().find(|m| m.role == Role::User) {
            if let Some(skill) = self
                .skill_registry
                .select_skill(&user_msg.content, messages)
            {
                info!("Skill '{}' triggered for message", skill.name());
                let skill_result = skill.execute(messages, user_msg).await?;

                for update in &skill_result.memory_updates {
                    info!(
                        "Memory update: key='{}', value='{}', op={:?}",
                        update.fact_key, update.fact_value, update.operation
                    );
                }

                if let Some(content) = skill_result.content {
                    return Ok((vec![Message::new(Role::Assistant, content)], false));
                }
            }
        }

        // PLANNER GATE: Check for multi-step intent only if no pending_plan
        // Priority: skill > pending_plan > planner > llm > direct answer
        if let Some(user_msg) = messages.iter().rev().find(|m| m.role == Role::User) {
            if let Some(plan) = self.planner.create_plan(&user_msg.content) {
                if let Some(first_step) = plan.first_step() {
                    info!("Planner executing first step: {:?}", first_step);

                    match first_step {
                        PlanStep::Tool(tool_name) => {
                            let tool_call = crate::domain::tool::ToolCall {
                                name: tool_name.clone(),
                                input: String::new(),
                            };

                            let tool_res = self.registry.execute_tool(&tool_call);
                            let tool_output_text = match tool_res.output {
                                Ok(data) => format!(
                                    "Tool result available: {}. Use this result to answer the user directly.",
                                    data
                                ),
                                Err(err) => format!("Tool execution error: {}", err),
                            };

                            // Set remaining steps as pending (creates pending_plan)
                            self.set_pending_plan(plan);

                            return Ok((vec![Message::new(Role::Tool, tool_output_text)], true));
                        }
                        PlanStep::Direct(content) => {
                            let normalized = self.planner.normalize_direct_step(content);

                            // Set remaining steps as pending
                            self.set_pending_plan(plan);

                            return Ok((vec![Message::new(Role::Assistant, normalized)], true));
                        }
                    }
                }
            }
        }

        // No pending plan, no tool, no skill, no planner - return LLM response
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
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

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
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

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
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

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
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

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
