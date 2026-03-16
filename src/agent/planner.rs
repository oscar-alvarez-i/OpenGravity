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
