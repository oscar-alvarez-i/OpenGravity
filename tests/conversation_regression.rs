//! Deterministic conversation regression tests.
//!
//! These tests protect against semantic regressions in the agent loop and tool execution protocol.
//! They use mock infrastructure to avoid external dependencies and provider token consumption.

use anyhow::Result;
use mockall::{mock, Sequence};
use open_gravity::agent::executor::Executor;
use open_gravity::agent::memory_bridge::MemoryBridge;
use open_gravity::agent::planner::Planner;
use open_gravity::agent::r#loop::AgentLoop;
use open_gravity::db::sqlite::Db;
use open_gravity::domain::message::{Message, Role};
use open_gravity::llm::models::LlmProvider;
use open_gravity::llm::LlmOrchestrator;
use open_gravity::skills::registry::SkillRegistry;
use open_gravity::tools::registry::Registry;

mock! {
    pub RegressionMockProvider {}
    #[async_trait::async_trait]
    impl LlmProvider for RegressionMockProvider {
        async fn generate_response(&self, system: &str, messages: &[Message]) -> Result<String>;
    }
}

// --- Test Group A: Tool Loop Stability ---

/**
 * test_tool_single_execution_finalizes
 *
 * Mock:
 * 1. TOOL:get_current_time
 * 2. final natural answer
 *
 * Validate:
 * - exactly one tool execution
 * - loop exits before max iteration
 * - persisted ordering correct (User -> Tool -> Assistant final)
 */
#[tokio::test]
async fn test_tool_single_execution_finalizes() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let mut seq = Sequence::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // 1. Tool call
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Checking time.\nTOOL:get_current_time".to_string()));

    // 2. Final answer
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("The time is 12:00.".to_string()));

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );
    let memory = MemoryBridge::new(&db, "test_user");
    let planner = Planner::new();
    let executor = Executor::new(&llm, &registry, &skill_registry);
    let agent_loop = AgentLoop::new(memory, planner, executor);

    let incoming = Message::new(Role::User, "What time is it?");
    let _ = agent_loop.run(incoming).await?;

    let memories = db.fetch_latest_memories("test_user", 10)?;

    // Expect: User, Tool, Assistant Final
    // "Checking time." Assistant reasoning should NOT be persisted (Observation 1)
    assert_eq!(
        memories.len(),
        3,
        "Should persist only User, Tool, and Final Assistant"
    );
    assert_eq!(memories[0].role, Role::User);
    assert_eq!(memories[1].role, Role::Tool);
    assert!(memories[1].content.contains("Tool result available:"));
    assert_eq!(memories[2].role, Role::Assistant);
    assert_eq!(memories[2].content, "The time is 12:00.");

    Ok(())
}

/**
 * test_tool_repeated_same_tool_hits_safe_boundary
 *
 * Duplicate tool prevention now protects against infinite loops.
 * When the same tool is called repeatedly, it's blocked after the first execution.
 */
#[tokio::test]
async fn test_tool_repeated_same_tool_hits_safe_boundary() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // Two calls: first executes tool, second is blocked by duplicate prevention
    mock_llm
        .expect_generate_response()
        .times(2)
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );
    let memory = MemoryBridge::new(&db, "test_user");
    let planner = Planner::new();
    let executor = Executor::new(&llm, &registry, &skill_registry);
    let agent_loop = AgentLoop::new(memory, planner, executor);

    let result = agent_loop.run(Message::new(Role::User, "loop test")).await;

    // With duplicate prevention, repeated same tool calls are blocked
    // So the loop terminates successfully instead of hitting max iterations
    assert!(result.is_ok());

    Ok(())
}

/**
 * test_tool_reasoning_not_persisted
 *
 * Validate persisted DB does NOT contain reasoning message when a tool is called.
 */
#[tokio::test]
async fn test_tool_reasoning_not_persisted() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let mut seq = Sequence::new();

    let mut mock_llm = MockRegressionMockProvider::new();
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Let me think... I need a tool.\nTOOL:get_current_time".to_string()));
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Done.".to_string()));

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );
    let memory = MemoryBridge::new(&db, "test_user");
    let agent_loop = AgentLoop::new(
        memory,
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    );

    let _ = agent_loop.run(Message::new(Role::User, "test")).await?;

    let memories = db.fetch_latest_memories("test_user", 10)?;
    for m in &memories {
        assert!(
            !m.content.contains("Let me think"),
            "Reasoning should not be in DB"
        );
    }

    Ok(())
}

