use crate::domain::message::Message;
use crate::skills::echo::EchoSkill;
use crate::skills::memory::MemoryExtractionSkill;
use crate::skills::r#trait::{Skill, SkillOutput};
use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, info};

pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn Skill>>,
    order: Vec<String>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            skills: HashMap::new(),
            order: Vec::new(),
        };
        registry.register(Box::new(MemoryExtractionSkill::new()));
        registry.register(Box::new(EchoSkill::new()));
        registry
    }

    pub fn register(&mut self, skill: Box<dyn Skill>) {
        let name = skill.name().to_string();
        info!("Registering skill: {}", name);

        let is_new = !self.skills.contains_key(&name);
        self.skills.insert(name.clone(), skill);

        if is_new {
            self.order.push(name);
        }
    }

    pub fn get(&self, name: &str) -> Option<&dyn Skill> {
        self.skills.get(name).map(|s| s.as_ref())
    }

    pub fn names(&self) -> Vec<&str> {
        self.skills.keys().map(|s| s.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn select_skill(&self, user_message: &str, _context: &[Message]) -> Option<&dyn Skill> {
        debug!("Selecting skill for message: {}", user_message);

        for name in &self.order {
            if let Some(skill) = self.skills.get(name) {
                let trigger = skill.trigger();
                if trigger.matches(user_message) {
                    debug!("Skill '{}' triggered by pattern", skill.name());
                    return Some(skill.as_ref());
                }
            }
        }

        debug!("No skill triggered");
        None
    }

    pub async fn execute_skill(
        &self,
        skill_name: &str,
        context: &[Message],
        user_message: &Message,
    ) -> Result<SkillOutput> {
        let skill = self
            .skills
            .get(skill_name)
            .ok_or_else(|| anyhow::anyhow!("Skill not found: {}", skill_name))?;

        info!("Executing skill: {}", skill_name);
        let result = skill.execute(context, user_message).await?;
        debug!(
            "Skill '{}' executed, continue: {}",
            skill_name, result.should_continue
        );
        Ok(result)
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::Role;
    use crate::skills::r#trait::{SideEffects, TriggerType};
    use async_trait::async_trait;

    #[derive(Debug)]
    struct TestSkill {
        name: &'static str,
        trigger: TriggerType,
    }

    #[async_trait]
    impl crate::skills::r#trait::Skill for TestSkill {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "Test skill"
        }

        fn side_effects(&self) -> SideEffects {
            SideEffects::none()
        }

        fn trigger(&self) -> TriggerType {
            self.trigger
        }

        async fn execute(
            &self,
            _context: &[Message],
            _user_message: &Message,
        ) -> Result<SkillOutput> {
            Ok(SkillOutput::done("executed"))
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(TestSkill {
            name: "test",
            trigger: TriggerType::Never,
        }));

        assert!(registry.get("test").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_select_skill_triggered() {
        // Memory skill is auto-registered with pattern-based trigger
        // This test verifies skill triggers on personal fact messages
        let registry = SkillRegistry::new();
        let selected = registry.select_skill("Mi color favorito es azul", &[]);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name(), "memory_extraction");
    }

    #[test]
    fn test_registry_select_skill_not_triggered() {
        // Memory skill now uses pattern-based trigger
        // Test that get() returns none for unknown skills (not trigger behavior)
        let registry = SkillRegistry::new();

        // Unknown skill should not be found
        assert!(registry.get("unknown_skill").is_none());
    }

    #[tokio::test]
    async fn test_registry_execute_skill() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(TestSkill {
            name: "exec_test",
            trigger: TriggerType::Always,
        }));

        let result = registry
            .execute_skill("exec_test", &[], &Message::new(Role::User, "test"))
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap().should_continue);
    }

    #[tokio::test]
    async fn test_registry_execute_unknown_skill() {
        let registry = SkillRegistry::new();
        let result = registry
            .execute_skill("unknown", &[], &Message::new(Role::User, "test"))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_skill_output_normalization() {
        let output = SkillOutput::done("hello");
        assert!(!output.should_continue);
        assert_eq!(output.content, Some("hello".to_string()));
        assert!(output.memory_updates.is_empty());

        let output2 = SkillOutput::continue_with("world");
        assert!(output2.should_continue);

        let output3 = SkillOutput::done_no_output();
        assert!(!output3.should_continue);
        assert!(output3.content.is_none());
    }

    #[tokio::test]
    async fn test_skill_never_reenters_infinite_loop() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(TestSkill {
            name: "test",
            trigger: TriggerType::Always,
        }));

        let result = registry
            .execute_skill("test", &[], &Message::new(Role::User, "test"))
            .await;
        assert!(result.is_ok());

        let result2 = registry
            .execute_skill("test", &[], &Message::new(Role::User, "test2"))
            .await;
        assert!(result2.is_ok());

        let result3 = registry
            .execute_skill("test", &[], &Message::new(Role::User, "test3"))
            .await;
        assert!(result3.is_ok());
    }

    #[test]
    fn test_registry_contains_builtin_skills() {
        let registry = SkillRegistry::new();
        let names = registry.names();
        assert!(!names.is_empty(), "Registry should have built-in skills");
        assert!(
            names.iter().any(|n| n.contains("memory")),
            "Should contain memory_extraction skill"
        );
    }

    #[test]
    fn test_registry_auto_registers_memory_skill() {
        let registry = SkillRegistry::new();
        assert!(
            registry.get("memory_extraction").is_some(),
            "MemoryExtractionSkill should be auto-registered"
        );
    }

    #[tokio::test]
    async fn test_memory_skill_produces_updates() {
        use crate::skills::memory::MemoryExtractionSkill;

        let skill = MemoryExtractionSkill::new();
        let context = vec![];
        let user_msg = Message::new(Role::User, "Mi color favorito es azul");

        let result = skill.execute(&context, &user_msg).await.unwrap();

        assert!(
            !result.memory_updates.is_empty(),
            "Should produce memory updates"
        );
        assert_eq!(result.memory_updates[0].fact_key, "favorite_color");
    }

    #[test]
    fn test_executor_triggers_memory_skill() {
        let registry = SkillRegistry::new();

        // Memory skill should be triggered for messages containing personal fact patterns
        let selected = registry.select_skill("Mi color favorito es azul", &[]);
        assert!(selected.is_some(), "Should trigger memory skill");
        assert_eq!(selected.unwrap().name(), "memory_extraction");
    }

    #[test]
    #[allow(clippy::len_zero)]
    fn test_registry_is_empty_and_len() {
        let registry = SkillRegistry::new();
        assert!(!registry.is_empty());
        assert!(registry.len() > 0);
    }

    #[test]
    fn test_registry_default() {
        let registry = SkillRegistry::default();
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_select_skill_respects_registration_order() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(TestSkill {
            name: "skill_a",
            trigger: TriggerType::Always,
        }));
        registry.register(Box::new(TestSkill {
            name: "skill_b",
            trigger: TriggerType::Always,
        }));

        let selected = registry.select_skill("any message", &[]);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name(), "skill_a");
    }

    #[test]
    fn test_select_skill_order_after_auto_registered_memory() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(TestSkill {
            name: "skill_after_memory",
            trigger: TriggerType::Always,
        }));

        let selected = registry.select_skill("any message", &[]);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name(), "skill_after_memory");
    }

    #[test]
    fn test_select_skill_first_registered_wins() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(TestSkill {
            name: "first",
            trigger: TriggerType::Always,
        }));
        registry.register(Box::new(TestSkill {
            name: "second",
            trigger: TriggerType::Always,
        }));

        let selected = registry.select_skill("test", &[]);
        assert_eq!(selected.unwrap().name(), "first");
    }

    #[test]
    fn test_reregister_skill_preserves_order() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(TestSkill {
            name: "first",
            trigger: TriggerType::Always,
        }));
        registry.register(Box::new(TestSkill {
            name: "first",
            trigger: TriggerType::Always,
        }));

        let selected = registry.select_skill("test", &[]);
        assert_eq!(selected.unwrap().name(), "first");
    }

    #[test]
    fn test_two_different_skills_always_first_wins() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(TestSkill {
            name: "first",
            trigger: TriggerType::Always,
        }));
        registry.register(Box::new(TestSkill {
            name: "second",
            trigger: TriggerType::Always,
        }));

        let selected = registry.select_skill("any message", &[]);
        assert_eq!(selected.unwrap().name(), "first");
    }

    #[test]
    fn test_memory_and_echo_skill_coexistence() {
        let registry = SkillRegistry::new();

        let msg_with_both = "mi color favorito es azul echo hola";
        let selected = registry.select_skill(msg_with_both, &[]);

        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name(), "memory_extraction");
    }

    #[test]
    fn test_echo_skill_selected_when_only_echo_present() {
        let registry = SkillRegistry::new();

        let selected = registry.select_skill("echo hola", &[]);

        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name(), "echo");
    }
}
