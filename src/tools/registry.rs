use crate::domain::tool::{FreshnessPolicy, ToolCall, ToolResult};
use std::collections::HashMap;

pub struct Registry {
    tools: HashMap<String, ToolDefinition>,
}

#[derive(Clone)]
struct ToolDefinition {
    freshness: FreshnessPolicy,
}

impl Registry {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let mut tools = HashMap::new();
        tools.insert(
            "get_current_time".to_string(),
            ToolDefinition {
                freshness: FreshnessPolicy::AlwaysFresh,
            },
        );
        Self { tools }
    }

    pub fn freshness_policy(&self, tool_name: &str) -> FreshnessPolicy {
        self.tools
            .get(tool_name)
            .map(|t| t.freshness)
            .unwrap_or_default()
    }

    /// Parses the LLM textual response to find `TOOL:tool_name`.
    /// ONLY accepts TOOL if it's on the last non-empty line.
    pub fn parse_tool_call(&self, response: &str) -> Option<ToolCall> {
        let lines: Vec<&str> = response.lines().collect();
        if lines.is_empty() {
            return None;
        }

        for line in lines.into_iter().rev() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(stripped) = trimmed.strip_prefix("TOOL:") {
                let tool_name = stripped.trim().to_string();
                if !tool_name.is_empty() {
                    return Some(ToolCall {
                        name: tool_name,
                        input: "".to_string(),
                    });
                }
            }
            break;
        }
        None
    }

    /// Zero-trust tool execution based on whitelist
    pub fn execute_tool(&self, call: &ToolCall) -> ToolResult {
        let output = match call.name.as_str() {
            "get_current_time" => super::current_time::execute(&call.input),
            _ => Err("Tool implementation not found or unauthorized".to_string()),
        };

        ToolResult {
            name: call.name.clone(),
            output,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_call_success() {
        let registry = Registry::new();
        let res = registry.parse_tool_call("I need the time.\nTOOL:get_current_time");
        assert!(res.is_some());
        assert_eq!(res.unwrap().name, "get_current_time");
    }

    #[test]
    fn test_parse_tool_only_last_line() {
        let registry = Registry::new();
        let res = registry.parse_tool_call("Texto previo\nTOOL:get_current_time");
        assert!(res.is_some());

        let res_invalid = registry.parse_tool_call("TOOL:get_current_time\nTexto después");
        assert!(
            res_invalid.is_none(),
            "TOOL not on last line should be rejected"
        );

        let res_no_tool = registry.parse_tool_call("Solo texto sin tool");
        assert!(res_no_tool.is_none());
    }

    #[test]
    fn test_parse_no_tool() {
        let registry = Registry::new();
        let res = registry.parse_tool_call("I need the time.");
        assert!(res.is_none());
    }

    #[test]
    fn test_execute_known_tool() {
        let registry = Registry::new();
        let call = ToolCall {
            name: "get_current_time".to_string(),
            input: "{}".to_string(),
        };
        let res = registry.execute_tool(&call);
        assert!(res.output.is_ok());
    }

    #[test]
    fn test_execute_unregistered_tool() {
        let registry = Registry::new();
        let call = ToolCall {
            name: "arbitrary_shell".to_string(),
            input: "ls".to_string(),
        };
        let res = registry.execute_tool(&call);
        assert!(res.output.is_err());
        assert_eq!(
            res.output.unwrap_err(),
            "Tool implementation not found or unauthorized"
        );
    }
}
