use crate::domain::tool::{ToolCall, ToolResult};

pub struct Registry;

impl Registry {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }

    /// Parses the LLM textual response to find `TOOL:tool_name`.
    /// Currently only parsing first tool for simplicity, without JSON structured extraction.
    pub fn parse_tool_call(&self, response: &str) -> Option<ToolCall> {
        for line in response.lines() {
            let trimmed = line.trim_start();
            if let Some(stripped) = trimmed.strip_prefix("TOOL:") {
                let tool_name = stripped.trim().to_string();
                if !tool_name.is_empty() {
                    return Some(ToolCall {
                        name: tool_name,
                        input: "".to_string(),
                    });
                }
            }
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
