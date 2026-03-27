use crate::domain::tool::{FreshnessPolicy, ToolCall, ToolResult};
use std::collections::HashMap;
use tracing::{debug, info};

pub struct Registry {
    tools: HashMap<String, ToolDefinition>,
}

struct ToolDefinition {
    freshness: FreshnessPolicy,
    handler: fn(&str) -> Result<String, String>,
}

impl Registry {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry
            .register(
                "get_current_time",
                FreshnessPolicy::AlwaysFresh,
                super::current_time::execute,
            )
            .unwrap();
        registry
    }

    pub fn freshness_policy(&self, tool_name: &str) -> FreshnessPolicy {
        self.tools
            .get(tool_name)
            .map(|t| t.freshness)
            .unwrap_or_default()
    }

    pub fn register(
        &mut self,
        name: impl Into<String>,
        freshness: FreshnessPolicy,
        handler: fn(&str) -> Result<String, String>,
    ) -> Result<(), String> {
        let name = name.into();
        if self.tools.contains_key(&name) {
            return Err(format!("Tool '{}' already registered", name));
        }
        self.tools
            .insert(name, ToolDefinition { freshness, handler });
        Ok(())
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
        debug!("Tool handler executing: {}", call.name);

        let output = match self.tools.get(&call.name) {
            Some(def) => (def.handler)(&call.input),
            None => Err("Tool implementation not found or unauthorized".to_string()),
        };

        match &output {
            Ok(result) => {
                info!(
                    "Tool '{}' executed successfully, result length: {}",
                    call.name,
                    result.len()
                );
            }
            Err(err) => {
                info!("Tool '{}' execution failed: {}", call.name, err);
            }
        }

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

    #[test]
    fn test_parse_tool_call_empty_string() {
        let registry = Registry::new();
        let res = registry.parse_tool_call("");
        assert!(res.is_none());
    }

    #[test]
    fn test_register_external_tool_executes() {
        let mut registry = Registry::new();
        registry
            .register("echo", FreshnessPolicy::Cacheable, |input: &str| {
                Ok(format!("echo: {}", input))
            })
            .unwrap();

        let call = ToolCall {
            name: "echo".to_string(),
            input: "hello".to_string(),
        };
        let res = registry.execute_tool(&call);
        assert!(res.output.is_ok());
        assert_eq!(res.output.unwrap(), "echo: hello");
    }

    #[test]
    fn test_register_unknown_tool_still_fails() {
        let registry = Registry::new();
        let call = ToolCall {
            name: "unknown_tool".to_string(),
            input: "".to_string(),
        };
        let res = registry.execute_tool(&call);
        assert!(res.output.is_err());
        assert_eq!(
            res.output.unwrap_err(),
            "Tool implementation not found or unauthorized"
        );
    }

    #[test]
    fn test_register_preserves_freshness_policy() {
        let mut registry = Registry::new();
        registry
            .register("test_tool", FreshnessPolicy::Cacheable, |_: &str| {
                Ok("result".to_string())
            })
            .unwrap();

        let freshness = registry.freshness_policy("test_tool");
        assert_eq!(freshness, FreshnessPolicy::Cacheable);

        let builtin_freshness = registry.freshness_policy("get_current_time");
        assert_eq!(builtin_freshness, FreshnessPolicy::AlwaysFresh);
    }

    #[test]
    fn test_duplicate_registration_rejected() {
        let mut registry = Registry::new();
        registry
            .register("test_tool", FreshnessPolicy::Cacheable, |_: &str| {
                Ok("first".to_string())
            })
            .unwrap();

        let result = registry.register("test_tool", FreshnessPolicy::AlwaysFresh, |_: &str| {
            Ok("second".to_string())
        });

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Tool 'test_tool' already registered");
    }

    #[test]
    fn test_builtin_tool_cannot_be_replaced() {
        let mut registry = Registry::new();

        let result = registry.register("get_current_time", FreshnessPolicy::AlwaysFresh, |_| {
            Ok("fake time".to_string())
        });

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Tool 'get_current_time' already registered"
        );
    }
}