/**
 * test_active_context_excludes_reasoning_after_tool
 *
 * Validates that after a TOOL call, the reasoning message is NOT in active context
 * sent to the next step.
 */
#[tokio::test]
async fn test_active_context_excludes_reasoning_after_tool() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let mut seq = Sequence::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // 1. First step returns reasoning + TOOL
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("I must consult the orb.\nTOOL:get_current_time".to_string()));

    // 2. Second step SHOULD NOT see "consult the orb"
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            let poisoned = messages
                .iter()
                .any(|m| m.content.contains("consult the orb"));
            if poisoned {
                Err(anyhow::anyhow!("Reasoning leaked into context!"))
            } else {
                Ok("The orb says 12:00.".to_string())
            }
        });

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );
    let agent_loop = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    );

    let res = agent_loop
        .run(Message::new(Role::User, "Orb time?"))
        .await?;
    assert_eq!(res.content, "The orb says 12:00.");

    Ok(())
}

// --- Test Group B: Memory Continuity ---

/**
 * test_memory_short_fact_recall
 *
 * "Mi color favorito es azul"
 * "Cuál es mi color favorito?"
 */
#[tokio::test]
async fn test_memory_short_fact_recall() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let planner = Planner::new();
    let mut seq = Sequence::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // Turn 1
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Entendido, azul es tu color favorito.".to_string()));

    // Turn 2
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            // Check that previous context is present
            let found = messages.iter().any(|m| m.content.contains("azul"));
            assert!(found, "Previous fact should be in context");
            Ok("Tu color favorito es azul.".to_string())
        });

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );

    // Run Turn 1
    {
        let memory = MemoryBridge::new(&db, "test_user");
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let agent_loop = AgentLoop::new(memory, planner.clone(), executor);
        agent_loop
            .run(Message::new(Role::User, "Mi color favorito es azul"))
            .await?;
    }

    // Run Turn 2
    {
        let memory = MemoryBridge::new(&db, "test_user");
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let agent_loop = AgentLoop::new(memory, planner, executor);
        let res = agent_loop
            .run(Message::new(Role::User, "Cuál es mi color favorito?"))
            .await?;
        assert!(res.content.contains("azul"));
    }

    Ok(())
}

/**
 * test_memory_with_tool_interleaving
 *
 * "Mi color favorito es verde"
 * "Qué hora es?"
 * "Cuál es mi color favorito?"
 */
#[tokio::test]
async fn test_memory_with_tool_interleaving() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let mut seq = Sequence::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // Turn 1: Save fact
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Verde, anotado.".to_string()));

    // Turn 2: Tool call
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Checking...\nTOOL:get_current_time".to_string()));
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Son las 12:00.".to_string()));

    // Turn 3: Recall
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            assert!(messages.iter().any(|m| m.content.contains("verde")));
            Ok("Tu color es verde.".to_string())
        });

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );
    let planner = Planner::new();

    // Turn 1
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        planner.clone(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "Mi favorito es verde"))
    .await?;

    // Turn 2
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        planner.clone(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "Qué hora es?"))
    .await?;

    // Turn 3
    let res = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        planner,
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "Cuál era mi color?"))
    .await?;

    assert!(res.content.contains("verde"));

    Ok(())
}

/**
 * test_tool_context_exact_order_after_two_turns
 *
 * Sequence:
 * Turn 1: Normal message
 * Turn 2: TOOL call
 *
 * Check exact context order in the Tool-finalization step (Iter 2 of Turn 2).
 */
