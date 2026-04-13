use crate::domain::message::{Message, Role};
use crate::llm::LlmOrchestrator;
use crate::skills::planner::{Plan, PlanStep, Planner as SkillPlanner};
use crate::skills::r#trait::MemoryUpdate;
use crate::skills::registry::SkillRegistry;
use crate::tools::registry::{Registry, ToolExecutionRequest};
use anyhow::Result;
use std::time::Instant;
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
    last_tool_executed: Option<String>,
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
            last_tool_executed: None,
        }
    }

    pub fn reset_loop_state(&mut self) {
        self.skill_just_ran = false;
    }

    pub fn has_pending_plan(&self) -> bool {
        self.pending_plan.is_some()
    }

    pub fn take_pending_plan(&mut self) -> Option<Plan> {
        self.pending_plan.take()
    }

    fn set_pending_plan(&mut self, plan: Plan) {
        let remaining: Vec<PlanStep> = plan.remaining_steps();
        if remaining.is_empty() {
            self.pending_plan = None;
            debug!("Pending plan cleared (no remaining steps)");
        } else {
            self.pending_plan = Some(crate::skills::planner::Plan { steps: remaining });
            debug!(
                "Pending plan set with {} remaining steps",
                self.pending_plan.as_ref().unwrap().len()
            );
        }
    }

    fn execute_tool_step(&mut self, tool_name: &str) -> Result<Message> {
        let request = ToolExecutionRequest {
            tool_name: tool_name.to_string(),
            input: String::new(),
        };

        let tool_res = self.registry.execute(request);
        let tool_output_text = match tool_res.success {
            true => format!(
                "Tool result available: {}. Use this result to answer the user directly.",
                tool_res.output
            ),
            false => format!(
                "Tool execution error: {}",
                tool_res.error.unwrap_or_default()
            ),
        };

        self.last_tool_executed = Some(tool_name.to_string());

        Ok(Message::new(Role::Tool, tool_output_text))
    }

    fn should_skip_duplicate(&self, tool_name: &str, last_msg: Option<&Message>) -> bool {
        let freshness_policy = self.registry.freshness_policy(tool_name);
        let is_fresh = freshness_policy.is_fresh();

        debug!(
            tool_name = %tool_name,
            freshness_policy = ?freshness_policy,
            "Checking freshness for duplicate detection"
        );

        if is_fresh {
            debug!(
                "Tool '{}' marked as AlwaysFresh, executing without duplicate check",
                tool_name
            );
            return false;
        }

        if let Some(msg) = last_msg {
            if msg.role == Role::Tool && msg.content.contains("Tool result available:") {
                debug!(
                    "Tool '{}' cacheable, duplicate detected, skipping execution",
                    tool_name
                );
                return true;
            }
        }
        debug!(
            "Tool '{}' cacheable, no duplicate found, executing",
            tool_name
        );
        false
    }

    fn should_block_identical_replay(&self, tool_name: &str) -> bool {
        if let Some(ref last_tool) = self.last_tool_executed {
            if last_tool == tool_name {
                debug!(
                    "Identical tool replay detected: '{}' - blocking to prevent infinite loop",
                    tool_name
                );
                return true;
            }
        }
        false
    }

    fn extract_assistant_text(response: &str) -> String {
        response
            .lines()
            .filter(|line| !line.trim_start().starts_with("TOOL:"))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    }

    fn compress_runtime_context(messages: &[Message]) -> Vec<Message> {
        use crate::domain::message::Role;
        use std::collections::HashMap;

        if messages.is_empty() {
            return messages.to_vec();
        }

        let mut after_tools = Vec::new();
        let mut i = 0;

        while i < messages.len() {
            let current = &messages[i];

            if current.role == Role::Tool {
                let mut last_tool = i;
                let mut j = i + 1;

                while j < messages.len() && messages[j].role == Role::Tool {
                    last_tool = j;
                    j += 1;
                }

                after_tools.push(messages[last_tool].clone());

                i = j;
            } else {
                after_tools.push(current.clone());
                i += 1;
            }
        }

        let mut last_memory_per_key: HashMap<String, usize> = HashMap::new();

        for (idx, msg) in after_tools.iter().enumerate() {
            if msg.role == Role::System
                && (msg.content.starts_with("MEMORY_SET:")
                    || msg.content.starts_with("MEMORY_UPDATE:")
                    || msg.content.starts_with("MEMORY_DELETE:"))
            {
                if let Some(key) = Self::extract_memory_key(&msg.content) {
                    last_memory_per_key.insert(key, idx);
                }
            }
        }

        let mut final_result = Vec::new();

        for (idx, msg) in after_tools.iter().enumerate() {
            if msg.role == Role::System
                && (msg.content.starts_with("MEMORY_SET:")
                    || msg.content.starts_with("MEMORY_UPDATE:")
                    || msg.content.starts_with("MEMORY_DELETE:"))
            {
                if let Some(key) = Self::extract_memory_key(&msg.content) {
                    if last_memory_per_key.get(&key) == Some(&idx) {
                        final_result.push(msg.clone());
                    }
                } else {
                    final_result.push(msg.clone());
                }
            } else {
                final_result.push(msg.clone());
            }
        }

        final_result
    }

    fn extract_memory_key(content: &str) -> Option<String> {
        for prefix in &["MEMORY_UPDATE:", "MEMORY_SET:", "MEMORY_DELETE:"] {
            if let Some(rest) = content.strip_prefix(prefix) {
                if let Some(pos) = rest.find('=') {
                    return Some(rest[..pos].to_string());
                }
                return Some(rest.to_string());
            }
        }
        None
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
        let _step_start = Instant::now();
        info!("Step execution started");

        let user_msg = messages.iter().rev().find(|m| m.role == Role::User);
        let msg_content = user_msg.map(|m| m.content.as_str());

        // A. Extract current user message
        if let Some(content) = msg_content {
            // C. PENDING PLAN - FIRST priority (resume interrupted work)
            // Skip skill if it already ran for this turn
            if let Some(plan) = self.pending_plan.take() {
                let branch_start = Instant::now();
                info!("Branch: pending_plan entered");

                self.skill_just_ran = true;
                let first_step = plan.first_step().cloned();
                if let Some(ref step) = first_step {
                    info!("Executing pending plan step: {:?}", step);

                    match step {
                        PlanStep::Tool(tool_name) => {
                            let tool_msg = self.execute_tool_step(tool_name)?;
                            self.set_pending_plan(plan);
                            info!(
                                "Branch: pending_plan exit, elapsed={:?}",
                                branch_start.elapsed()
                            );
                            return Ok(StepResult::new(vec![tool_msg], true));
                        }
                        PlanStep::Direct(_content) => {
                            let remaining_count = plan.remaining_steps().len();
                            self.set_pending_plan(plan);
                            debug!(
                                "Pending Direct step consumed, remaining: {}",
                                remaining_count
                            );
                            info!(
                                "Branch: pending_plan exit, elapsed={:?}",
                                branch_start.elapsed()
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
                        let branch_start = Instant::now();
                        self.skill_just_ran = true;
                        info!("Branch: skill entered, skill={}", skill.name());
                        let factual_msg = Message::new(Role::User, factual);
                        let skill_result = skill.execute(messages, &factual_msg).await?;

                        for update in &skill_result.memory_updates {
                            info!(
                                "Memory update: key='{}', value='{}', op={:?}",
                                update.fact_key, update.fact_value, update.operation
                            );
                        }

                        let plan_for_pending = if !skill_result.memory_updates.is_empty() {
                            self.planner.create_plan(content)
                        } else {
                            None
                        };

                        let mut step_result = if let Some(c) = skill_result.content {
                            StepResult::new(
                                vec![Message::new(Role::Assistant, c)],
                                plan_for_pending.is_some() || skill_result.should_continue,
                            )
                        } else {
                            StepResult::new(Vec::new(), true)
                        };

                        if !skill_result.memory_updates.is_empty() {
                            step_result.memory_updates = skill_result.memory_updates;
                            if let Some(plan) = plan_for_pending {
                                self.set_pending_plan(plan);
                            }
                        }

                        info!(
                            "Branch: skill exit, skill={}, elapsed={:?}",
                            skill.name(),
                            branch_start.elapsed()
                        );
                        return Ok(step_result);
                    }
                }

                // B2. SKILL - full message pattern match
                if let Some(skill) = self.skill_registry.select_skill(content, messages) {
                    let branch_start = Instant::now();
                    self.skill_just_ran = true;
                    info!("Branch: skill entered, skill={}", skill.name());
                    let skill_result = skill.execute(messages, user_msg.unwrap()).await?;

                    for update in &skill_result.memory_updates {
                        info!(
                            "Memory update: key='{}', value='{}', op={:?}",
                            update.fact_key, update.fact_value, update.operation
                        );
                    }

                    let has_memory_updates = !skill_result.memory_updates.is_empty();

                    if let Some(content) = skill_result.content {
                        let mut step_result =
                            StepResult::new(vec![Message::new(Role::Assistant, content)], false);
                        if has_memory_updates {
                            step_result.memory_updates = skill_result.memory_updates;
                        }
                        info!(
                            "Branch: skill exit, skill={}, elapsed={:?}",
                            skill.name(),
                            branch_start.elapsed()
                        );
                        return Ok(step_result);
                    }

                    if has_memory_updates {
                        info!(
                            "Branch: skill exit, skill={}, elapsed={:?}",
                            skill.name(),
                            branch_start.elapsed()
                        );
                        return Ok(StepResult::new(vec![], true)
                            .with_memory_updates(skill_result.memory_updates));
                    }
                }
            }

            // D. PLANNER - create plan for remaining steps (skip if skill already ran)
            if !self.skill_just_ran {
                if let Some(plan) = self.planner.create_plan(content) {
                    let branch_start = Instant::now();
                    info!("Branch: planner entered");

                    let first_step = plan.first_step().cloned();
                    if let Some(ref step) = first_step {
                        info!("Planner executing first step: {:?}", step);

                        match step {
                            PlanStep::Tool(tool_name) => {
                                if self.should_block_identical_replay(tool_name) {
                                    info!(
                                        "Branch: planner blocked_identical_replay, elapsed={:?}",
                                        branch_start.elapsed()
                                    );
                                    return Ok(StepResult::new(vec![], false));
                                }
                                let tool_msg = self.execute_tool_step(tool_name)?;
                                self.set_pending_plan(plan);
                                info!(
                                    "Branch: planner exit, plan_generated=true, elapsed={:?}",
                                    branch_start.elapsed()
                                );
                                return Ok(StepResult::new(vec![tool_msg], true));
                            }
                            PlanStep::Direct(_content) => {
                                let remaining = plan.remaining_steps();
                                if !remaining.is_empty() {
                                    self.set_pending_plan(crate::skills::planner::Plan {
                                        steps: remaining,
                                    });
                                }
                                info!(
                                    "Branch: planner exit, plan_generated=true, elapsed={:?}",
                                    branch_start.elapsed()
                                );
                                return Ok(StepResult::new(vec![], true));
                            }
                        }
                    }
                }
            }
        }

        // E. LLM - only if no skill/planner/pending_plan action
        let branch_start = Instant::now();
        info!("Branch: llm entered");

        debug!("Executing LLM step message context:");
        debug!("  Context [0] System: {}", system_prompt);
        for (i, msg) in messages.iter().enumerate() {
            debug!("  Context [{}] {:?}: {}", i + 1, msg.role, msg.content);
        }

        let tool_blocked = messages
            .iter()
            .any(|m| m.role == Role::Tool && m.content.contains("Tool result available:"));

        let compressed_messages = Self::compress_runtime_context(messages);

        let response_text = self
            .llm
            .generate(system_prompt, &compressed_messages)
            .await?;
        debug!("Raw LLM response: {}", response_text);

        info!("Branch: llm exit, elapsed={:?}", branch_start.elapsed());

        let tool_call = self.registry.parse_tool_call(&response_text);

        if tool_blocked && tool_call.is_some() {
            let text_without_tool = Self::extract_assistant_text(&response_text);

            return Ok(StepResult::new(
                vec![Message::new(Role::Assistant, text_without_tool)],
                false,
            ));
        }

        // F. TOOL EXECUTION
        let tool_branch_start = Instant::now();
        if let Some(tool_call) = tool_call {
            info!("Branch: tool entered, tool={}", tool_call.name);
            info!(
                "Tool call detected: {} with input: '{}'",
                tool_call.name, tool_call.input
            );

            if self.should_skip_duplicate(&tool_call.name, messages.last()) {
                debug!(
                    "Tool '{}' already executed in previous turn - blocking duplicate",
                    tool_call.name
                );
                let assistant_content = Self::extract_assistant_text(&response_text);
                info!(
                    "Branch: tool exit, tool=blocked, elapsed={:?}",
                    tool_branch_start.elapsed()
                );

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

            if self.should_block_identical_replay(&tool_call.name) {
                let assistant_content = Self::extract_assistant_text(&response_text);
                info!(
                    "Branch: tool exit, tool=blocked_identical_replay, elapsed={:?}",
                    tool_branch_start.elapsed()
                );
                return Ok(StepResult::new(
                    vec![Message::new(Role::Assistant, assistant_content)],
                    false,
                ));
            }

            let request = ToolExecutionRequest {
                tool_name: tool_call.name.clone(),
                input: tool_call.input.clone(),
            };
            let tool_res = self.registry.execute(request);
            let tool_output_text = match tool_res.success {
                true => format!("Tool result available: {}. Use this result to answer the user directly without calling the tool again.", tool_res.output),
                false => format!("Tool execution error: {}", tool_res.error.unwrap_or_default()),
            };

            self.last_tool_executed = Some(tool_call.name);

            info!("Returning Tool message containing execution output.");
            info!(
                "Branch: tool exit, tool=executed, elapsed={:?}",
                tool_branch_start.elapsed()
            );
            return Ok(StepResult::new(
                vec![Message::new(Role::Tool, tool_output_text)],
                tool_res.success,
            ));
        }

        info!("Branch: fallback (assistant response)");
        self.last_tool_executed = None;
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
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let result = executor.execute_step("sys", &[]).await.unwrap();
        let msgs = result.messages;
        let should_continue = result.should_continue;

        assert!(!should_continue);
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
                Box::pin(async { Ok("I am thinking.\nTOOL:get_current_time".to_string()) })
            });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
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
    async fn test_executor_trailing_content_rejected() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq.expect_generate_response().returning(|_, _| {
            Box::pin(async {
                Ok("Thinking...\nTOOL:get_current_time\nIllegal trailing content".to_string())
            })
        });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "trigger debug log")];
        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(
            !result.should_continue,
            "Trailing content after TOOL rejects the tool call"
        );
        assert_eq!(
            result.messages.len(),
            1,
            "Full response treated as Assistant message when TOOL has trailing content"
        );
        assert_eq!(result.messages[0].role, Role::Assistant);
    }

    #[tokio::test]
    async fn test_get_current_time_always_executes_fresh() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![
            Message::new(Role::User, "What time is it?"),
            Message::new(Role::Tool, "Tool result available: UTC: 12:00".to_string()),
        ];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(
            !result.should_continue,
            "Same-turn tool call after fresh tool result should be blocked"
        );
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Assistant);
    }

    #[tokio::test]
    async fn test_same_turn_tool_only_strips_returns_empty() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_weather".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![
            Message::new(Role::User, "weather?"),
            Message::new(Role::Tool, "Tool result available: sunny".to_string()),
        ];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Assistant);
        assert!(result.messages[0].content.is_empty());
    }

    #[tokio::test]
    async fn test_skill_runs_before_llm() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .returning(|_, _| Box::pin(async { Ok("response".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "Mi color favorito es azul")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue, "Memory skill continues to LLM");
        assert!(
            result.messages.is_empty(),
            "No assistant message from memory skill"
        );
        assert!(!result.memory_updates.is_empty(), "Memory should be saved");
        assert!(!executor.has_pending_plan());
    }

    #[tokio::test]
    async fn test_memory_fragment_before_transient_detection() {
        let mock_groq = MockLlmProvider::new();

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
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
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
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
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
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

    #[tokio::test]
    async fn test_executor_duplicate_tool_blocked() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_weather".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![
            Message::new(Role::User, "What's the weather?"),
            Message::new(Role::Tool, "Tool result available: sunny".to_string()),
        ];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Assistant);
    }

    #[tokio::test]
    async fn test_executor_pending_plan_cleared_when_empty() {
        let mock_groq = MockLlmProvider::new();

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        executor.pending_plan = Some(Plan {
            steps: vec![PlanStep::Direct("final step".to_string())],
        });

        let messages = vec![Message::new(Role::User, "execute")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue);
        assert!(result.messages.is_empty());
        assert!(!executor.has_pending_plan());
    }

    #[tokio::test]
    async fn test_executor_skill_miss_falls_through_to_llm() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("LLM response".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "hello world")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Assistant);
        assert_eq!(result.messages[0].content, "LLM response");
    }

    #[tokio::test]
    async fn test_executor_b1_factual_miss_continues() {
        let mock_groq = MockLlmProvider::new();
        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(
            Role::User,
            "mi color favorito es azul y después get_unknown_tool",
        )];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue);
        assert!(result.messages.is_empty());
        assert!(!executor.has_pending_plan());
    }

    #[tokio::test]
    async fn test_executor_pending_plan_tool_execution() {
        let mock_groq = MockLlmProvider::new();

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        executor.pending_plan = Some(Plan {
            steps: vec![
                PlanStep::Tool("get_current_time".to_string()),
                PlanStep::Direct("continue".to_string()),
            ],
        });

        let messages = vec![Message::new(Role::User, "skip")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Tool);
        assert!(executor.has_pending_plan());
    }

    #[tokio::test]
    async fn test_executor_skip_duplicate_non_fresh_tool() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_weather".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![
            Message::new(Role::User, "weather?"),
            Message::new(Role::Tool, "Tool result available: sunny".to_string()),
        ];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Assistant);
    }

    #[tokio::test]
    async fn test_executor_planner_direct_with_remaining() {
        // pending_plan Direct consumes the step and sets remaining plan
        // no LLM call needed since pending_plan is taken
        let mock_groq = MockLlmProvider::new();

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        executor.pending_plan = Some(Plan {
            steps: vec![
                PlanStep::Direct("first".to_string()),
                PlanStep::Direct("second".to_string()),
            ],
        });

        let messages = vec![Message::new(Role::User, "go")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue);
        assert!(result.messages.is_empty());
        assert!(executor.has_pending_plan());
    }

    #[tokio::test]
    async fn test_executor_direct_step_from_factual_continues() {
        let mock_groq = MockLlmProvider::new();
        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(
            Role::User,
            "mi color favorito es azul y después contame un chiste",
        )];

        executor.execute_step("sys", &messages).await.unwrap();

        assert!(
            executor.has_pending_plan(),
            "pending_plan should be created with Direct step"
        );

        let next_messages = vec![Message::new(Role::User, "go")];
        let result2 = executor.execute_step("sys", &next_messages).await.unwrap();

        assert!(result2.should_continue);
        assert!(result2.messages.is_empty());
        assert!(
            !executor.has_pending_plan(),
            "Direct step should be consumed"
        );
    }

    #[tokio::test]
    async fn test_executor_tool_result_error_path() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:nonexistent_tool".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "test")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Tool);
        assert!(result.messages[0].content.contains("Tool execution error"));
    }

    #[tokio::test]
    async fn test_executor_b1_preserves_skill_content() {
        #[derive(Debug)]
        struct ContentSkill;
        #[async_trait::async_trait]
        impl crate::skills::r#trait::Skill for ContentSkill {
            fn name(&self) -> &str {
                "content_skill"
            }
            fn description(&self) -> &str {
                "test"
            }
            fn side_effects(&self) -> crate::skills::r#trait::SideEffects {
                crate::skills::r#trait::SideEffects::none()
            }
            fn trigger(&self) -> crate::skills::r#trait::TriggerType {
                crate::skills::r#trait::TriggerType::OnPattern("B1TEST")
            }
            async fn execute(
                &self,
                _context: &[Message],
                _user_message: &Message,
            ) -> anyhow::Result<crate::skills::r#trait::SkillOutput> {
                Ok(crate::skills::r#trait::SkillOutput::continue_with(
                    "B1 content preserved",
                ))
            }
        }

        let mut skill_registry = SkillRegistry::new();
        skill_registry.register(Box::new(ContentSkill));

        let mock_groq = MockLlmProvider::new();
        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "B1TEST: algo y después algo más")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(
            result.should_continue,
            "B1 skill with content but no pending plan: should_continue follows skill.should_continue"
        );
        assert_eq!(
            result.messages.len(),
            1,
            "B1 should return exactly one Assistant message with skill content"
        );
        assert_eq!(result.messages[0].role, Role::Assistant);
        assert_eq!(
            result.messages[0].content, "B1 content preserved",
            "B1 skill content must not be discarded"
        );
    }

    #[tokio::test]
    async fn test_executor_b1_content_with_pending_plan_continues() {
        #[derive(Debug)]
        struct ContentWithPlanSkill;
        #[async_trait::async_trait]
        impl crate::skills::r#trait::Skill for ContentWithPlanSkill {
            fn name(&self) -> &str {
                "content_with_plan_skill"
            }
            fn description(&self) -> &str {
                "test"
            }
            fn side_effects(&self) -> crate::skills::r#trait::SideEffects {
                crate::skills::r#trait::SideEffects::reads_writes()
            }
            fn trigger(&self) -> crate::skills::r#trait::TriggerType {
                crate::skills::r#trait::TriggerType::OnPattern("mi color")
            }
            async fn execute(
                &self,
                _context: &[Message],
                _user_message: &Message,
            ) -> anyhow::Result<crate::skills::r#trait::SkillOutput> {
                Ok(
                    crate::skills::r#trait::SkillOutput::done("Got fact").with_memory_updates(
                        vec![crate::skills::r#trait::MemoryUpdate {
                            fact_key: "color".to_string(),
                            fact_value: "azul".to_string(),
                            operation: crate::skills::r#trait::MemoryOperation::Set,
                        }],
                    ),
                )
            }
        }

        let mut skill_registry = SkillRegistry::new();
        skill_registry.register(Box::new(ContentWithPlanSkill));

        let mock_groq = MockLlmProvider::new();
        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(
            Role::User,
            "Mi color favorito es azul y después dime la hora",
        )];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(
            executor.has_pending_plan(),
            "Pending plan is set by B1 even with content"
        );
        assert!(
            result.should_continue,
            "B1: with content AND pending plan, should_continue must be true (loop must continue)"
        );
    }

    #[tokio::test]
    async fn test_executor_b1_sets_pending_plan_from_memory() {
        // Test that B1 (factual fragment skill) sets pending plan from create_plan
        // "Mi color favorito es azul y después dime la hora" triggers:
        // 1. B1: extract factual fragment + memory update
        // 2. create_plan on full content creates pending plan
        // 3. pending plan contains Direct step "deme la hora"
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .returning(|_, _| Box::pin(async { Ok("Final".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(
            Role::User,
            "Mi color favorito es azul y después dime la hora",
        )];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        // B1 should trigger, create_plan creates pending plan
        assert!(executor.has_pending_plan());
        assert!(result.should_continue);
        assert!(result.messages.is_empty());
    }

    #[tokio::test]
    async fn test_executor_non_cacheable_tool_executes_fresh() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![
            Message::new(Role::User, "decime la hora"),
            Message::new(Role::Tool, "Tool result available: previous result"),
        ];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Assistant);
    }

    #[test]
    fn test_compress_runtime_context_memory_same_key_keeps_last() {
        let messages = vec![
            Message::new(Role::User, "user1"),
            Message::new(Role::System, "MEMORY_SET:favorite_color=verde"),
            Message::new(Role::System, "MEMORY_SET:favorite_color=azul"),
            Message::new(Role::User, "user2"),
        ];

        let compressed = Executor::compress_runtime_context(&messages);

        let memory_count = compressed
            .iter()
            .filter(|m| m.role == Role::System && m.content.contains("MEMORY_"))
            .count();

        assert_eq!(memory_count, 1, "Should keep only one memory");
        assert!(
            compressed
                .iter()
                .any(|m| m.content.contains("favorite_color=azul")),
            "Should keep latest value"
        );
    }

    #[test]
    fn test_compress_runtime_context_memory_different_keys_keeps_both() {
        let messages = vec![
            Message::new(Role::User, "user1"),
            Message::new(Role::System, "MEMORY_SET:favorite_color=azul"),
            Message::new(Role::System, "MEMORY_SET:occupation=engineer"),
            Message::new(Role::User, "user2"),
        ];

        let compressed = Executor::compress_runtime_context(&messages);

        let memory_count = compressed
            .iter()
            .filter(|m| m.role == Role::System && m.content.contains("MEMORY_"))
            .count();

        assert_eq!(memory_count, 2, "Should keep both different keys");
    }

    #[test]
    fn test_compress_runtime_context_tool_compression_intact() {
        let messages = vec![
            Message::new(Role::User, "user1"),
            Message::new(Role::Tool, "Tool result: first"),
            Message::new(Role::Tool, "Tool result: second"),
            Message::new(Role::User, "user2"),
        ];

        let compressed = Executor::compress_runtime_context(&messages);

        let tool_count = compressed.iter().filter(|m| m.role == Role::Tool).count();

        assert_eq!(tool_count, 1, "Should keep only last tool");
        assert!(
            compressed.iter().any(|m| m.content.contains("second")),
            "Should keep second tool result"
        );
    }

    #[test]
    fn test_compress_runtime_context_memory_update_different_keys_preserved() {
        let messages = vec![
            Message::new(Role::User, "user1"),
            Message::new(Role::System, "MEMORY_UPDATE:color=verde"),
            Message::new(Role::System, "MEMORY_UPDATE:size=large"),
            Message::new(Role::User, "user2"),
        ];

        let compressed = Executor::compress_runtime_context(&messages);

        let memory_count = compressed
            .iter()
            .filter(|m| m.role == Role::System && m.content.starts_with("MEMORY_"))
            .count();

        assert_eq!(memory_count, 2, "Should keep both different memory updates");
    }

    #[test]
    fn test_compress_runtime_context_mixed_tool_and_memory_order_preserved() {
        let messages = vec![
            Message::new(Role::User, "user1"),
            Message::new(Role::System, "MEMORY_SET:favorite_color=verde"),
            Message::new(Role::Tool, "Tool result: first"),
            Message::new(Role::System, "MEMORY_SET:favorite_color=azul"),
            Message::new(Role::User, "user2"),
        ];

        let compressed = Executor::compress_runtime_context(&messages);

        let memory_count = compressed
            .iter()
            .filter(|m| m.role == Role::System && m.content.contains("MEMORY_"))
            .count();

        assert_eq!(memory_count, 1);

        let tool_count = compressed.iter().filter(|m| m.role == Role::Tool).count();

        assert_eq!(tool_count, 1);

        assert!(compressed
            .iter()
            .any(|m| m.content.contains("favorite_color=azul")));
    }

    #[tokio::test]
    async fn test_executor_tool_blocked_path() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async {
                    Ok("I will call get_current_time\nTOOL:get_current_time".to_string())
                })
            });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![
            Message::new(Role::User, "what time is it"),
            Message::new(Role::Tool, "Tool result available: UTC 10:00".to_string()),
        ];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(!result.should_continue, "Tool blocked should not continue");
        assert_eq!(result.messages.len(), 1, "Should return assistant message");
        assert_eq!(
            result.messages[0].role,
            Role::Assistant,
            "Should be assistant message"
        );
        assert!(
            !result.messages[0].content.contains("TOOL:"),
            "Tool call should be stripped"
        );
    }

    #[tokio::test]
    async fn test_executor_duplicate_tool_skip_path() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_weather".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![
            Message::new(Role::User, "weather?"),
            Message::new(Role::Tool, "Tool result available: sunny".to_string()),
        ];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(
            !result.should_continue,
            "Duplicate tool should not continue"
        );
        assert_eq!(result.messages.len(), 1, "Should return assistant message");
        assert_eq!(
            result.messages[0].role,
            Role::Assistant,
            "Should be assistant"
        );
    }

    #[tokio::test]
    async fn test_executor_fallback_no_tool_call() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("Hello, how can I help you?".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "hello")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(!result.should_continue, "Fallback should not continue");
        assert_eq!(result.messages.len(), 1, "Should return assistant message");
        assert_eq!(result.messages[0].role, Role::Assistant);
        assert_eq!(result.messages[0].content, "Hello, how can I help you?");
    }

    #[tokio::test]
    async fn test_executor_llm_with_tool_call_executed() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "what time")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue, "Tool execution should continue");
        assert_eq!(result.messages.len(), 1, "Should return tool message");
        assert_eq!(
            result.messages[0].role,
            Role::Tool,
            "Should be tool message"
        );
    }

    #[tokio::test]
    async fn test_executor_planner_with_pending_plan_direct() {
        let mock_groq = MockLlmProvider::new();

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        executor.pending_plan = Some(Plan {
            steps: vec![
                PlanStep::Direct("first".to_string()),
                PlanStep::Direct("second".to_string()),
                PlanStep::Direct("third".to_string()),
            ],
        });

        let messages = vec![Message::new(Role::User, "go")];

        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue);
        assert!(
            result.messages.is_empty(),
            "Direct step should not produce messages"
        );
        assert!(executor.has_pending_plan(), "Should have remaining plan");

        let remaining = executor.take_pending_plan().unwrap();
        assert_eq!(remaining.len(), 2, "Should have 2 remaining steps");
    }

    #[test]
    fn test_compress_runtime_context_empty_input() {
        let messages: Vec<Message> = vec![];
        let compressed = Executor::compress_runtime_context(&messages);
        assert!(
            compressed.is_empty(),
            "Empty input should return empty output"
        );
    }

    #[test]
    fn test_compress_runtime_context_only_tools_keeps_last() {
        let messages = vec![
            Message::new(Role::Tool, "Tool result: first"),
            Message::new(Role::Tool, "Tool result: second"),
            Message::new(Role::Tool, "Tool result: third"),
        ];

        let compressed = Executor::compress_runtime_context(&messages);

        assert_eq!(compressed.len(), 1, "Should keep only last tool");
        assert!(
            compressed[0].content.contains("third"),
            "Should be last tool result"
        );
    }

    #[tokio::test]
    async fn test_executor_pending_plan_sets_remaining_steps() {
        let mock_groq = MockLlmProvider::new();
        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        executor.pending_plan = Some(Plan {
            steps: vec![
                PlanStep::Tool("get_current_time".to_string()),
                PlanStep::Direct("continue".to_string()),
            ],
        });

        let messages = vec![Message::new(Role::User, "go")];
        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Tool);
        assert!(executor.has_pending_plan());
    }

    #[test]
    fn test_extract_memory_key_parses_correctly() {
        let key1 = Executor::extract_memory_key("MEMORY_SET:color=azul");
        assert_eq!(key1, Some("color".to_string()));

        let key2 = Executor::extract_memory_key("MEMORY_UPDATE:size=large");
        assert_eq!(key2, Some("size".to_string()));

        let key3 = Executor::extract_memory_key("MEMORY_DELETE:temp");
        assert_eq!(key3, Some("temp".to_string()));

        let key4 = Executor::extract_memory_key("MEMORY_SET:nokeyequals");
        assert_eq!(key4, Some("nokeyequals".to_string()));

        let key5 = Executor::extract_memory_key("USER:message");
        assert_eq!(key5, None);
    }

    #[tokio::test]
    async fn test_executor_tool_execution_error_path() {
        let mock_groq = MockLlmProvider::new();
        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        executor.pending_plan = Some(Plan {
            steps: vec![PlanStep::Tool("nonexistent_tool".to_string())],
        });

        let messages = vec![Message::new(Role::User, "test")];
        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Tool);
        assert!(
            result.messages[0].content.contains("Tool execution error"),
            "Should contain error message"
        );
    }

    #[tokio::test]
    async fn test_executor_should_skip_duplicate_when_fresh() {
        let mock_groq = MockLlmProvider::new();
        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);

        let should_skip = executor.should_skip_duplicate(
            "get_current_time",
            Some(&Message::new(
                Role::Tool,
                "Tool result available: UTC 10:00".to_string(),
            )),
        );

        assert!(
            !should_skip,
            "Non-cacheable tool should not be skipped even if result exists"
        );
    }

    #[tokio::test]
    async fn test_executor_llm_response_with_assistant_reasoning() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async {
                    Ok("Let me think about that.\nTOOL:get_current_time".to_string())
                })
            });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "what time")];
        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(result.should_continue);
        assert_eq!(result.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_executor_skill_with_content_no_pending_plan() {
        #[derive(Debug)]
        struct SimpleSkill;
        #[async_trait::async_trait]
        impl crate::skills::r#trait::Skill for SimpleSkill {
            fn name(&self) -> &str {
                "simple_skill"
            }
            fn description(&self) -> &str {
                "test"
            }
            fn side_effects(&self) -> crate::skills::r#trait::SideEffects {
                crate::skills::r#trait::SideEffects::none()
            }
            fn trigger(&self) -> crate::skills::r#trait::TriggerType {
                crate::skills::r#trait::TriggerType::OnPattern("hello")
            }
            async fn execute(
                &self,
                _context: &[Message],
                _user_message: &Message,
            ) -> anyhow::Result<crate::skills::r#trait::SkillOutput> {
                Ok(crate::skills::r#trait::SkillOutput::done("Hello back!"))
            }
        }

        let mut skill_registry = SkillRegistry::new();
        skill_registry.register(Box::new(SimpleSkill));

        let mock_groq = MockLlmProvider::new();
        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "hello world")];
        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Assistant);
    }

    #[test]
    fn test_executor_extract_assistant_text_strips_tool_line() {
        let text = "Let me think about this\nTOOL:get_current_time\nMore text";
        let result = Executor::extract_assistant_text(text);
        assert!(!result.contains("TOOL:"));
        assert!(result.contains("Let me think about this"));
    }

    #[test]
    fn test_executor_extract_assistant_text_no_tool() {
        let text = "Just a normal response";
        let result = Executor::extract_assistant_text(text);
        assert_eq!(result, "Just a normal response");
    }

    #[tokio::test]
    async fn test_executor_identical_replay_lifecycle_blocks_immediate() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(2)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_weather".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let result1 = executor.execute_step("sys", &[]).await.unwrap();
        assert!(!result1.should_continue, "Tool failure should not continue");
        assert_eq!(result1.messages[0].role, Role::Tool);

        let result2 = executor
            .execute_step("sys", &[Message::new(Role::User, "weather again")])
            .await
            .unwrap();
        assert!(
            !result2.should_continue,
            "Identical tool replay should be blocked"
        );
    }

    #[tokio::test]
    async fn test_executor_pending_plan_repeated_steps_allowed() {
        let mock_groq = MockLlmProvider::new();
        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        executor.pending_plan = Some(Plan {
            steps: vec![
                PlanStep::Tool("get_weather".to_string()),
                PlanStep::Tool("get_weather".to_string()),
            ],
        });

        let messages = vec![Message::new(Role::User, "go")];
        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(
            result.should_continue,
            "Repeated tool in pending_plan should be allowed"
        );
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Tool);
    }

    #[tokio::test]
    async fn test_executor_skill_triggers_and_returns_content() {
        #[derive(Debug)]
        struct TriggerSkill;
        #[async_trait::async_trait]
        impl crate::skills::r#trait::Skill for TriggerSkill {
            fn name(&self) -> &str {
                "trigger_skill"
            }
            fn description(&self) -> &str {
                "test"
            }
            fn side_effects(&self) -> crate::skills::r#trait::SideEffects {
                crate::skills::r#trait::SideEffects::none()
            }
            fn trigger(&self) -> crate::skills::r#trait::TriggerType {
                crate::skills::r#trait::TriggerType::OnPattern("test")
            }
            async fn execute(
                &self,
                _context: &[Message],
                _user_message: &Message,
            ) -> anyhow::Result<crate::skills::r#trait::SkillOutput> {
                Ok(crate::skills::r#trait::SkillOutput::done("Skill response"))
            }
        }

        let mut skill_registry = SkillRegistry::new();
        skill_registry.register(Box::new(TriggerSkill));

        let mock_groq = MockLlmProvider::new();
        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "test input")];
        let result = executor.execute_step("sys", &messages).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_executor_skill_not_stale_across_runs() {
        #[derive(Debug)]
        struct TriggerSkill;
        #[async_trait::async_trait]
        impl crate::skills::r#trait::Skill for TriggerSkill {
            fn name(&self) -> &str {
                "trigger_skill"
            }
            fn description(&self) -> &str {
                "test"
            }
            fn side_effects(&self) -> crate::skills::r#trait::SideEffects {
                crate::skills::r#trait::SideEffects::none()
            }
            fn trigger(&self) -> crate::skills::r#trait::TriggerType {
                crate::skills::r#trait::TriggerType::OnPattern("run")
            }
            async fn execute(
                &self,
                _context: &[Message],
                _user_message: &Message,
            ) -> anyhow::Result<crate::skills::r#trait::SkillOutput> {
                Ok(crate::skills::r#trait::SkillOutput::done("Skill response"))
            }
        }

        let db = crate::db::sqlite::Db::new(":memory:").unwrap();
        let memory = crate::agent::memory_bridge::MemoryBridge::new(&db, "user");
        let planner = crate::agent::planner::Planner::new();

        let mut skill_registry = SkillRegistry::new();
        skill_registry.register(Box::new(TriggerSkill));

        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .returning(|_, _| Box::pin(async { Ok("fallback".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = crate::agent::r#loop::AgentLoop::new(memory, planner, executor);

        let first_run = agent_loop
            .run(Message::new(Role::User, "trigger run first"))
            .await
            .unwrap();
        assert!(
            first_run.content.contains("Skill response"),
            "First run should execute skill"
        );

        let second_run = agent_loop
            .run(Message::new(Role::User, "trigger run second"))
            .await
            .unwrap();
        assert!(
            second_run.content.contains("Skill response"),
            "Second run should also execute skill (not stale)"
        );
    }
}
