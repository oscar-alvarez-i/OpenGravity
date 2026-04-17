//! E2E Scenario Tests — Phase 2
//!
//! Single source of truth for end-to-end behavioral validation.
//! Each test maps 1:1 to documented scenarios in docs/testing/e2e-scenarios.md

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
use open_gravity::tools::local::{clear_notes_path, set_notes_path};
use open_gravity::tools::registry::Registry;
use std::fs;
use std::path::PathBuf;

mock! {
    pub RegressionMockProvider {}
    #[async_trait::async_trait]
    impl LlmProvider for RegressionMockProvider {
        async fn generate_response(&self, system: &str, messages: &[Message]) -> Result<String>;
    }
}

// =============================================================================
// Shared Test Helpers
// =============================================================================

fn unique_notes_path(test_name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_nanos();
    std::env::temp_dir().join(format!(
        "og_test_{}_{}_{}.txt",
        test_name,
        std::process::id(),
        nanos
    ))
}

fn read_notes_at(path: &PathBuf) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn user(msg: &str) -> Message {
    Message::new(Role::User, msg)
}

// =============================================================================
// Escenario 1 — Write + Read básico
// =============================================================================

#[tokio::test]
async fn escenario_1_write_read_basico() -> Result<()> {
    let db = Db::new(":memory:")?;
    let path = unique_notes_path("escenario_1");
    let _ = fs::remove_file(&path);
    set_notes_path(path.clone());

    let mut mock = MockRegressionMockProvider::new();
    mock.expect_generate_response()
        .returning(|_, _| Ok("TOOL:write_local_note:hola mundo".to_string()));
    mock.expect_generate_response()
        .returning(|_, _| Ok("Nota guardada.".to_string()));

    let llm = LlmOrchestrator::new(vec![Box::new(mock)]);
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let mut agent = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    );

    let res = agent.run(user("Guardá: hola mundo")).await?;
    assert!(!res.content.is_empty());

    let content = read_notes_at(&path).unwrap_or_default();
    assert!(
        content.contains("hola mundo"),
        "File should contain written note"
    );

    let _ = fs::remove_file(&path);
    clear_notes_path();
    Ok(())
}

// =============================================================================
// Escenario 2 — Escritura duplicada (idempotencia)
// =============================================================================

#[tokio::test]
async fn escenario_2_idempotencia() -> Result<()> {
    let db = Db::new(":memory:")?;
    let path = unique_notes_path("escenario_2");
    let _ = fs::remove_file(&path);
    set_notes_path(path.clone());

    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let mut mock1 = MockRegressionMockProvider::new();
    mock1
        .expect_generate_response()
        .returning(|_, _| Ok("TOOL:write_local_note:hola mundo".to_string()));
    mock1
        .expect_generate_response()
        .returning(|_, _| Ok("Nota guardada.".to_string()));

    let llm1 = LlmOrchestrator::new(vec![Box::new(mock1)]);

    let res1 = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm1, &registry, &skill_registry),
    )
    .run(user("Guardá: hola mundo"))
    .await?;
    assert!(!res1.content.is_empty());

    let content1 = read_notes_at(&path).unwrap_or_default();
    assert!(content1.contains("hola mundo"));

    let mut mock2 = MockRegressionMockProvider::new();
    mock2
        .expect_generate_response()
        .returning(|_, _| Ok("TOOL:write_local_note:hola mundo".to_string()));
    mock2
        .expect_generate_response()
        .returning(|_, _| Ok("Nota duplicada - ya existe.".to_string()));

    let llm2 = LlmOrchestrator::new(vec![Box::new(mock2)]);

    let res2 = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm2, &registry, &skill_registry),
    )
    .run(user("Guardá: hola mundo"))
    .await?;
    assert!(!res2.content.is_empty());

    let content2 = read_notes_at(&path).unwrap_or_default();
    let count = content2.matches("hola mundo").count();
    assert_eq!(count, 1, "Should have exactly one entry (idempotency)");

    let _ = fs::remove_file(&path);
    clear_notes_path();
    Ok(())
}

