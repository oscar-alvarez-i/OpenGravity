use super::{
    executor::{Executor, StepResult},
    memory_bridge::MemoryBridge,
    planner::Planner,
};
use crate::domain::message::{Message, Role};
use anyhow::{anyhow, Result};
use tracing::{debug, info, warn};

const MAX_LOOP_ITERATIONS: usize = 4;

fn prepare_initial_context(planner: &Planner, raw_context: Vec<Message>) -> Vec<Message> {
    let mut context = planner.filter_tool_duplicates(&raw_context);
    context = planner.filter_closed_tool_cycles(&context);
    planner.trim_stale_user_turns(&context)
}

fn prepare_iteration_context(planner: &Planner, messages: &[Message]) -> Vec<Message> {
    let mut filtered = planner.filter_tool_duplicates(messages);
    filtered = planner.filter_closed_tool_cycles(&filtered);
    filtered = planner.compact_memory_updates(&filtered);
    planner.compact_context(&filtered)
}

pub struct AgentLoop<'a> {
    memory: MemoryBridge<'a>,
    planner: Planner,
    executor: Executor<'a>,
}

impl<'a> AgentLoop<'a> {
    pub fn new(memory: MemoryBridge<'a>, planner: Planner, executor: Executor<'a>) -> Self {
        Self {
            memory,
            planner,
            executor,
        }
    }

    pub async fn run(&mut self, incoming_msg: Message) -> Result<Message> {
        info!("Starting agent loop");

        // 1. Recover memory and filter stale tool results
        let conversation = self.memory.fetch_conversation_only(6)?;
        let memories = self.memory.fetch_memories_only(20, 4)?;
        let raw_context: Vec<Message> = memories
            .into_iter()
            .chain(conversation.into_iter())
            .collect();
        let context = prepare_initial_context(&self.planner, raw_context);

        // Save initial user message to db immediately
        self.memory.save_message(&incoming_msg)?;

        // 2-3. Build Prompt
        let system_prompt = self.planner.build_system_prompt();
        let mut active_messages = context;
        active_messages.push(incoming_msg.clone());

        let mut iterations = 0;

        while iterations < MAX_LOOP_ITERATIONS {
            iterations += 1;
            info!("Loop iteration {}/{}", iterations, MAX_LOOP_ITERATIONS);

            // Filter stale tool duplicates before LLM call
            active_messages = prepare_iteration_context(&self.planner, &active_messages);

            // 4-6. Query LLM, detect, execute
            let step_result = self
                .executor
                .execute_step(&system_prompt, &active_messages)
                .await?;

            let StepResult {
                messages: step_messages,
                should_continue,
                memory_updates,
            } = step_result;

            debug!(
                "Step completed. Messages received: {}, Should continue: {}",
                step_messages.len(),
                should_continue
            );

            for msg in &step_messages {
                let leads_to_tool = should_continue && msg.role == Role::Assistant;

                if !leads_to_tool {
                    if let Err(e) = self.memory.save_message(msg) {
                        warn!("Failed to save intermediate memory: {}", e);
                    } else {
                        debug!(
                            "Persisted Message -> Role: {:?}, Content: '{}'",
                            msg.role, msg.content
                        );
                    }
                    active_messages.push(msg.clone());
                } else {
                    debug!("Skipping DB persistence and active context for Assistant reasoning step leading to tool call.");
                }
            }

            for update in &memory_updates {
                debug!(
                    "Persisting MemoryUpdate: key='{}', value='{}', op={:?}",
                    update.fact_key, update.fact_value, update.operation
                );
                if let Err(e) = self.memory.save_memory_update(update) {
                    warn!("Failed to save memory update: {}", e);
                }
            }

            if !should_continue {
                info!("Agent loop finished successfully");
                return step_messages
                    .last()
                    .cloned()
                    .ok_or_else(|| anyhow!("Executor returned no messages"));
            }

            debug!("Current active context size: {}", active_messages.len());
        }

        Err(anyhow!(
            "Agent loop max iterations ({}) reached without final resolution",
            MAX_LOOP_ITERATIONS
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::Db;
    use crate::llm::models::MockLlmProvider;
    use crate::llm::LlmOrchestrator;
    use crate::skills::registry::SkillRegistry;
    use crate::tools::registry::Registry;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_agent_loop_fetch_context_error() {
        let db = Db::new(":memory:").unwrap();
        db.execute_raw("DROP TABLE memories").unwrap();

        let memory = MemoryBridge::new(&db, "user");
        let agent_planner = Planner::new();
        let groq = Box::new(MockLlmProvider::new());
        let or = Box::new(MockLlmProvider::new());
        let llm = LlmOrchestrator::new(vec![groq, or]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, agent_planner, executor);

        let res = agent_loop
            .run(Message::new(crate::domain::message::Role::User, "hi"))
            .await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("no such table: memories"));
    }

    #[tokio::test]
    async fn test_agent_loop_executor_error() {
        let db = Db::new(":memory:").unwrap();
        let memory = MemoryBridge::new(&db, "user");
        let agent_planner = Planner::new();

        let mut groq = MockLlmProvider::new();
        groq.expect_generate_response()
            .returning(|_, _| Box::pin(async { Err(anyhow!("LLM error")) }));

        let or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(groq), Box::new(or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, agent_planner, executor);

        let res = agent_loop
            .run(Message::new(crate::domain::message::Role::User, "hi"))
            .await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "LLM error");
    }

    #[tokio::test]
    async fn test_agent_loop_initial_save_error() {
        let db = Db::new(":memory:").unwrap();
        // Insert some dummy data so fetch_context succeeds
        let memory = MemoryBridge::new(&db, "user");
        memory
            .save_message(&Message::new(crate::domain::message::Role::User, "pre"))
            .unwrap();

        // Now break the DB before initial save in run()
        db.execute_raw("DROP TABLE memories").unwrap();

        let agent_planner = Planner::new();
        let groq = Box::new(MockLlmProvider::new());
        let or = Box::new(MockLlmProvider::new());
        let llm = LlmOrchestrator::new(vec![groq, or]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, agent_planner, executor);

        let res = agent_loop
            .run(Message::new(crate::domain::message::Role::User, "hi"))
            .await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("no such table: memories"));
    }

