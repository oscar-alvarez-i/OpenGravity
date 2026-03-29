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
use open_gravity::skills::planner::Planner as SkillPlanner;
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

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);
    let memory = MemoryBridge::new(&db, "test_user");
    let planner = Planner::new();
    let executor = Executor::new(&llm, &registry, &skill_registry);
    let mut agent_loop = AgentLoop::new(memory, planner, executor);

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
 * Note: get_current_time is EXCLUDED from duplicate prevention (always fresh).
 * This test verifies the loop behavior with get_current_time.
 */
#[tokio::test]
async fn test_tool_repeated_same_tool_hits_safe_boundary() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // get_current_time is excluded from duplicate prevention
    // so it may loop until max_iterations
    mock_llm
        .expect_generate_response()
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);
    let memory = MemoryBridge::new(&db, "test_user");
    let planner = Planner::new();
    let executor = Executor::new(&llm, &registry, &skill_registry);
    let mut agent_loop = AgentLoop::new(memory, planner, executor);

    let result = agent_loop.run(Message::new(Role::User, "loop test")).await;

    // Either completes or hits max iterations - both are acceptable for get_current_time
    if let Err(e) = &result {
        assert!(
            e.to_string().contains("max iterations"),
            "Unexpected error: {}",
            e
        );
    }

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

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);
    let memory = MemoryBridge::new(&db, "test_user");
    let mut agent_loop = AgentLoop::new(
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

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);
    let mut agent_loop = AgentLoop::new(
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

    // Turn 1: Memory skill extracts fact, LLM still called to respond
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Entendido, azul es tu color favorito.".to_string()));

    // Turn 2: Recall from memory
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            let found = messages.iter().any(|m| m.content.contains("azul"));
            assert!(found, "Previous fact should be in context");
            Ok("Tu color favorito es azul.".to_string())
        });

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);

    // Run Turn 1
    {
        let memory = MemoryBridge::new(&db, "test_user");
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, planner.clone(), executor);
        agent_loop
            .run(Message::new(Role::User, "Mi color favorito es azul"))
            .await?;
    }

    // Run Turn 2
    {
        let memory = MemoryBridge::new(&db, "test_user");
        let executor = Executor::new(&llm, &registry, &skill_registry);
        let mut agent_loop = AgentLoop::new(memory, planner, executor);
        let res = agent_loop
            .run(Message::new(Role::User, "Cuál es mi color favorito?"))
            .await;
        assert!(res.is_ok());
        assert!(res.unwrap().content.contains("azul"));
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

    // Turn 1: Memory skill extracts fact, LLM still called
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Entendido, verde.".to_string()));

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

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);
    let planner = Planner::new();

    // Turn 1
    {
        let mut agent_loop = AgentLoop::new(
            MemoryBridge::new(&db, "u"),
            planner.clone(),
            Executor::new(&llm, &registry, &skill_registry),
        );
        agent_loop
            .run(Message::new(Role::User, "Mi favorito es verde"))
            .await?;
    }

    // Turn 2
    {
        let mut agent_loop = AgentLoop::new(
            MemoryBridge::new(&db, "u"),
            planner.clone(),
            Executor::new(&llm, &registry, &skill_registry),
        );
        agent_loop
            .run(Message::new(Role::User, "Qué hora es?"))
            .await?;
    }

    // Turn 3
    let mut agent_loop = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        planner,
        Executor::new(&llm, &registry, &skill_registry),
    );
    let res = agent_loop
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
            // Expected context (Phase 4 rule: tool exists, no assistant after → drop all assistants):
            // 0: User (Turn 1)
            // 1: User (Turn 2)
            // 2: Tool (Turn 2)
            // NO assistants (reasoning or turn responses).
            assert_eq!(messages.len(), 3);
            assert_eq!(messages[0].content, "T1");
            assert_eq!(messages[1].content, "T2");
            assert_eq!(messages[2].role, Role::Tool);

            let reasoning_leaked = messages.iter().any(|m| m.content.contains("Wait."));
            assert!(
                !reasoning_leaked,
                "Reasoning should not be in final Turn 2 context"
            );

            Ok("Final.".to_string())
        });

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);
    let planner = Planner::new();
    let _skill_planner = SkillPlanner::new();

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

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);
    let planner = Planner::new();
    let _skill_planner = SkillPlanner::new();

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

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);
    let _skill_planner = SkillPlanner::new();
    let mut agent_loop = AgentLoop::new(
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

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);
    let _skill_planner = SkillPlanner::new();
    let mut agent_loop = AgentLoop::new(
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

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);
    let _skill_planner = SkillPlanner::new();
    let mut agent_loop = AgentLoop::new(
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

// --- Stress Probe: Memory overwrite + tool + recall ---

/**
 * probe_memory_overwrite_tool_recall
 *
 * Scenario:
 * Turn 1: Memory update (color=verde)
 * Turn 2: Memory update (color=azul), then tool call
 * Turn 3: Recall - should only see azul, not verde
 */
#[tokio::test]
async fn probe_memory_overwrite_tool_recall() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let mut mock_llm = MockRegressionMockProvider::new();
    let mut seq = Sequence::new();

    // Turn 1: Memory update verde
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("OK".to_string()));

    // Turn 2: Memory update azul + tool
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Done".to_string()));

    // Turn 3: Recall - check context
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            // Check that only latest memory is present (SET or UPDATE)
            let memories: Vec<_> = messages
                .iter()
                .filter(|m| {
                    m.content.contains("MEMORY_SET:") || m.content.contains("MEMORY_UPDATE:")
                })
                .collect();
            // Should have exactly 1 memory (latest)
            assert_eq!(
                memories.len(),
                1,
                "Should have exactly 1 memory, got {:?}",
                memories.len()
            );
            // Must contain azul
            assert!(
                memories.iter().any(|m| m.content.contains("azul")),
                "Latest memory should contain azul"
            );
            // Must NOT contain verde
            assert!(
                !memories.iter().any(|m| m.content.contains("verde")),
                "Stale memory verde should be excluded"
            );
            Ok("Final".to_string())
        });

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);

    // Turn 1
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "Mi color favorito es verde"))
    .await?;

    // Turn 2
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(
        Role::User,
        "Mi color favorito es azul y también dime la hora",
    ))
    .await?;

    // Turn 3
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "Cual es mi color favorito?"))
    .await?;

    Ok(())
}

