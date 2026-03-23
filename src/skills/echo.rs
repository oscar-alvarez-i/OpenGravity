use crate::domain::message::Message;
use crate::skills::r#trait::{SideEffects, Skill, SkillOutput, TriggerType};
use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug)]
pub struct EchoSkill;

impl EchoSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EchoSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for EchoSkill {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echoes user message after removing 'echo' prefix"
    }

    fn side_effects(&self) -> SideEffects {
        SideEffects::none()
    }

    fn trigger(&self) -> TriggerType {
        TriggerType::OnPattern("echo")
    }

    async fn execute(&self, _context: &[Message], user_message: &Message) -> Result<SkillOutput> {
        let content = user_message.content.to_lowercase();
        let echo_removed = content
            .strip_prefix("echo")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if echo_removed.is_empty() {
            Ok(SkillOutput::done_no_output())
        } else {
            Ok(SkillOutput::done(echo_removed))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::Role;

    #[test]
    fn test_echo_trigger_matches() {
        let skill = EchoSkill::new();
        let trigger = skill.trigger();

        assert!(trigger.matches("echo hola"));
        assert!(trigger.matches("ECHO mundo"));
        assert!(trigger.matches("say echo test"));
    }

    #[test]
    fn test_echo_trigger_no_match() {
        let skill = EchoSkill::new();
        let trigger = skill.trigger();

        assert!(!trigger.matches("hello"));
        assert!(!trigger.matches(""));
    }

    #[tokio::test]
    async fn test_echo_removes_prefix() {
        let skill = EchoSkill::new();
        let user_msg = Message::new(Role::User, "echo hola");

        let result = skill.execute(&[], &user_msg).await.unwrap();

        assert!(result.content.is_some());
        assert_eq!(result.content.unwrap(), "hola");
    }

    #[tokio::test]
    async fn test_echo_empty_after_prefix() {
        let skill = EchoSkill::new();
        let user_msg = Message::new(Role::User, "echo");

        let result = skill.execute(&[], &user_msg).await.unwrap();

        assert!(result.content.is_none());
    }

    #[tokio::test]
    async fn test_echo_trims_whitespace() {
        let skill = EchoSkill::new();
        let user_msg = Message::new(Role::User, "echo   world  ");

        let result = skill.execute(&[], &user_msg).await.unwrap();

        assert_eq!(result.content.unwrap(), "world");
    }
}
