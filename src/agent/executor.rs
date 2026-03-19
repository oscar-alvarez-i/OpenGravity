use crate::domain::message::{Message, Role};
use crate::llm::LlmOrchestrator;
use crate::skills::planner::{Plan, PlanStep, Planner as SkillPlanner};
use crate::skills::r#trait::MemoryUpdate;
use crate::skills::registry::SkillRegistry;
use crate::tools::registry::Registry;
use anyhow::Result;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct StepResult {
    pub messages: Vec<Message>,
    pub should_continue: bool,
    pub memory_updates: Vec<MemoryUpdate>,
}

impl StepResult {
    pub fn new(messages: Vec<Message>, should_continue: bool) -> Self {
        Self {
            messages,
            should_continue,
            memory_updates: Vec::new(),
        }
    }

    pub fn with_memory_updates(mut self, updates: Vec<MemoryUpdate>) -> Self {
        self.memory_updates = updates;
        self
    }
}

pub struct Executor<'a> {
    llm: &'a LlmOrchestrator,
    registry: &'a Registry,
    skill_registry: &'a SkillRegistry,
    planner: SkillPlanner,
    pending_plan: Option<Plan>,
    skill_just_ran: bool,
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
            skill_just_ran: false,
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
        let len = remaining.len();
        if !remaining.is_empty() {
            self.pending_plan = Some(Plan { steps: remaining });
            debug!("Pending plan set with {} remaining steps", len);
        } else {
            self.pending_plan = None;
            debug!("Pending plan cleared (no remaining steps)");
        }
    }

    fn execute_tool_step(&self, tool_name: &str) -> Result<Message> {
        let tool_call = crate::domain::tool::ToolCall {
            name: tool_name.to_string(),
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

        Ok(Message::new(Role::Tool, tool_output_text))
    }

    fn should_skip_duplicate(&self, tool_name: &str, last_msg: Option<&Message>) -> bool {
        if self.registry.freshness_policy(tool_name).is_fresh() {
            return false;
        }

        if let Some(msg) = last_msg {
            if msg.role == Role::Tool && msg.content.contains("Tool result available:") {
                return true;
            }
        }
        false
    }

    /// Evaluates messages with strict execution order:
    /// A. Extract current user message
    /// B. Skill (if factual fragment exists)
    /// C. Pending plan (ONLY if no new factual user input)
    /// D. Planner
    /// E. LLM
    /// F. Tool execution
    pub async fn execute_step(
        &mut self,
        system_prompt: &str,
        messages: &[Message],
    ) -> Result<StepResult> {
        let user_msg = messages.iter().rev().find(|m| m.role == Role::User);
        let msg_content = user_msg.map(|m| m.content.as_str());

        // A. Extract current user message
        if let Some(content) = msg_content {
            // C. PENDING PLAN - FIRST priority (resume interrupted work)
            // Skip skill if it already ran for this turn
            if let Some(plan) = self.pending_plan.take() {
                self.skill_just_ran = true;
                let first_step = plan.first_step().cloned();
                if let Some(ref step) = first_step {
                    info!("Executing pending plan step: {:?}", step);

                    match step {
                        PlanStep::Tool(tool_name) => {
                            let tool_msg = self.execute_tool_step(tool_name)?;
                            let remaining = plan.remaining_steps();
                            if !remaining.is_empty() {
                                self.set_pending_plan(crate::skills::planner::Plan {
                                    steps: remaining,
                                });
                            }
                            return Ok(StepResult::new(vec![tool_msg], true));
                        }
                        PlanStep::Direct(_content) => {
                            let remaining = plan.remaining_steps();
                            let remaining_count = remaining.len();
                            if remaining_count > 0 {
                                self.set_pending_plan(crate::skills::planner::Plan {
                                    steps: remaining,
                                });
                            }
                            debug!(
                                "Pending Direct step consumed, remaining: {}",
                                remaining_count
                            );
                            return Ok(StepResult::new(vec![], true));
                        }
                    }
                }
            }

            // B. SKILL FIRST - if factual fragment exists (only if skill hasn't run this turn)
            if !self.skill_just_ran {
                if let Some((factual, _remaining)) = self.planner.split_message(content) {
                    let skill = self.skill_registry.select_skill(&factual, messages);

                    if let Some(skill) = skill {
                        self.skill_just_ran = true;
                        info!(
                            "Skill '{}' triggered for factual fragment: {}",
                            skill.name(),
                            factual
                        );
                        let factual_msg = Message::new(Role::User, factual);
                        let skill_result = skill.execute(messages, &factual_msg).await?;

                        for update in &skill_result.memory_updates {
                            info!(
                                "Memory update: key='{}', value='{}', op={:?}",
                                update.fact_key, update.fact_value, update.operation
                            );
                        }

                        if let Some(content) = skill_result.content {
                            return Ok(StepResult::new(
                                vec![Message::new(Role::Assistant, content)],
                                false,
                            ));
                        }

                        if !skill_result.memory_updates.is_empty() {
                            if let Some(plan) = self.planner.create_plan(content) {
                                self.set_pending_plan(plan);
                            }
                            return Ok(StepResult::new(vec![], true)
                                .with_memory_updates(skill_result.memory_updates));
                        }
                    }
                }

                // B2. SKILL - full message pattern match
                let skill = self.skill_registry.select_skill(content, messages);
                if let Some(skill) = skill {
                    self.skill_just_ran = true;
                    info!("Skill '{}' triggered for message", skill.name());
                    let skill_result = skill.execute(messages, user_msg.unwrap()).await?;

                    for update in &skill_result.memory_updates {
                        info!(
                            "Memory update: key='{}', value='{}', op={:?}",
                            update.fact_key, update.fact_value, update.operation
                        );
                    }

                    if let Some(content) = skill_result.content {
                        return Ok(StepResult::new(
                            vec![Message::new(Role::Assistant, content)],
                            false,
                        ));
                    }

                    if !skill_result.memory_updates.is_empty() {
                        return Ok(StepResult::new(vec![], false)
                            .with_memory_updates(skill_result.memory_updates));
                    }
                }
            }

            // D. PLANNER - create plan for remaining steps (skip if skill already ran)
            if !self.skill_just_ran {
                if let Some(plan) = self.planner.create_plan(content) {
                    let first_step = plan.first_step().cloned();
                    if let Some(ref step) = first_step {
                        info!("Planner executing first step: {:?}", step);

                        match step {
                            PlanStep::Tool(tool_name) => {
                                let tool_msg = self.execute_tool_step(tool_name)?;
                                self.set_pending_plan(plan);
                                return Ok(StepResult::new(vec![tool_msg], true));
                            }
                            PlanStep::Direct(_content) => {
                                let remaining = plan.remaining_steps();
                                if !remaining.is_empty() {
                                    self.set_pending_plan(crate::skills::planner::Plan {
                                        steps: remaining,
                                    });
                                }
                                return Ok(StepResult::new(vec![], true));
                            }
                        }
                    }
                }
            }
        }

        // E. LLM - only if no skill/planner/pending_plan action
        debug!("Executing LLM step message context:");
        debug!("  Context [0] System: {}", system_prompt);
        for (i, msg) in messages.iter().enumerate() {
            debug!("  Context [{}] {:?}: {}", i + 1, msg.role, msg.content);
        }
        let response_text = self.llm.generate(system_prompt, messages).await?;
        debug!("Raw LLM response: {}", response_text);

        // F. TOOL EXECUTION
        if let Some(tool_call) = self.registry.parse_tool_call(&response_text) {
            info!(
                "Tool call detected: {} with input: '{}'",
                tool_call.name, tool_call.input
            );

            if self.should_skip_duplicate(&tool_call.name, messages.last()) {
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

                return Ok(StepResult::new(
                    vec![Message::new(Role::Assistant, assistant_content)],
                    false,
                ));
            }

            let mut tool_line_found = false;
            let mut trailing_content = Vec::new();

            for line in response_text.lines() {
                if !tool_line_found {
                    if line.trim_start().starts_with("TOOL:") {
                        tool_line_found = true;
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

            let tool_res = self.registry.execute_tool(&tool_call);
            let tool_output_text = match tool_res.output {
                Ok(data) => format!("Tool result available: {}. Use this result to answer the user directly without calling the tool again.", data),
                Err(err) => format!("Tool execution error: {}", err),
            };

            info!("Returning Tool message containing execution output.");
            return Ok(StepResult::new(
                vec![Message::new(Role::Tool, tool_output_text)],
                true,
            ));
        }

        Ok(StepResult::new(
            vec![Message::new(Role::Assistant, response_text)],
            false,
        ))
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

        let result = executor.execute_step("sys", &[]).await.unwrap();
        let msgs = result.messages;
        let should_continue = result.should_continue;

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

        let result = executor.execute_step("sys", &[]).await.unwrap();
        let msgs = result.messages;
        let should_continue = result.should_continue;

        assert!(should_continue);
        assert_eq!(
            msgs.len(),
            1,
            "Reasoning not persisted when TOOL call present"
        );
        assert_eq!(msgs[0].role, Role::Tool);
        assert!(msgs[0].content.contains("Tool result available:"));
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

        let messages = vec![Message::new(Role::User, "trigger debug log")];
        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert_eq!(
            result.messages.len(),
            1,
            "Only Tool result returned, reasoning dropped"
        );
    }

    #[tokio::test]
    async fn test_get_current_time_always_executes_fresh() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(mock_groq), Box::new(mock_or));
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![
            Message::new(Role::User, "What time is it?"),
            Message::new(Role::Tool, "Tool result available: UTC: 12:00".to_string()),
        ];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(
            result.should_continue,
            "get_current_time AlwaysFresh should always execute"
        );
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Tool);
    }

    #[tokio::test]
    async fn test_skill_runs_before_llm() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .returning(|_, _| Box::pin(async { Ok("response".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(mock_groq), Box::new(mock_or));
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "Mi color favorito es azul")];

        let result = executor.execute_step("sys", &messages).await.unwrap();
        let msgs = result.messages;

        assert!(!result.should_continue);
        assert!(!msgs.is_empty());
        assert_eq!(msgs[0].role, Role::Assistant);
        assert!(!executor.has_pending_plan());
    }

    #[tokio::test]
    async fn test_memory_fragment_before_transient_detection() {
        let mock_groq = MockLlmProvider::new();

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(mock_groq), Box::new(mock_or));
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(
            Role::User,
            "Mi color favorito es verde y después decime la hora",
        )];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue);
        assert!(
            result.messages.is_empty(),
            "Memory-only skill returns no messages"
        );
        assert!(
            !result.memory_updates.is_empty(),
            "Memory updates should be present"
        );
        assert!(executor.has_pending_plan());

        let pending = executor.take_pending_plan().unwrap();
        assert_eq!(pending.len(), 1);
        assert!(matches!(pending.first_step(), Some(PlanStep::Direct(_))));
    }

    #[tokio::test]
    async fn test_pending_direct_preserved_without_factual_input() {
        let mock_groq = MockLlmProvider::new();

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(mock_groq), Box::new(mock_or));
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        executor.pending_plan = Some(Plan {
            steps: vec![PlanStep::Direct("decime la hora".to_string())],
        });

        let messages = vec![Message::new(Role::User, "Hola")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue);
        assert!(
            result.messages.is_empty(),
            "Pending Direct should not inject user message"
        );
        assert!(
            !executor.has_pending_plan(),
            "Pending plan should be consumed"
        );
    }

    #[tokio::test]
    async fn test_loop_resolves_in_three_iterations() {
        let db = crate::db::sqlite::Db::new(":memory:").unwrap();
        let memory = crate::agent::memory_bridge::MemoryBridge::new(&db, "user");
        let planner = crate::agent::planner::Planner::new();

        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("La hora actual es las 3pm".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(mock_groq), Box::new(mock_or));
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = crate::agent::r#loop::AgentLoop::new(memory, planner, executor);

        let res = agent_loop
            .run(Message::new(
                Role::User,
                "Mi color favorito es verde y después get_current_time",
            ))
            .await;

        assert!(res.is_ok(), "Loop should complete: {:?}", res);
        assert!(res.unwrap().content.contains("3pm"));
    }
}