// --- Stress Probe: Multiple assistant/tool alternation ---

/**
 * probe_multiple_assistant_tool_alternation
 *
 * Scenario:
 * Three turns in same conversation with tool calls
 * Context should compact to latest assistant block only
 */
#[tokio::test]
async fn probe_multiple_assistant_tool_alternation() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let mut mock_llm = MockRegressionMockProvider::new();
    let mut seq = Sequence::new();

    // Turn 1
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Answer1".to_string()));

    // Turn 2
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("TOOL:get_weather".to_string()));
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            // Turn 2 ends with tool, so no assistant block
            // Stale "Answer1" should be filtered out
            let assistants: Vec<_> = messages
                .iter()
                .filter(|m| m.role == Role::Assistant)
                .collect();
            // Should have 0 assistants (tool-ending turn has no assistant block)
            assert_eq!(
                assistants.len(),
                0,
                "Tool-ending context should have 0 assistants, got {:?}",
                assistants.len()
            );
            Ok("Answer2".to_string())
        });

    // Turn 3
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            // Turn 3 ends with tool, so no assistant block
            let assistants: Vec<_> = messages
                .iter()
                .filter(|m| m.role == Role::Assistant)
                .collect();
            // Should have 0 assistants (tool-ending)
            assert_eq!(
                assistants.len(),
                0,
                "Tool-ending context should have 0 assistants, got {:?}",
                assistants.len()
            );
            Ok("Final".to_string())
        });

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);

    // Turn 1
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "query"))
    .await?;

    // Turn 2
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "weather check"))
    .await?;

    // Turn 3
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "something else"))
    .await?;

    Ok(())
}

// --- Stress Probe: Memory update inside same turn + follow-up recall ---

/**
 * probe_memory_update_same_turn_followup_recall
 *
 * Scenario:
 * Turn 1: Memory update + tool in same message
 * Turn 2: Recall - structural validation of assistant block
 */
#[tokio::test]
async fn probe_memory_update_same_turn_followup_recall() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let mut mock_llm = MockRegressionMockProvider::new();
    let mut seq = Sequence::new();

    // Turn 1: Memory update + tool call
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Final answer".to_string()));

    // Turn 2: Recall - structural validation
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            // Check assistant block structure after tool-ending Turn 1
            // No assistants should appear after tool result
            let mut seen_tool = false;
            for msg in messages.iter() {
                if msg.role == Role::Tool {
                    seen_tool = true;
                } else if seen_tool && msg.role == Role::Assistant {
                    panic!("Should not have assistant after tool in context");
                }
            }
            Ok("Recalled".to_string())
        });

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);

    // Turn 1: memory + tool
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(
        Role::User,
        "Mi occupation is engineer and what's the time?",
    ))
    .await?;

    // Turn 2: recall
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "What is my occupation?"))
    .await?;

    Ok(())
}

// --- Phase 14: New Functional Tests ---

/**
 * test_memory_delete_end_to_end
 *
 * Validates MEMORY_DELETE operation works across turns.
 * Verifies both prompt-level and DB-level deletion.
 */
