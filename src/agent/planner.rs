use crate::domain::message::Message;

const SYSTEM_PROMPT_TEMPLATE: &str = include_str!("../prompts/system.md");
const IDENTITY_RULES: &str = include_str!("../prompts/identity_rules.md");
const TOOL_RULES: &str = include_str!("../prompts/tool_rules.md");
const MEMORY_RULES: &str = include_str!("../prompts/memory_rules.md");

#[derive(Clone)]
pub struct Planner;

impl Planner {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }

    pub fn build_system_prompt(&self) -> String {
        SYSTEM_PROMPT_TEMPLATE
            .replace("{identity_rules}", IDENTITY_RULES.trim())
            .replace("{tool_rules}", TOOL_RULES.trim())
            .replace("{memory_rules}", MEMORY_RULES.trim())
    }

    pub fn assemble_messages(
        &self,
        context: &[Message],
        latest_user_msg: &Message,
    ) -> Vec<Message> {
        let mut messages = context.to_vec();
        messages.push(latest_user_msg.clone());
        messages
    }

    pub fn filter_tool_duplicates(&self, messages: &[Message]) -> Vec<Message> {
        use std::collections::HashMap;
        let mut last_tool_idx: HashMap<String, usize> = HashMap::new();

        for (i, msg) in messages.iter().enumerate().rev() {
            if msg.role == crate::domain::message::Role::Tool {
                if let Some(tool_name) = self.extract_tool_name_from_result(&msg.content) {
                    last_tool_idx.entry(tool_name).or_insert(i);
                }
            }
        }

        let keep_indices: std::collections::HashSet<usize> = last_tool_idx.into_values().collect();

        messages
            .iter()
            .enumerate()
            .filter(|(i, _)| keep_indices.contains(i))
            .map(|(_, m)| m.clone())
            .collect()
    }

    fn extract_tool_name_from_result(&self, content: &str) -> Option<String> {
        if content.contains("get_current_time") {
            Some("get_current_time".to_string())
        } else if content.contains("get_weather") {
            Some("get_weather".to_string())
        } else if content.contains("get_date") {
            Some("get_date".to_string())
        } else if content.contains("Tool result available:") {
            Some("unknown".to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_build() {
        let planner = Planner::new();
        let sys = planner.build_system_prompt();
        assert!(sys.contains("TOOL:"));
    }

    #[test]
    fn test_planner_assemble_messages() {
        let planner = Planner::new();
        let context = vec![
            Message::new(crate::domain::message::Role::User, "Hello"),
            Message::new(crate::domain::message::Role::Assistant, "Hi"),
        ];
        let latest = Message::new(crate::domain::message::Role::User, "How are you?");

        let assembled = planner.assemble_messages(&context, &latest);
        assert_eq!(assembled.len(), 3);
        assert_eq!(assembled[0].content, "Hello");
        assert_eq!(assembled[2].content, "How are you?");
    }
}
