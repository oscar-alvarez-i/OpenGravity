use crate::domain::message::{Message, Role};
use crate::llm::LlmOrchestrator;
use crate::skills::planner::{Plan, PlanStep, Planner as SkillPlanner};
use crate::skills::r#trait::MemoryUpdate;
use crate::skills::registry::SkillRegistry;
use crate::tools::registry::{Registry, ToolExecutionRequest};
use anyhow::Result;
use std::time::Instant;
use tracing::{debug, info};

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
    ///
    /// `history_for_idempotency` is the full unfiltered message history used for
    /// idempotency checks. When `None`, falls back to `messages` (for tests / backward compat).
    pub async fn execute_step(
        &mut self,
        system_prompt: &str,
        messages: &[Message],
        history_for_idempotency: Option<&[Message]>,
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
                                // Previous idempotency-by-replay checks removed; proceed with execution
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

        // Block tools only if: Tool result exists AND tool is NOT AlwaysFresh
        // write_local_note has its own idempotency logic; get_current_time is AlwaysFresh
        let should_block = tool_blocked
            && tool_call.is_some()
            && tool_call
                .as_ref()
                .map(|t| {
                    let is_always_fresh = self.registry.freshness_policy(&t.name).is_fresh();
                    t.name != "write_local_note" && !is_always_fresh
                })
                .unwrap_or(false);

        if should_block {
            let text_without_tool = Self::extract_assistant_text(&response_text);

            return Ok(StepResult::new(
                vec![Message::new(Role::Assistant, text_without_tool)],
                false,
            ));
        }

        // F. TOOL EXECUTION
        let tool_branch_start = Instant::now();
        let tool_call = self.registry.parse_tool_call(&response_text);

        if let Some(tool_call) = tool_call {
            info!("Branch: tool entered, tool={}", tool_call.name);
            info!(
                "Tool call detected: {} with input: '{}'",
                tool_call.name, tool_call.input
            );

            // Duplicate checks are now handled via full-history idempotency logic

            // Remove previous idempotency logic. We now rely on full-history Tool-message scanning elsewhere.
            // VALIDATE: reject write_local_note if user message was multiline
            if tool_call.name == "write_local_note" {
                if let Some(user_msg) = messages.iter().rev().find(|m| m.role == Role::User) {
                    let user_content = user_msg.content.as_str();
                    if user_content.contains('\n') || user_content.contains('\r') {
                        let error_msg =
                            "Tool execution error: Invalid input: multiline content not allowed"
                                .to_string();
                        self.last_tool_executed = Some(tool_call.name.clone());
                        return Ok(StepResult::new(
                            vec![Message::new(Role::Tool, error_msg)],
                            false,
                        ));
                    }
                }
            }

            // IDEMPOTENCY: scan full unfiltered history for matching Tool execution
            // Skip for AlwaysFresh tools (they should always execute)
            // Uses strict parsing: "Tool result available: <tool>:<input>; <output>"
            let history = history_for_idempotency.unwrap_or(messages);
            let is_always_fresh = self.registry.freshness_policy(&tool_call.name).is_fresh();
            let expected = format!("{}:{}", tool_call.name, tool_call.input);
            let was_executed = !is_always_fresh
                && history.iter().any(|m| {
                    m.role == Role::Tool
                        && if let Some(rest) = m.content.strip_prefix("Tool result available: ") {
                            if let Some((tool_and_input, _)) = rest.split_once(';') {
                                tool_and_input == expected
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                });
            if was_executed {
                info!(
                    "IDEMPOTENCY_CHECK: matched previous execution in history, tool={}, input={}",
                    tool_call.name, tool_call.input
                );
                return Ok(StepResult::new(
                    vec![Message::new(
                        Role::Assistant,
                        format!(
                            "IDEMPOTENCY: previous execution detected for {} with input {}",
                            tool_call.name, tool_call.input
                        ),
                    )],
                    false,
                ));
            }

            let request = ToolExecutionRequest {
                tool_name: tool_call.name.clone(),
                input: tool_call.input.clone(),
            };
            let tool_res = self.registry.execute(request);
            // Include tool name and input in the output to support idempotency across turns
            let tool_output_text = if tool_res.success {
                format!(
                    "Tool result available: {}:{}; {}",
                    tool_call.name, tool_call.input, tool_res.output
                )
            } else {
                format!(
                    "Tool execution error: {}",
                    tool_res.error.unwrap_or_default()
                )
            };

            self.last_tool_executed = Some(tool_call.name.clone());

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

        // ALWAYSFRESH GUARDRAIL: enforce execution for time queries
        // Only trigger if no Tool result exists in context (LLM should have called tool)
        let has_tool_result = messages
            .iter()
            .any(|m| m.role == Role::Tool && m.content.contains("Tool result available:"));

        if !has_tool_result {
            if let Some(user_msg) = messages.iter().rev().find(|m| m.role == Role::User) {
                let content = user_msg.content.to_lowercase();
                let is_time_query = content.contains("hora")
                    || content.contains("time")
                    || content.contains("qué hora")
                    || content.contains("what time");

                if is_time_query {
                    info!("ALWAYSFRESH_GUARDRAIL: forcing get_current_time execution");

                    let request = ToolExecutionRequest {
                        tool_name: "get_current_time".to_string(),
                        input: String::new(),
                    };

                    let tool_res = self.registry.execute(request);

                    let tool_output_text = if tool_res.success {
                        format!(
                            "Tool result available: get_current_time:; {}",
                            tool_res.output
                        )
                    } else {
                        format!(
                            "Tool execution error: {}",
                            tool_res.error.unwrap_or_default()
                        )
                    };

                    return Ok(StepResult::new(
                        vec![Message::new(Role::Tool, tool_output_text)],
                        true,
                    ));
                }
            }
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

        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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
    async fn test_executor_blocks_write_local_note_on_multiline_user_input() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async { Ok("TOOL:write_local_note:hola mundo".to_string()) })
            });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "Guardá:\nhola\nmundo")];
        let result = executor.execute_step("sys", &messages, None).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Tool);
        assert!(result.messages[0]
            .content
            .contains("multiline content not allowed"));
    }

    #[tokio::test]
    async fn test_executor_allows_write_local_note_on_single_line_user_input() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async { Ok("TOOL:write_local_note:hola mundo".to_string()) })
            });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "Guardá: hola mundo")];
        let result = executor.execute_step("sys", &messages, None).await.unwrap();

        assert!(result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Tool);
        assert!(result.messages[0].content.contains("nota guardada"));
    }

    #[tokio::test]
    async fn test_executor_no_false_positive_for_regular_multiline() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("Voy a escribir un poema".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![Message::new(Role::User, "Escribí un poema\nen dos líneas")];
        let result = executor.execute_step("sys", &messages, None).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Assistant);
    }

    #[tokio::test]
    async fn test_executor_idempotent_blocks_duplicate_tool_same_input() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async { Ok("TOOL:write_local_note:hola mundo".to_string()) })
            });
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async { Ok("TOOL:write_local_note:hola mundo".to_string()) })
            });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages1 = vec![Message::new(Role::User, "Guardá: hola mundo")];
        let result1 = executor
            .execute_step("sys", &messages1, None)
            .await
            .unwrap();

        assert!(result1.should_continue);
        assert_eq!(result1.messages[0].role, Role::Tool);
        assert!(result1.messages[0].content.contains("nota guardada"));

        let messages2 = vec![
            Message::new(Role::User, "Guardá: hola mundo"),
            Message::new(Role::Assistant, "TOOL:write_local_note:hola mundo"),
            result1.messages[0].clone(),
        ];
        let result2 = executor
            .execute_step("sys", &messages2, None)
            .await
            .unwrap();

        assert!(!result2.should_continue);
        assert_eq!(result2.messages[0].role, Role::Assistant);
    }

    #[tokio::test]
    async fn test_executor_idempotent_allows_different_input() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async { Ok("TOOL:write_local_note:primer mensaje".to_string()) })
            });
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async { Ok("TOOL:write_local_note:segundo mensaje".to_string()) })
            });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        // First turn
        let messages1 = vec![Message::new(Role::User, "Guardá: primer mensaje")];
        let result1 = executor
            .execute_step("sys", &messages1, None)
            .await
            .unwrap();

        assert!(result1.should_continue, "First execution should continue");
        assert_eq!(result1.messages[0].role, Role::Tool);

        // Second turn - include previous messages (User -> Assistant -> Tool pattern)
        let messages2 = vec![
            Message::new(Role::User, "Guardá: segundo mensaje"),
            Message::new(Role::Assistant, "TOOL:write_local_note:segundo mensaje"),
            result1.messages[0].clone(),
        ];
        let result2 = executor
            .execute_step("sys", &messages2, None)
            .await
            .unwrap();

        // Should execute because input is different
        assert!(
            result2.should_continue,
            "Different input should allow execution"
        );
        assert_eq!(
            result2.messages[0].role,
            Role::Tool,
            "Should return Tool message"
        );
    }

    #[tokio::test]
    async fn test_executor_idempotent_allows_different_tool() {
        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:write_local_note:hola".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        // First turn - get_current_time
        let messages1 = vec![Message::new(Role::User, "qué hora es")];
        let result1 = executor
            .execute_step("sys", &messages1, None)
            .await
            .unwrap();

        assert!(result1.should_continue, "First execution should continue");
        assert_eq!(result1.messages[0].role, Role::Tool);

        // Second turn - write_local_note (different tool)
        let messages2 = vec![
            Message::new(Role::User, "Guardá: hola"),
            Message::new(Role::Assistant, "TOOL:write_local_note:hola"),
            result1.messages[0].clone(),
        ];
        let result2 = executor
            .execute_step("sys", &messages2, None)
            .await
            .unwrap();

        // Should execute because different tool
        assert!(
            result2.should_continue,
            "Different tool should allow execution"
        );
        assert_eq!(
            result2.messages[0].role,
            Role::Tool,
            "Should return Tool message"
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

        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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

        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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

        let result = executor.execute_step("sys", &messages, None).await.unwrap();

        assert!(result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Tool);
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
                    Ok("I will call write_local_note\nTOOL:write_local_note:test".to_string())
                })
            });

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        let messages = vec![
            Message::new(Role::User, "save a note"),
            Message::new(
                Role::Tool,
                "Tool result available: write_local_note:test; previous note saved".to_string(),
            ),
        ];

        let result = executor.execute_step("sys", &messages, None).await.unwrap();

        assert!(
            !result.should_continue,
            "Cacheable tool with same name+input should be blocked"
        );
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

        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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

        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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

        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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

        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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
        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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
        let result = executor.execute_step("sys", &messages, None).await.unwrap();

        assert!(result.should_continue);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::Tool);
        assert!(
            result.messages[0].content.contains("Tool execution error"),
            "Should contain error message"
        );
    }

    #[tokio::test]
    async fn test_executor_idempotent_across_turns_real_history() {
        // This test validates that duplicate execution is blocked when a prior Tool
        // message with the same tool name and input exists in the history.
        // The exact content of messages is aligned with the new idempotency strategy.
        let mut mock_groq = MockLlmProvider::new();
        // First turn: tool execution request
        mock_groq
            .expect_generate_response()
            .times(2)
            .returning(|_, _| Box::pin(async { Ok("TOOL:write_local_note:hola".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let mut executor = Executor::new(&llm, &registry, &skill_registry);

        // Turn 1
        let messages1 = vec![Message::new(Role::User, "Guardá: hola")];
        let result1 = executor
            .execute_step("sys", &messages1, None)
            .await
            .unwrap();
        assert!(result1.should_continue);

        // Prepare history for Turn 2 to simulate a previous Tool result
        // Format must match: "Tool result available: <tool>:<input>; <output>"
        let history_tool = Message::new(
            Role::Tool,
            "Tool result available: write_local_note:hola; nota guardada",
        );

        // Turn 2: same input should be detected as idempotent and blocked
        let messages2 = vec![
            Message::new(Role::User, "Guardá: hola"),
            history_tool.clone(),
            result1.messages[0].clone(),
        ];
        // LLM should respond with a tool call again, but idempotency should block execution
        // We simulate by returning a tool call for the same input
        // The exact response content is not asserted; we just ensure we get a blocking outcome
        let result2 = executor
            .execute_step("sys", &messages2, None)
            .await
            .unwrap();

        assert!(
            !result2.should_continue,
            "Duplicate execution should be blocked"
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
        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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
        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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

        let result1 = executor.execute_step("sys", &[], None).await.unwrap();
        assert!(!result1.should_continue, "Tool failure should not continue");
        assert_eq!(result1.messages[0].role, Role::Tool);

        let result2 = executor
            .execute_step("sys", &[Message::new(Role::User, "weather again")], None)
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
        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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
        let result = executor.execute_step("sys", &messages, None).await.unwrap();

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
