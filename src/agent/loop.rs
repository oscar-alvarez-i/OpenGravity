use super::{executor::Executor, memory_bridge::MemoryBridge, planner::Planner};
use crate::domain::message::{Message, Role};
use anyhow::{anyhow, Result};
use tracing::{debug, info, warn};

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

    pub async fn run(&self, incoming_msg: Message) -> Result<Message> {
        info!("Starting agent loop");

        // 1. Recover memory
        let context = self.memory.fetch_context(10)?;

        // Save initial user message to db immediately
        self.memory.save_message(&incoming_msg)?;

        // 2-3. Build Prompt
        let system_prompt = self.planner.build_system_prompt();
        let mut active_messages = self.planner.assemble_messages(&context, &incoming_msg);

        let mut iterations = 0;
        let max_iterations = 3;

        while iterations < max_iterations {
            iterations += 1;
            info!("Loop iteration {}/{}", iterations, max_iterations);

            // 4-6. Query LLM, detect, execute
            let (step_messages, should_continue) = self
                .executor
                .execute_step(&system_prompt, &active_messages)
                .await?;

            debug!(
                "Step completed. Messages received: {}, Should continue: {}",
                step_messages.len(),
                should_continue
            );

            for msg in &step_messages {
                // Assistant reasoning should not be persisted OR added to active context when it leads to a tool call
                // (Observation 1: Prevent context pollution from internal reasoning)
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

            if !should_continue {
                // Return final answer (should be the last message)
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
            max_iterations
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::Db;
    use crate::llm::models::MockLlmProvider;
    use crate::llm::LlmOrchestrator;
    use crate::tools::registry::Registry;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_agent_loop_fetch_context_error() {
        let db = Db::new(":memory:").unwrap();
        // Break the DB explicitly by dropping the table it needs
        db.execute_raw("DROP TABLE memories").unwrap();

        let memory = MemoryBridge::new(&db, "user");
        let planner = Planner::new();
        let groq = Box::new(MockLlmProvider::new());
        let or = Box::new(MockLlmProvider::new());
        let llm = LlmOrchestrator::new(groq, or);
        let registry = Registry::new();
        let executor = Executor::new(&llm, &registry);
        let agent_loop = AgentLoop::new(memory, planner, executor);

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
        let planner = Planner::new();

        let mut groq = MockLlmProvider::new();
        groq.expect_generate_response()
            .returning(|_, _| Box::pin(async { Err(anyhow!("LLM error")) }));

        let or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(groq), Box::new(or));
        let registry = Registry::new();
        let executor = Executor::new(&llm, &registry);
        let agent_loop = AgentLoop::new(memory, planner, executor);

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

        let planner = Planner::new();
        let groq = Box::new(MockLlmProvider::new());
        let or = Box::new(MockLlmProvider::new());
        let llm = LlmOrchestrator::new(groq, or);
        let registry = Registry::new();
        let executor = Executor::new(&llm, &registry);
        let agent_loop = AgentLoop::new(memory, planner, executor);

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
        let llm = LlmOrchestrator::new(Box::new(groq), Box::new(or));
        let registry = Registry::new();
        let executor = Executor::new(&llm, &registry);
        let agent_loop = AgentLoop::new(memory, planner, executor);

        // This should NOT fail the loop because intermediate save failure is non-fatal
        let res = agent_loop
            .run(Message::new(crate::domain::message::Role::User, "hi"))
            .await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().content, "Final answer");
    }

    #[tokio::test]
    async fn test_agent_loop_max_iterations() {
        let db = Db::new(":memory:").unwrap();
        let memory = MemoryBridge::new(&db, "user");
        let planner = Planner::new();

        let mut groq = MockLlmProvider::new();
        // Return TOOL call 4 times (max is 3) or until loop stops
        groq.expect_generate_response()
            .times(3)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));

        let or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(Box::new(groq), Box::new(or));
        let registry = Registry::new();
        let executor = Executor::new(&llm, &registry);
        let agent_loop = AgentLoop::new(memory, planner, executor);

        let res = agent_loop
            .run(Message::new(crate::domain::message::Role::User, "hi"))
            .await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("max iterations (3) reached"));
    }

    #[tokio::test]
    async fn test_agent_loop_with_tool_success() {
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
        let llm = LlmOrchestrator::new(Box::new(groq), Box::new(or));
        let registry = Registry::new();
        let executor = Executor::new(&llm, &registry);
        let agent_loop = AgentLoop::new(memory, planner, executor);

        let res = agent_loop
            .run(Message::new(crate::domain::message::Role::User, "hi"))
            .await
            .unwrap();
        assert_eq!(res.content, "Final Answer");
    }
}
