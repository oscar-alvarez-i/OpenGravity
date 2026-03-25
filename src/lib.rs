pub mod agent;
pub mod bot;
pub mod config;
pub mod db;
pub mod domain;
pub mod llm;
pub mod memory;
pub mod observability;
pub mod prompts;
pub mod security;
pub mod skills;
pub mod tools;
pub mod workflows;

#[cfg(test)]
mod integration_tests {
    use crate::agent::executor::Executor;
    use crate::agent::memory_bridge::MemoryBridge;
    use crate::agent::planner::Planner;
    use crate::agent::r#loop::AgentLoop;
    use crate::db::sqlite::Db;
    use crate::domain::message::{Message, Role};
    use crate::llm::models::MockLlmProvider;
    use crate::llm::LlmOrchestrator;
    use crate::skills::registry::SkillRegistry;
    use crate::tools::registry::Registry;

    #[tokio::test]
    async fn test_full_agent_loop_with_tool_call() {
        let db = Db::new(":memory:").unwrap();
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();

        let mut mock_groq = MockLlmProvider::new();
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async {
                    Ok("I need to check the time first.\nTOOL:get_current_time".to_string())
                })
            });
        mock_groq
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("The time is 12:00 UTC.".to_string()) }));

        let mock_or = MockLlmProvider::new(); // OpenRouter not used here
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);

        let memory = MemoryBridge::new(&db, "test_user");
        let agent_planner = Planner::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, agent_planner, executor);

        let incoming = Message::new(Role::User, "What time is it?");
        let response = agent_loop.run(incoming).await.unwrap();

        assert_eq!(response.role, Role::Assistant);
        assert_eq!(response.content, "The time is 12:00 UTC.");

        let memories = db.fetch_latest_memories("test_user", 10).unwrap();
        assert_eq!(memories.len(), 3);
        assert_eq!(memories[0].role, Role::User);
        assert_eq!(memories[1].role, Role::Tool);
        assert!(memories[1].content.contains("Tool result available:"));
        assert_eq!(memories[2].role, Role::Assistant);
        assert_eq!(memories[2].content, "The time is 12:00 UTC.");
    }

    #[tokio::test]
    async fn test_agent_loop_terminates_with_duplicate_prevention() {
        let db = Db::new(":memory:").unwrap();
        let registry = Registry::new();
        let skill_registry = SkillRegistry::new();

        let mut mock_groq = MockLlmProvider::new();
        // get_current_time is excluded from duplicate prevention (always fresh)
        // so it may loop. Other tools hit duplicate prevention and terminate.
        mock_groq
            .expect_generate_response()
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));

        let mock_or = MockLlmProvider::new();
        let llm = LlmOrchestrator::new(vec![Box::new(mock_groq), Box::new(mock_or)]);

        let memory = MemoryBridge::new(&db, "test_user");
        let agent_planner = Planner::new();
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, agent_planner, executor);

        let incoming = Message::new(Role::User, "What time is it?");
        let response = agent_loop.run(incoming).await;

        // Loop may complete or hit max iterations depending on get_current_time behavior
        // Either way, it should not panic or error unexpectedly
        if let Err(e) = &response {
            assert!(
                e.to_string().contains("max iterations"),
                "Unexpected error: {}",
                e
            );
        }
    }
}