// =============================================================================
// Escenario 3 — Escritura con input distinto
// =============================================================================

#[tokio::test]
async fn escenario_3_inputs_distintos() -> Result<()> {
    let db = Db::new(":memory:")?;
    let path = unique_notes_path("escenario_3");
    let _ = fs::remove_file(&path);
    set_notes_path(path.clone());

    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let mut mock1 = MockRegressionMockProvider::new();
    mock1
        .expect_generate_response()
        .returning(|_, _| Ok("TOOL:write_local_note:hola mundo".to_string()));
    mock1
        .expect_generate_response()
        .returning(|_, _| Ok("Nota guardada.".to_string()));

    let llm1 = LlmOrchestrator::new(vec![Box::new(mock1)]);

    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm1, &registry, &skill_registry),
    )
    .run(user("Guardá: hola mundo"))
    .await?;

    let mut mock2 = MockRegressionMockProvider::new();
    mock2
        .expect_generate_response()
        .returning(|_, _| Ok("TOOL:write_local_note:hola".to_string()));
    mock2
        .expect_generate_response()
        .returning(|_, _| Ok("Nota guardada.".to_string()));

    let llm2 = LlmOrchestrator::new(vec![Box::new(mock2)]);

    let res2 = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm2, &registry, &skill_registry),
    )
    .run(user("Guardá: hola"))
    .await?;
    assert!(!res2.content.is_empty());

    let content = read_notes_at(&path).unwrap_or_default();
    assert!(content.contains("hola mundo"));
    assert!(content.contains("hola"));
    assert_eq!(
        content.matches("hola").count(),
        2,
        "Both inputs should be present"
    );

    let _ = fs::remove_file(&path);
    clear_notes_path();
    Ok(())
}

// =============================================================================
// Escenario 4 — Tool AlwaysFresh (hora)
// =============================================================================

#[tokio::test]
async fn escenario_4_alwaysfresh_time() -> Result<()> {
    let db = Db::new(":memory:")?;
    let path = unique_notes_path("escenario_4");
    let _ = fs::remove_file(&path);
    set_notes_path(path.clone());

    let mut mock = MockRegressionMockProvider::new();
    let mut seq = Sequence::new();

    mock.expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));
    mock.expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Son las 10:00.".to_string()));

    mock.expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));
    mock.expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("Son las 10:01.".to_string()));

    let llm = LlmOrchestrator::new(vec![Box::new(mock)]);
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let res1 = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(user("Qué hora es?"))
    .await?;
    assert!(res1.content.contains("10:00"));

    let res2 = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(user("Qué hora es?"))
    .await?;
    assert!(res2.content.contains("10:01") || res2.content != res1.content);

    let _ = fs::remove_file(&path);
    clear_notes_path();
    Ok(())
}

// =============================================================================
// Escenario 5 — Loop interno no duplica tool
// =============================================================================

#[tokio::test]
async fn escenario_5_no_duplicate_in_loop() -> Result<()> {
    let db = Db::new(":memory:")?;
    let path = unique_notes_path("escenario_5");
    let _ = fs::remove_file(&path);
    set_notes_path(path.clone());

    let mut mock = MockRegressionMockProvider::new();
    let mut seq = Sequence::new();

    mock.expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("TOOL:get_current_time".to_string()));

    mock.expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, messages| {
            let has_tool_result = messages
                .iter()
                .any(|m| m.role == Role::Tool && m.content.contains("get_current_time"));
            assert!(has_tool_result, "Tool result should be in context");
            Ok("La hora actual es 10:00.".to_string())
        });

    let llm = LlmOrchestrator::new(vec![Box::new(mock)]);
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let res = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(user("Qué hora es?"))
    .await?;

    assert!(res.content.contains("10:00"));

    let _ = fs::remove_file(&path);
    clear_notes_path();
    Ok(())
}

