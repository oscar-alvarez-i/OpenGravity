use crate::domain::message::Message;
use crate::skills::r#trait::{
    MemoryOperation, MemoryUpdate, SideEffects, Skill, SkillOutput, TriggerType,
};
use anyhow::Result;
use async_trait::async_trait;
use tracing::{debug, info};

#[derive(Debug)]
pub struct MemoryExtractionSkill {
    stable_fact_patterns: Vec<FactPattern>,
}

#[derive(Clone, Debug)]
struct FactPattern {
    key: &'static str,
    patterns: Vec<&'static str>,
    value_patterns: Vec<&'static str>,
}

impl FactPattern {
    fn matches(&self, text: &str) -> Option<usize> {
        let text_lower = text.to_lowercase();
        for pattern in &self.patterns {
            if let Some(pos) = text_lower.find(pattern) {
                return Some(pos);
            }
        }
        None
    }

    fn extract_value(&self, text: &str, pattern_pos: usize) -> Option<String> {
        let text_lower = text.to_lowercase();

        for vp in &self.value_patterns {
            if let Some(vp_pos) = text_lower[pattern_pos..].find(vp) {
                let actual_pos = pattern_pos + vp_pos;
                let after = &text[actual_pos + vp.len()..];
                let value = after.trim();
                if value.starts_with('"') || value.starts_with('\'') {
                    if let Some(end_quote) = value[1..].find(&value[0..1]) {
                        return Some(value[1..=end_quote].to_string());
                    }
                }
                if let Some(first_word) = value.split_whitespace().next() {
                    if !first_word.is_empty() {
                        return Some(first_word.to_string());
                    }
                }
            }
        }

        None
    }
}

impl MemoryExtractionSkill {
    pub fn new() -> Self {
        Self {
            stable_fact_patterns: vec![
                FactPattern {
                    key: "favorite_color",
                    patterns: vec!["mi color favorito", "mi color preferido", "favorite color"],
                    value_patterns: vec!["es ", "is "],
                },
                FactPattern {
                    key: "occupation",
                    patterns: vec![
                        "trabajo con",
                        "trabajo en",
                        "mi trabajo",
                        "me dedico a",
                        "occupation",
                        "i work as",
                        "i work with",
                    ],
                    value_patterns: vec![
                        "trabajo con ",
                        "trabajo en ",
                        "me dedico a ",
                        "work with ",
                        "work as ",
                    ],
                },
                FactPattern {
                    key: "location",
                    patterns: vec!["vivo en", "mi ciudad", "located in", "i live in"],
                    value_patterns: vec!["vivo en ", "located in ", "i live in "],
                },
                FactPattern {
                    key: "language",
                    patterns: vec!["hablo", "mi idioma", "i speak", "my language"],
                    value_patterns: vec!["hablo ", "i speak "],
                },
                FactPattern {
                    key: "name",
                    patterns: vec!["me llamo", "mi nombre es", "i am called", "my name is"],
                    value_patterns: vec![
                        "me llamo ",
                        "mi nombre es ",
                        "i am called ",
                        "my name is ",
                    ],
                },
                FactPattern {
                    key: "profession",
                    patterns: vec![
                        "soy desarrollador",
                        "soy ingeniero",
                        "i am a developer",
                        "i am an engineer",
                    ],
                    value_patterns: vec!["soy ", "i am a ", "i am an "],
                },
            ],
        }
    }

    fn extract_fact(&self, text: &str) -> Option<(String, String)> {
        for pattern in &self.stable_fact_patterns {
            if let Some(pattern_pos) = pattern.matches(text) {
                if let Some(value) = pattern.extract_value(text, pattern_pos) {
                    let key = pattern.key.to_string();
                    return Some((key, value));
                }
            }
        }
        None
    }

    fn is_transient_fact(&self, text: &str) -> bool {
        let transient_indicators = vec![
            "hoy ",
            "ahora ",
            "en este momento",
            "this moment",
            "tomé",
            "estoy tomando",
            "i took",
            "i am having",
            "que hora",
            "what time",
            "hora actual",
            "la hora",
            "el clima",
            "the weather",
            "how are you",
            "what are you doing",
        ];

        let text_lower = text.to_lowercase();
        for indicator in &transient_indicators {
            if text_lower.contains(indicator) {
                debug!("Detected transient indicator '{}' in message", indicator);
                return true;
            }
        }
        false
    }

