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

        let tool_indices_to_keep: std::collections::HashSet<usize> =
            last_tool_idx.into_values().collect();

        messages
            .iter()
            .enumerate()
            .filter(|(i, msg)| {
                if msg.role != crate::domain::message::Role::Tool {
                    true
                } else {
                    tool_indices_to_keep.contains(i)
                }
            })
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

    pub fn compact_context(&self, messages: &[Message]) -> Vec<Message> {
        use crate::domain::message::Role;

        fn is_block_message(msg: &Message) -> bool {
            msg.role == Role::Assistant
                || (msg.role == Role::System && msg.content.starts_with("MEMORY_UPDATE:"))
        }

        let last_is_block = messages.last().map(is_block_message).unwrap_or(false);

        let block_start_idx = if last_is_block {
            let mut start_idx = messages.len();
            for (i, msg) in messages.iter().enumerate().rev() {
                if is_block_message(msg) {
                    start_idx = i;
                } else {
                    break;
                }
            }
            Some(start_idx)
        } else {
            None
        };

        messages
            .iter()
            .enumerate()
            .filter(|(i, msg)| {
                if msg.role == Role::Assistant {
                    block_start_idx.is_some_and(|start| *i >= start)
                } else {
                    true
                }
            })
            .map(|(_, m)| m.clone())
            .collect()
    }

    pub fn compact_memory_updates(&self, messages: &[Message]) -> Vec<Message> {
        use crate::domain::message::Role;
        use std::collections::HashMap;

        let mut last_update_per_key: HashMap<String, (usize, Message)> = HashMap::new();

        for (i, msg) in messages.iter().enumerate() {
            if msg.role == Role::System && msg.content.starts_with("MEMORY_UPDATE:") {
                if let Some(key) = self.extract_memory_key(&msg.content) {
                    last_update_per_key.insert(key, (i, msg.clone()));
                }
            }
        }

        let latest_update_indices: std::collections::HashSet<usize> =
            last_update_per_key.values().map(|(i, _)| *i).collect();

        messages
            .iter()
            .enumerate()
            .filter(|(i, msg)| {
                if msg.role == Role::System && msg.content.starts_with("MEMORY_UPDATE:") {
                    latest_update_indices.contains(i)
                } else {
                    true
                }
            })
            .map(|(_, m)| m.clone())
            .collect()
    }

    fn extract_memory_key(&self, content: &str) -> Option<String> {
        let prefix = "MEMORY_UPDATE:";
        if let Some(rest) = content.strip_prefix(prefix) {
            if let Some(pos) = rest.find('=') {
                return Some(rest[..pos].to_string());
            }
        }
        None
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

    #[test]
    fn test_filter_tool_duplicates_keeps_only_latest() {
        let planner = Planner::new();

        let messages = vec![
            Message::new(crate::domain::message::Role::User, "Primero"),
            Message::new(
                crate::domain::message::Role::Tool,
                "Tool result available: old data",
            ),
            Message::new(crate::domain::message::Role::Assistant, "Some answer"),
            Message::new(
                crate::domain::message::Role::Tool,
                "Tool result available: fresh data",
            ),
            Message::new(crate::domain::message::Role::User, "Segundo"),
        ];

        let filtered = planner.filter_tool_duplicates(&messages);

        let tool_count = filtered
            .iter()
            .filter(|m| m.role == crate::domain::message::Role::Tool)
            .count();
        assert_eq!(tool_count, 1, "Only one Tool should remain");
        assert!(
            filtered.iter().any(|m| m.content.contains("fresh data")),
            "Fresh tool result should be kept"
        );
    }

    #[test]
    fn test_compact_context_keeps_only_latest_assistant() {
        let planner = Planner::new();

        let messages = vec![
            Message::new(crate::domain::message::Role::User, "First user"),
            Message::new(
                crate::domain::message::Role::Assistant,
                "First assistant response",
            ),
            Message::new(crate::domain::message::Role::User, "Second user"),
            Message::new(
                crate::domain::message::Role::Tool,
                "Tool result available: data",
            ),
            Message::new(
                crate::domain::message::Role::Assistant,
                "Second assistant after tool",
            ),
        ];

        let compacted = planner.compact_context(&messages);

        assert_eq!(compacted.len(), 4);
        assert_eq!(compacted[0].content, "First user");
        assert_eq!(compacted[1].content, "Second user");
        assert_eq!(compacted[2].content, "Tool result available: data");
        assert_eq!(
            compacted[3].content, "Second assistant after tool",
            "Assistant after tool retained"
        );

        let assistant_count = compacted
            .iter()
            .filter(|m| m.role == crate::domain::message::Role::Assistant)
            .count();
        assert_eq!(assistant_count, 1, "Only one assistant should remain");
    }

    #[test]
    fn test_compact_context_drops_assistant_before_tool() {
        let planner = Planner::new();

        let messages = vec![
            Message::new(crate::domain::message::Role::User, "First user"),
            Message::new(
                crate::domain::message::Role::Assistant,
                "Reasoning before tool",
            ),
            Message::new(
                crate::domain::message::Role::Tool,
                "Tool result available: data",
            ),
            Message::new(
                crate::domain::message::Role::Assistant,
                "Final answer after tool",
            ),
        ];

        let compacted = planner.compact_context(&messages);

        let assistant_before_tool = compacted.iter().any(|m| {
            m.role == crate::domain::message::Role::Assistant
                && m.content == "Reasoning before tool"
        });
        assert!(
            !assistant_before_tool,
            "Assistant before tool should be dropped"
        );

        let assistant_count = compacted
            .iter()
            .filter(|m| m.role == crate::domain::message::Role::Assistant)
            .count();
        assert_eq!(assistant_count, 1, "Only assistant after tool remains");
    }

    #[test]
    fn test_compact_context_no_tool_keeps_latest_assistant() {
        let planner = Planner::new();

        let messages = vec![
            Message::new(crate::domain::message::Role::User, "First"),
            Message::new(crate::domain::message::Role::Assistant, "First response"),
            Message::new(crate::domain::message::Role::User, "Second"),
            Message::new(crate::domain::message::Role::Assistant, "Second response"),
        ];

        let compacted = planner.compact_context(&messages);

        assert_eq!(compacted.len(), 3);
        let assistant_count = compacted
            .iter()
            .filter(|m| m.role == crate::domain::message::Role::Assistant)
            .count();
        assert_eq!(assistant_count, 1, "Latest assistant kept when no tool");
        assert_eq!(compacted.last().unwrap().content, "Second response");
    }

    #[test]
    fn test_compact_context_latest_assistant_block_preserved() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(crate::domain::message::Role::User, "user"),
            Message::new(crate::domain::message::Role::Assistant, "assistant old"),
            Message::new(crate::domain::message::Role::Tool, "tool fresh"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final1"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final2"),
        ];
        let compacted = planner.compact_context(&messages);
        assert_eq!(compacted.len(), 4);
        assert_eq!(compacted[0].content, "user");
        assert_eq!(compacted[1].content, "tool fresh");
        assert_eq!(compacted[2].content, "assistant final1");
        assert_eq!(compacted[3].content, "assistant final2");
    }

    #[test]
    fn test_compact_context_assistant_before_block_removed() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(crate::domain::message::Role::User, "user"),
            Message::new(crate::domain::message::Role::Assistant, "assistant old1"),
            Message::new(crate::domain::message::Role::Assistant, "assistant old2"),
            Message::new(crate::domain::message::Role::User, "user2"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final"),
        ];
        let compacted = planner.compact_context(&messages);
        assert_eq!(compacted.len(), 3);
        assert_eq!(compacted[0].content, "user");
        assert_eq!(compacted[1].content, "user2");
        assert_eq!(compacted[2].content, "assistant final");
    }

    #[test]
    fn test_compact_context_tool_ending_removes_all_assistant() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(crate::domain::message::Role::User, "user"),
            Message::new(crate::domain::message::Role::Assistant, "assistant stale"),
            Message::new(crate::domain::message::Role::Tool, "tool fresh"),
        ];
        let compacted = planner.compact_context(&messages);
        assert_eq!(compacted.len(), 2);
        assert_eq!(compacted[0].content, "user");
        assert_eq!(compacted[1].content, "tool fresh");
    }

    #[test]
    fn test_compact_context_multiple_tools_preserves_latest_block() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(crate::domain::message::Role::User, "user"),
            Message::new(crate::domain::message::Role::Assistant, "assistant old"),
            Message::new(crate::domain::message::Role::Tool, "tool A"),
            Message::new(crate::domain::message::Role::Assistant, "assistant stale"),
            Message::new(crate::domain::message::Role::Tool, "tool B"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final1"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final2"),
        ];
        let compacted = planner.compact_context(&messages);
        assert_eq!(compacted.len(), 5);
        assert_eq!(compacted[0].content, "user");
        assert_eq!(compacted[1].content, "tool A");
        assert_eq!(compacted[2].content, "tool B");
        assert_eq!(compacted[3].content, "assistant final1");
        assert_eq!(compacted[4].content, "assistant final2");
    }

    #[test]
    fn test_compact_context_no_tool_preserves_latest_block() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(crate::domain::message::Role::User, "user1"),
            Message::new(crate::domain::message::Role::Assistant, "old1"),
            Message::new(crate::domain::message::Role::Assistant, "old2"),
            Message::new(crate::domain::message::Role::User, "user2"),
            Message::new(crate::domain::message::Role::Assistant, "final1"),
            Message::new(crate::domain::message::Role::Assistant, "final2"),
        ];
        let compacted = planner.compact_context(&messages);
        assert_eq!(compacted.len(), 4);
        assert_eq!(compacted[0].content, "user1");
        assert_eq!(compacted[1].content, "user2");
        assert_eq!(compacted[2].content, "final1");
        assert_eq!(compacted[3].content, "final2");
    }

    #[test]
    fn test_compact_context_assistant_after_tool_preserved() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(crate::domain::message::Role::User, "user"),
            Message::new(crate::domain::message::Role::Tool, "tool fresh"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final"),
        ];
        let compacted = planner.compact_context(&messages);
        assert_eq!(compacted.len(), 3);
        assert_eq!(compacted[0].content, "user");
        assert_eq!(compacted[1].content, "tool fresh");
        assert_eq!(compacted[2].content, "assistant final");
    }

    #[test]
    fn test_compact_context_no_tool_latest_assistant() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(crate::domain::message::Role::User, "user"),
            Message::new(crate::domain::message::Role::Assistant, "assistant 1"),
            Message::new(crate::domain::message::Role::Assistant, "assistant 2"),
        ];
        let compacted = planner.compact_context(&messages);
        assert_eq!(compacted.len(), 3);
        assert_eq!(compacted[0].content, "user");
        assert_eq!(compacted[1].content, "assistant 1");
        assert_eq!(compacted[2].content, "assistant 2");
    }

    #[test]
    fn test_compact_context_memory_update_does_not_break_latest_assistant_block() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(crate::domain::message::Role::User, "user"),
            Message::new(crate::domain::message::Role::Tool, "tool fresh"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final1"),
            Message::new(
                crate::domain::message::Role::System,
                "MEMORY_UPDATE:favorite_color=green",
            ),
            Message::new(crate::domain::message::Role::Assistant, "assistant final2"),
        ];
        let compacted = planner.compact_context(&messages);
        assert_eq!(compacted.len(), 5);
        assert_eq!(compacted[0].content, "user");
        assert_eq!(compacted[1].content, "tool fresh");
        assert_eq!(compacted[2].content, "assistant final1");
        assert_eq!(compacted[3].content, "MEMORY_UPDATE:favorite_color=green");
        assert_eq!(compacted[4].content, "assistant final2");
    }

    #[test]
    fn test_compact_context_multiple_memory_updates_inside_block() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(crate::domain::message::Role::User, "user"),
            Message::new(crate::domain::message::Role::Tool, "tool fresh"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final1"),
            Message::new(crate::domain::message::Role::System, "MEMORY_UPDATE:a=1"),
            Message::new(crate::domain::message::Role::System, "MEMORY_UPDATE:b=2"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final2"),
        ];
        let compacted = planner.compact_context(&messages);
        assert_eq!(compacted.len(), 6);
        assert_eq!(compacted[0].content, "user");
        assert_eq!(compacted[1].content, "tool fresh");
        assert_eq!(compacted[2].content, "assistant final1");
        assert_eq!(compacted[3].content, "MEMORY_UPDATE:a=1");
        assert_eq!(compacted[4].content, "MEMORY_UPDATE:b=2");
        assert_eq!(compacted[5].content, "assistant final2");
    }

    #[test]
    fn test_compact_context_non_memory_system_breaks_block() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(crate::domain::message::Role::User, "user"),
            Message::new(crate::domain::message::Role::Tool, "tool fresh"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final1"),
            Message::new(crate::domain::message::Role::System, "unrelated system"),
            Message::new(crate::domain::message::Role::Assistant, "assistant final2"),
        ];
        let compacted = planner.compact_context(&messages);
        assert_eq!(compacted.len(), 4);
        assert_eq!(compacted[0].content, "user");
        assert_eq!(compacted[1].content, "tool fresh");
        assert_eq!(compacted[2].content, "unrelated system");
        assert_eq!(compacted[3].content, "assistant final2");
    }

    #[test]
    fn test_compact_memory_updates_same_key_keeps_latest() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(
                crate::domain::message::Role::System,
                "MEMORY_UPDATE:favorite_color=verde",
            ),
            Message::new(
                crate::domain::message::Role::System,
                "MEMORY_UPDATE:favorite_color=azul",
            ),
        ];
        let compacted = planner.compact_memory_updates(&messages);
        assert_eq!(compacted.len(), 1);
        assert_eq!(compacted[0].content, "MEMORY_UPDATE:favorite_color=azul");
    }

    #[test]
    fn test_compact_memory_updates_different_keys_keeps_latest_of_each() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(
                crate::domain::message::Role::System,
                "MEMORY_UPDATE:favorite_color=verde",
            ),
            Message::new(
                crate::domain::message::Role::System,
                "MEMORY_UPDATE:occupation=engineer",
            ),
        ];
        let compacted = planner.compact_memory_updates(&messages);
        assert_eq!(compacted.len(), 2);
    }

    #[test]
    fn test_compact_memory_updates_preserves_non_memory_messages() {
        let planner = Planner::new();
        let messages = vec![
            Message::new(
                crate::domain::message::Role::System,
                "MEMORY_UPDATE:favorite_color=verde",
            ),
            Message::new(crate::domain::message::Role::User, "user message"),
            Message::new(
                crate::domain::message::Role::System,
                "MEMORY_UPDATE:favorite_color=azul",
            ),
            Message::new(
                crate::domain::message::Role::Tool,
                "Tool result available: time",
            ),
        ];
        let compacted = planner.compact_memory_updates(&messages);
        assert_eq!(compacted.len(), 3);
        assert_eq!(compacted[0].content, "user message");
        assert_eq!(compacted[1].content, "MEMORY_UPDATE:favorite_color=azul");
        assert_eq!(compacted[2].content, "Tool result available: time");
    }
}