// =============================================================================
// Escenario 6 — Tool failure handling
// =============================================================================

#[tokio::test]
async fn escenario_6_tool_failure() -> Result<()> {
    let db = Db::new(":memory:")?;
    let path = unique_notes_path("escenario_6");
    let _ = fs::remove_file(&path);
    set_notes_path(path.clone());

    let mut mock = MockRegressionMockProvider::new();
    mock.expect_generate_response()
        .returning(|_, _| Ok("TOOL:write_local_note:".to_string()));

    let llm = LlmOrchestrator::new(vec![Box::new(mock)]);
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let res = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(user("Guardá: "))
    .await;

    match res {
        Ok(msg) => {
            let lower = msg.content.to_lowercase();
            assert!(lower.contains("error") || lower.contains("invalid") || lower.contains("vac"));
        }
        Err(e) => {
            let err_lower = e.to_string().to_lowercase();
            assert!(err_lower.contains("error") || err_lower.contains("invalid"));
        }
    }

    let _ = fs::remove_file(&path);
    clear_notes_path();
    Ok(())
}

// =============================================================================
// Escenario 7 — Regresión de contexto
// =============================================================================

#[tokio::test]
async fn escenario_7_regresion_contexto() -> Result<()> {
    let db = Db::new(":memory:")?;
    let path = unique_notes_path("escenario_7");
    let _ = fs::remove_file(&path);
    set_notes_path(path.clone());

    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let mut mock1 = MockRegressionMockProvider::new();
    mock1
        .expect_generate_response()
        .returning(|_, _| Ok("TOOL:write_local_note:hola mundo".to_string()));
    mock1
        .expect_generate_response()
        .returning(|_, _| Ok("Guardado.".to_string()));

    let llm1 = LlmOrchestrator::new(vec![Box::new(mock1)]);

    AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm1, &registry, &skill_registry),
    )
    .run(user("Guardá: hola mundo"))
    .await?;

    let mut mock2 = MockRegressionMockProvider::new();
    mock2
        .expect_generate_response()
        .returning(|_, _| Ok("TOOL:write_local_note:hola".to_string()));
    mock2
        .expect_generate_response()
        .returning(|_, _| Ok("Guardado.".to_string()));

    let llm2 = LlmOrchestrator::new(vec![Box::new(mock2)]);

    let res2 = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm2, &registry, &skill_registry),
    )
    .run(user("Guardá: hola"))
    .await?;

    assert!(!res2.content.to_lowercase().contains("no sé"));

    let content = read_notes_at(&path).unwrap_or_default();
    assert!(content.contains("hola mundo"));
    assert!(content.contains("hola"));

    let _ = fs::remove_file(&path);
    clear_notes_path();
    Ok(())
}

// =============================================================================
// Escenario 8 — Guardrail AlwaysFresh
// =============================================================================

#[tokio::test]
async fn escenario_8_guardrail_alwaysfresh() -> Result<()> {
    let db = Db::new(":memory:")?;
    let path = unique_notes_path("escenario_8");
    let _ = fs::remove_file(&path);
    set_notes_path(path.clone());

    let mut mock = MockRegressionMockProvider::new();
    let mut seq = Sequence::new();

    mock.expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("No tengo acceso al tiempo.".to_string()));
    mock.expect_generate_response()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok("La hora es las 12:00.".to_string()));

    let llm = LlmOrchestrator::new(vec![Box::new(mock)]);
    let registry = Registry::new();
    let skill_registry = SkillRegistry::new();

    let res = AgentLoop::new(
        MemoryBridge::new(&db, "u"),
        Planner::new(),
        Executor::new(&llm, &registry, &skill_registry),
    )
    .run(user("Qué hora es?"))
    .await?;

    assert!(!res.content.contains("tengo acceso") && !res.content.contains("No puedo"));

    let _ = fs::remove_file(&path);
    clear_notes_path();
    Ok(())
}
