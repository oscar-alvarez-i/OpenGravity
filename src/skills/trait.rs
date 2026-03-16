use crate::domain::message::Message;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Skill: Send + Sync {
    /// Unique identifier for the skill.
    fn name(&self) -> &str;

    /// Brief description of what the skill does.
    fn description(&self) -> &str;

    /// Execute the skill logic given the current context.
    /// This is a placeholder for future Phase 2 implementation.
    async fn execute(&self, messages: &[Message]) -> Result<String>;
}