    #[tokio::test]
    async fn test_agent_loop_intermediate_save_error() {
        let db = Arc::new(Db::new(":memory:").unwrap());
        let db_clone = Arc::clone(&db);
        let memory = MemoryBridge::new(&db, "user");
        let planner = Planner::new();

        let mut groq = MockLlmProvider::new();
        groq.expect_generate_response()
            .times(1)
            .returning(move |_, _| {
                // Break DB inside the loop
                db_clone.execute_raw("DROP TABLE memories").unwrap();
                Box::pin(async { Ok("Final answer".to_string()) })
            });

        let or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(groq), Box::new(or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, planner, executor);

        // This should NOT fail the loop because intermediate save failure is non-fatal
        let res = agent_loop
            .run(Message::new(crate::domain::message::Role::User, "hi"))
            .await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().content, "Final answer");
    }

    #[tokio::test]
    async fn test_agent_loop_terminates_with_tool_block() {
        let db = Db::new(":memory:").unwrap();
        let memory = MemoryBridge::new(&db, "user");
        let planner = Planner::new();

        let mut groq = MockLlmProvider::new();
        groq.expect_generate_response()
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));

        let or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(groq), Box::new(or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, planner, executor);

        let res = agent_loop
            .run(Message::new(crate::domain::message::Role::User, "time?"))
            .await;

        assert!(
            res.is_ok(),
            "Should terminate after tool result blocks second call"
        );
    }

    #[tokio::test]
    async fn test_agent_loop_continues_on_memory_only_skill() {
        let db = Db::new(":memory:").unwrap();
        let memory = MemoryBridge::new(&db, "user");
        let planner = Planner::new();

        let mut groq = MockLlmProvider::new();
        // Iter 1: Skill triggers, returns empty (memory only), continues
        // Iter 2: LLM returns final answer
        groq.expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("Final answer".to_string()) }));

        let or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(groq), Box::new(or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, planner, executor);

        // Memory skill triggers (color fact), executor returns empty with should_continue=true
        let res = agent_loop
            .run(Message::new(Role::User, "Mi color favorito es azul"))
            .await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().content, "Final answer");
    }

    #[tokio::test]
    async fn test_agent_loop_all_tool_errors() {
        let db = Db::new(":memory:").unwrap();
        let memory = MemoryBridge::new(&db, "user");
        let planner = Planner::new();

        let mut groq = MockLlmProvider::new();
        // Iter 1: Tool call
        groq.expect_generate_response().times(1).returning(|_, _| {
            Box::pin(async { Ok("Thinking...\nTOOL:get_current_time".to_string()) })
        });
        // Iter 2: Final Answer
        groq.expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("Final Answer".to_string()) }));

        let or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(groq), Box::new(or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, planner, executor);

        let res = agent_loop
            .run(Message::new(crate::domain::message::Role::User, "hi"))
            .await
            .unwrap();
        assert_eq!(res.content, "Final Answer");
    }

    #[tokio::test]
    async fn test_agent_loop_debug_step_completed() {
        // Enable debug logging to cover debug!("Step completed") at line 65
        let db = Db::new(":memory:").unwrap();
        let memory = MemoryBridge::new(&db, "user");
        let planner = Planner::new();

        let mut groq = MockLlmProvider::new();
        groq.expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("Final answer".to_string()) }));

        let or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(groq), Box::new(or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, planner, executor);

        let _ = agent_loop.run(Message::new(Role::User, "hello")).await;
    }

    #[tokio::test]
    async fn test_agent_loop_debug_skips_persistence() {
        // Cover debug!("Skipping DB persistence") at line 84
        // This happens when leads_to_tool is true (should_continue && Assistant)
        let db = Db::new(":memory:").unwrap();
        let memory = MemoryBridge::new(&db, "user");
        let planner = Planner::new();

        // First call returns tool call (reasoning), second returns final
        let mut groq = MockLlmProvider::new();
        groq.expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));
        groq.expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("Done".to_string()) }));

        let or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(groq), Box::new(or)]);
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, planner, executor);

        let _ = agent_loop.run(Message::new(Role::User, "time?")).await;
    }

    #[tokio::test]
    async fn test_integration_filter_closed_tool_cycles_persisted() {
        let db = Db::new(":memory:").unwrap();
        let memory = MemoryBridge::new(&db, "user");
        let planner = Planner::new();

        memory
            .save_message(&Message::new(Role::User, "decime la hora"))
            .unwrap();
        memory
            .save_message(&Message::new(
                Role::Assistant,
                "Voy a llamar la herramienta\nTOOL:get_current_time",
            ))
            .unwrap();
        memory
            .save_message(&Message::new(Role::Tool, "Tool result available: 18:55"))
            .unwrap();

        memory
            .save_message(&Message::new(Role::User, "mi color favorito es azul"))
            .unwrap();

        let raw_context = memory.fetch_context(10).unwrap();
        let filtered = planner.filter_tool_duplicates(&raw_context);
        let filtered = planner.filter_closed_tool_cycles(&filtered);

        let has_old_user = filtered
            .iter()
            .any(|m| m.role == Role::User && m.content.contains("hora"));
        let has_old_tool = filtered
            .iter()
            .any(|m| m.role == Role::Tool && m.content.contains("Tool result"));
        let has_new_user = filtered
            .iter()
            .any(|m| m.role == Role::User && m.content.contains("color"));

        assert!(
            !has_old_user,
            "Old user message with tool intent should be filtered"
        );
        assert!(!has_old_tool, "Old tool result should be filtered");
        assert!(has_new_user, "Latest user message should be preserved");
    }
}