    fn find_existing_fact(
        &self,
        key: &str,
        context: &[Message],
        current_text: &str,
    ) -> Option<String> {
        let current_lower = current_text.to_lowercase();
        for msg in context {
            if msg.role == crate::domain::message::Role::User {
                let msg_lower = msg.content.to_lowercase();
                if msg_lower.starts_with(&current_lower) && msg_lower != current_lower {
                    continue;
                }
                if let Some((fact_key, fact_value)) = self.extract_fact(&msg.content) {
                    if fact_key == key {
                        return Some(fact_value);
                    }
                }
            }
        }
        None
    }

    fn is_fact_update(
        &self,
        key: &str,
        new_value: &str,
        context: &[Message],
        current_text: &str,
    ) -> bool {
        if let Some(existing) = self.find_existing_fact(key, context, current_text) {
            if existing.to_lowercase() != new_value.to_lowercase() {
                debug!(
                    "Fact '{}' update detected: '{}' -> '{}'",
                    key, existing, new_value
                );
                return true;
            }
        }
        false
    }
}

impl Default for MemoryExtractionSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for MemoryExtractionSkill {
    fn name(&self) -> &str {
        "memory_extraction"
    }

    fn description(&self) -> &str {
        "Extracts and persists stable user facts from conversation"
    }

    fn side_effects(&self) -> SideEffects {
        SideEffects::reads_writes()
    }

    fn trigger(&self) -> TriggerType {
        TriggerType::OnAnyPattern(&["mi ", "trabajo", "vivo ", "soy ", "me gusta"])
    }

    async fn execute(&self, context: &[Message], user_message: &Message) -> Result<SkillOutput> {
        info!("Executing memory_extraction_skill");

        let text = &user_message.content;

        if self.is_transient_fact(text) {
            debug!("Ignoring transient fact: {}", text);
            return Ok(SkillOutput::done_no_output());
        }

        if let Some((key, value)) = self.extract_fact(text) {
            debug!("Extracted stable fact: key={}, value={}", key, value);

            // Check for update vs duplicate using normalized values
            if self.is_fact_update(&key, &value, context, text) {
                info!("Updating existing fact: {}", key);
                return Ok(
                    SkillOutput::done_no_output().with_memory_updates(vec![MemoryUpdate {
                        fact_key: key,
                        fact_value: value,
                        operation: MemoryOperation::Update,
                    }]),
                );
            }

            // Check if this exact fact already exists (no duplicate) using normalized values
            if self
                .find_existing_fact(&key, context, text)
                .map(|existing| existing.to_lowercase() == value.to_lowercase())
                .unwrap_or(false)
            {
                debug!("Existing fact found for key='{}'", key);
                return Ok(SkillOutput::done_no_output());
            }

            info!("Persisting new stable fact: {}", key);
            return Ok(
                SkillOutput::done_no_output().with_memory_updates(vec![MemoryUpdate {
                    fact_key: key,
                    fact_value: value,
                    operation: MemoryOperation::Set,
                }]),
            );
        }

        debug!("No extractable fact found in message");
        Ok(SkillOutput::done_no_output())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::Role;

    fn create_context(messages: Vec<(&str, Role)>) -> Vec<Message> {
        messages
            .into_iter()
            .map(|(content, role)| Message::new(role, content))
            .collect()
    }

    #[test]
    fn test_extract_stable_fact_color() {
        let skill = MemoryExtractionSkill::new();
        let result = skill.extract_fact("Mi color favorito es azul");
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key, "favorite_color");
        // Should extract normalized value "azul" not full sentence
        assert_eq!(value, "azul");
    }