#[tokio::test]
async fn test_tool_context_exact_order_after_two_turns() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let mut seq = Sequence::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // Turn 1: Normal
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Hello user.".to_string()));

    // Turn 2: Tool Call
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Wait.\nTOOL:get_current_time".to_string()));

    // Validation step
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            // Expected context:
            // 0: User (Turn 1)
            // 1: Assistant (Turn 1)
            // 2: User (Turn 2)
            // 3: Tool (Turn 2)
            // NO "Wait." reasoning.
            assert_eq!(messages.len(), 4);
            assert_eq!(messages[0].content, "T1");
            assert_eq!(messages[1].content, "Hello user.");
            assert_eq!(messages[2].content, "T2");
            assert_eq!(messages[3].role, Role::Tool);

            let reasoning_leaked = messages.iter().any(|m| m.content.contains("Wait."));
            assert!(
                !reasoning_leaked,
                "Reasoning should not be in final Turn 2 context"
            );

            Ok("Final.".to_string())
        });

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );
    let planner = Planner::new();

    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        planner.clone(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "T1"))
    .await?;

    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        planner,
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "T2"))
    .await?;

    Ok(())
}

// --- Test Group C: Freshness & Pollution ---

/**
 * test_time_tool_not_reuses_previous_result
 *
 * Verifies that each tool call executes and gets fresh data,
 * even if a previous result is in history.
 */
#[tokio::test]
async fn test_time_tool_not_reuses_previous_result() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let mut seq = Sequence::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // Call 1
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("La hora es A.".to_string()));

    // Call 2
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("La hora es B.".to_string()));

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );
    let planner = Planner::new();

    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        planner.clone(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "Hora?"))
    .await?;

    let res = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        planner,
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "Hora otra vez?"))
    .await?;

    // The fact that it re-called the tool is validated by the sequence requiring mock expectations
    assert!(res.content.contains("hora es B"));

    Ok(())
}

/**
 * test_tool_result_only_once_in_context
 *
 * Ensures tool result isn't duplicated between memory fetch and active context append.
 */
#[tokio::test]
async fn test_tool_result_only_once_in_context() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let mut seq = Sequence::new();
    let mut mock_llm = MockRegressionMockProvider::new();

    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Thinking.\nTOOL:get_current_time".to_string()));

    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            let tool_count = messages.iter().filter(|m| m.role == Role::Tool).count();
            assert_eq!(
                tool_count, 1,
                "Tool result should appear exactly once in context"
            );
            Ok("12:00".to_string())
        });

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );
    let agent_loop = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    );

    agent_loop.run(Message::new(Role::User, "time")).await?;
    Ok(())
}

// --- Test Group D: Long Context Safety ---

/**
 * test_tool_after_long_context
 *
 * Simulate 15 message history.
 */
#[tokio::test]
async fn test_tool_after_long_context() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let memory = MemoryBridge::new(&db, "test_user");

    // Fill history with 15 messages
    for i in 0..15 {
        memory.save_message(&Message::new(Role::User, format!("msg {}", i)))?;
        memory.save_message(&Message::new(Role::Assistant, format!("ack {}", i)))?;
    }

    let mut mock_llm = MockRegressionMockProvider::new();
    let mut seq = Sequence::new();
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            // Planner::fetch_context(10) limits history in prompt
            assert!(messages.len() <= 21); // 10 pairs + current user msg
            Ok("TOOL:get_current_time".to_string())
        });
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("12:00".to_string()));

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );
    let agent_loop = AgentLoop::new(
        memory,
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    );

    let res = agent_loop.run(Message::new(Role::User, "Time?")).await?;
    assert_eq!(res.content, "12:00");

    Ok(())
}

// --- Legacy Consistency Tests (Updated for Observation 1 Re-hardening) ---

#[tokio::test]
async fn test_tool_split_flow_v2() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let mut seq = Sequence::new();

    let mut mock_llm = MockRegressionMockProvider::new();
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Reasoning.\nTOOL:get_current_time".to_string()));
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Final.".to_string()));

    let llm = LlmOrchestrator::new(
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    );
    let agent_loop = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    );

    agent_loop.run(Message::new(Role::User, "test")).await?;

    let memories = db.fetch_latest_memories("u", 10)?;
    // Should be: User, Tool, Assistant. Reasoning is skipped.
    assert_eq!(memories.len(), 3);
    assert_eq!(memories[0].role, Role::User);
    assert_eq!(memories[1].role, Role::Tool);
    assert_eq!(memories[2].role, Role::Assistant);

    Ok(())
}
