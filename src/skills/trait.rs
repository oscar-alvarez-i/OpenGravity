use crate::domain::message::Message;
use anyhow::Result;
use async_trait::async_trait;
use std::fmt::Debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SideEffects {
    pub reads_context: bool,
    pub writes_memory: bool,
    pub emits_user_output: bool,
    pub internal_only: bool,
}

impl SideEffects {
    pub const fn none() -> Self {
        Self {
            reads_context: false,
            writes_memory: false,
            emits_user_output: false,
            internal_only: true,
        }
    }

    pub const fn reads() -> Self {
        Self {
            reads_context: true,
            writes_memory: false,
            emits_user_output: false,
            internal_only: true,
        }
    }

    pub const fn writes() -> Self {
        Self {
            reads_context: false,
            writes_memory: true,
            emits_user_output: false,
            internal_only: true,
        }
    }

    pub const fn reads_writes() -> Self {
        Self {
            reads_context: true,
            writes_memory: true,
            emits_user_output: false,
            internal_only: true,
        }
    }

    pub const fn reads_writes_output() -> Self {
        Self {
            reads_context: true,
            writes_memory: true,
            emits_user_output: true,
            internal_only: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    Always,
    Never,
    OnPattern(&'static str),
    OnAnyPattern(&'static [&'static str]),
}

impl TriggerType {
    pub fn matches(&self, message: &str) -> bool {
        let message_lower = message.to_lowercase();
        match self {
            TriggerType::Always => true,
            TriggerType::Never => false,
            TriggerType::OnPattern(pattern) => message_lower.contains(&pattern.to_lowercase()),
            TriggerType::OnAnyPattern(patterns) => patterns
                .iter()
                .any(|p| message_lower.contains(&p.to_lowercase())),
        }
    }
}

#[async_trait]
pub trait Skill: Send + Sync + Debug {
    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn side_effects(&self) -> SideEffects;

    fn trigger(&self) -> TriggerType;

    async fn execute(&self, context: &[Message], user_message: &Message) -> Result<SkillOutput>;
}

#[derive(Debug, Clone)]
pub struct SkillOutput {
    pub content: Option<String>,
    pub should_continue: bool,
    pub memory_updates: Vec<MemoryUpdate>,
}

impl SkillOutput {
    pub fn continue_with(content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
            should_continue: true,
            memory_updates: Vec::new(),
        }
    }

    pub fn done(content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
            should_continue: false,
            memory_updates: Vec::new(),
        }
    }

    pub fn done_no_output() -> Self {
        Self {
            content: None,
            should_continue: false,
            memory_updates: Vec::new(),
        }
    }

    pub fn with_memory_updates(mut self, updates: Vec<MemoryUpdate>) -> Self {
        self.memory_updates = updates;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryUpdate {
    pub fact_key: String,
    pub fact_value: String,
    pub operation: MemoryOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryOperation {
    Set,
    Update,
    Delete,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_type_always() {
        let trigger = TriggerType::Always;
        assert!(trigger.matches("anything"));
    }

    #[test]
    fn test_trigger_type_never() {
        let trigger = TriggerType::Never;
        assert!(!trigger.matches("anything"));
    }
}