#[tokio::test]
async fn test_memory_delete_end_to_end() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let mut seq = Sequence::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // Turn 1: SET memory
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("OK".to_string()));

    // Turn 2: DELETE via assistant directive
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("MEMORY_DELETE:temporary_data".to_string()));

    // Turn 3: Verify deletion - memory should not appear as SET/UPDATE in prompt
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            let has_set_or_update = messages.iter().any(|m| {
                m.content.contains("MEMORY_SET:temporary_data")
                    || m.content.contains("MEMORY_UPDATE:temporary_data")
            });
            assert!(
                !has_set_or_update,
                "Deleted memory should not appear as SET/UPDATE in prompt"
            );
            Ok("Done".to_string())
        });

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);

    // Turn 1: SET
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(
        Role::User,
        "remember temporary_data=test_value",
    ))
    .await?;

    // Verify SET worked via fetch + filter
    let memories_after_set = db.fetch_latest_memories("u", 20)?;
    let has_memory_after_set = memories_after_set
        .iter()
        .any(|m| m.content.contains("temporary_data"));
    assert!(has_memory_after_set, "Memory should be set after Turn 1");

    // Turn 2: DELETE
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "forget temporary_data"))
    .await?;

    // Verify DELETE marker exists in DB
    let memories_after_delete = db.fetch_latest_memories("u", 20)?;
    let has_delete_marker = memories_after_delete
        .iter()
        .any(|m| m.content.contains("MEMORY_DELETE:temporary_data"));
    assert!(
        has_delete_marker,
        "DELETE marker should exist in DB after Turn 2"
    );

    // Turn 3: Verify prompt does not contain deleted memory
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "what was temporary_data?"))
    .await?;

    Ok(())
}

/**
 * test_echo_skill_in_conversation
 *
 * Validates echo skill triggers correctly in multi-turn conversation.
 */
#[tokio::test]
async fn test_echo_skill_in_conversation() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // Echo skill returns directly - no LLM call expected
    mock_llm.expect_generate_response().times(0);

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);

    // Echo skill triggers on "echo" pattern
    let mut agent_loop = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    );

    let res = agent_loop
        .run(Message::new(Role::User, "echo hello world"))
        .await?;

    // Echo skill strips "echo" prefix and returns remaining content
    let content_lower = res.content.to_lowercase();
    assert!(
        content_lower.contains("hello") && content_lower.contains("world"),
        "Echo should return stripped content, got: {}",
        res.content
    );

    Ok(())
}

/**
 * test_memory_overwrite_pending_plan_fresh_tool
 *
 * Validates: memory overwrite + AlwaysFresh tool execution.
 * Verifies DB final state and tool execution actually occurred.
 */
#[tokio::test]
async fn test_memory_overwrite_pending_plan_fresh_tool() -> Result<()> {
    let db = Db::new(":memory:")?;
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();
    let mut seq = Sequence::new();

    let mut mock_llm = MockRegressionMockProvider::new();

    // Turn 1: Memory SET azul
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Remembered azul".to_string()));

    // Turn 2: Memory overwrite verde + tool call (AlwaysFresh bypasses duplicate prevention)
    // First LLM call: returns TOOL directive
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));

    // Second LLM call: tool executed, returns final response with time
    mock_llm
        .expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            // Verify tool result is in context - confirms tool branch executed
            let tool_result = messages.iter().find(|m| m.role == Role::Tool);
            assert!(tool_result.is_some(), "Tool result should be in context");
            assert!(
                !tool_result.unwrap().content.is_empty(),
                "Tool result should have content"
            );
            Ok("Time is 10:00".to_string())
        });

    let llm = LlmOrchestrator::new(vec![
        Box::new(mock_llm),
        Box::new(MockRegressionMockProvider::new()),
    ]);

    // Turn 1: SET memory
    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(Role::User, "Mi color favorito es azul"))
    .await?;

    // Verify SET worked via fetch + filter
    let memories_after_set = db.fetch_latest_memories("u", 20)?;
    let has_azul = memories_after_set
        .iter()
        .any(|m| m.content.contains("favorite_color") && m.content.contains("azul"));
    assert!(has_azul, "Memory azul should be set after Turn 1");

    // Turn 2: Overwrite + tool call
    let res = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(Message::new(
        Role::User,
        "Mi color favorito es verde y después dime la hora",
    ))
    .await?;

    // Verify: Tool execution actually happened (response contains time info)
    assert!(
        res.content.contains("10:00") || res.content.contains("Time"),
        "Tool should have executed and returned time, got: {}",
        res.content
    );

    // Verify: DB final state - at least one memory with latest value
    let memories = db.fetch_latest_memories("u", 10)?;
    let has_verde_memory = memories
        .iter()
        .any(|m| m.content.contains("favorite_color") && m.content.contains("verde"));
    assert!(
        has_verde_memory,
        "Should have memory with verde, got memories: {:?}",
        memories
            .iter()
            .filter(|m| m.content.contains("favorite_color"))
            .collect::<Vec<_>>()
    );

    Ok(())
}