    #[test]
    fn test_extract_stable_fact_occupation() {
        let skill = MemoryExtractionSkill::new();
        let result = skill.extract_fact("Trabajo con Python ahora");
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key, "occupation");
        // Should extract normalized value "Python" not full sentence
        assert_eq!(value, "Python");
    }

    #[test]
    fn test_normalize_favorite_color_value() {
        let skill = MemoryExtractionSkill::new();
        let result = skill.extract_fact("Mi color favorito es verde");
        assert!(result.is_some());
        let (_key, value) = result.unwrap();
        assert_eq!(value, "verde", "Should extract 'verde' not full sentence");
    }

    #[test]
    fn test_normalize_occupation_value() {
        let skill = MemoryExtractionSkill::new();
        let result = skill.extract_fact("Trabajo con Rust");
        assert!(result.is_some());
        let (_key, value) = result.unwrap();
        assert_eq!(value, "Rust", "Should extract 'Rust' not full sentence");
    }

    #[test]
    fn test_ignore_transient_coffee() {
        let skill = MemoryExtractionSkill::new();
        assert!(skill.is_transient_fact("Hoy tomé café"));
    }

    #[test]
    fn test_ignore_transient_time() {
        let skill = MemoryExtractionSkill::new();
        assert!(skill.is_transient_fact("Estoy mirando la hora"));
    }

    #[test]
    fn test_ignore_irrelevant() {
        let skill = MemoryExtractionSkill::new();
        assert!(!skill.is_transient_fact("Hola como estas"));
        assert!(skill.extract_fact("Hola como estas").is_none());
    }

    #[tokio::test]
    async fn test_memory_extracts_stable_fact_only() {
        let skill = MemoryExtractionSkill::new();
        let context = create_context(vec![]);
        let user_msg = Message::new(Role::User, "Mi color favorito es azul");

        let result = skill.execute(&context, &user_msg).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.memory_updates.len(), 1);
        assert_eq!(result.memory_updates[0].fact_key, "favorite_color");
        assert_eq!(result.memory_updates[0].operation, MemoryOperation::Set);
    }

    #[tokio::test]
    async fn test_memory_ignores_transient_fact() {
        let skill = MemoryExtractionSkill::new();
        let context = create_context(vec![]);
        let user_msg = Message::new(Role::User, "Hoy tomé café");

        let result = skill.execute(&context, &user_msg).await.unwrap();

        assert!(!result.should_continue);
        assert!(result.memory_updates.is_empty());
        assert!(result.content.is_none());
    }

    #[tokio::test]
    async fn test_memory_no_duplicate_fact() {
        let skill = MemoryExtractionSkill::new();
        let context = create_context(vec![("Mi color favorito es azul", Role::User)]);
        let user_msg = Message::new(Role::User, "Mi color favorito es azul");

        let result = skill.execute(&context, &user_msg).await.unwrap();

        assert!(result.memory_updates.is_empty());
    }

    #[tokio::test]
    async fn test_memory_update_existing_fact() {
        let skill = MemoryExtractionSkill::new();
        let context = create_context(vec![("Mi color favorito es azul", Role::User)]);
        let user_msg = Message::new(Role::User, "Mi color favorito es verde");

        let result = skill.execute(&context, &user_msg).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.memory_updates.len(), 1);
        assert_eq!(result.memory_updates[0].fact_key, "favorite_color");
        assert_eq!(result.memory_updates[0].operation, MemoryOperation::Update);
    }

    #[test]
    fn test_trigger_only_on_personal_facts() {
        let skill = MemoryExtractionSkill::new();
        let trigger = skill.trigger();

        assert!(
            trigger.matches("Mi color favorito es azul"),
            "Should trigger on 'mi'"
        );
        assert!(
            trigger.matches("mi nombre es Juan"),
            "Should trigger on lowercase 'mi'"
        );
        assert!(
            trigger.matches("Trabajo con Python"),
            "Should trigger on 'Trabajo'"
        );
        assert!(
            trigger.matches("Vivo en Madrid"),
            "Should trigger on 'Vivo'"
        );
        assert!(
            trigger.matches("Soy desarrollador"),
            "Should trigger on 'Soy'"
        );
        assert!(
            trigger.matches("Me gusta el azul"),
            "Should trigger on 'Me gusta'"
        );

        assert!(
            !trigger.matches("¿Qué hora es?"),
            "Should NOT trigger on question"
        );
        assert!(
            !trigger.matches("Hola cómo estás"),
            "Should NOT trigger on greeting"
        );
        assert!(
            !trigger.matches("What time is it?"),
            "Should NOT trigger on English question"
        );
    }

    #[tokio::test]
    async fn test_clean_db_first_fact_produces_insert() {
        let skill = MemoryExtractionSkill::new();
        let context = create_context(vec![]);
        let user_msg = Message::new(Role::User, "Mi color favorito es azul");

        let result = skill.execute(&context, &user_msg).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.memory_updates.len(), 1);
        assert_eq!(result.memory_updates[0].fact_key, "favorite_color");
        assert_eq!(result.memory_updates[0].fact_value, "azul");
        assert_eq!(result.memory_updates[0].operation, MemoryOperation::Set);
    }

    #[tokio::test]
    async fn test_fact_fragment_with_full_message_in_context() {
        let skill = MemoryExtractionSkill::new();
        let context = create_context(vec![(
            "Mi color favorito es verde y después decime la hora",
            Role::User,
        )]);
        let user_msg = Message::new(Role::User, "Mi color favorito es verde");

        let result = skill.execute(&context, &user_msg).await.unwrap();

        assert!(!result.should_continue);
        assert_eq!(
            result.memory_updates.len(),
            1,
            "Should extract fact from fragment, not find duplicate"
        );
        assert_eq!(result.memory_updates[0].fact_key, "favorite_color");
        assert_eq!(result.memory_updates[0].fact_value, "verde");
    }
}
