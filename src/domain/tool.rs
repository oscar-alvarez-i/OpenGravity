use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum FreshnessPolicy {
    #[default]
    Cacheable,
    AlwaysFresh,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    pub name: String,
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResult {
    pub name: String,
    pub output: Result<String, String>,
}

impl FreshnessPolicy {
    pub fn is_fresh(&self) -> bool {
        matches!(self, FreshnessPolicy::AlwaysFresh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_call() {
        let call = ToolCall {
            name: "get_current_time".to_string(),
            input: "".to_string(),
        };
        assert_eq!(call.name, "get_current_time");
    }

    #[test]
    fn test_freshness_policy_default() {
        assert!(!FreshnessPolicy::default().is_fresh());
    }

    #[test]
    fn test_freshness_policy_always_fresh() {
        assert!(FreshnessPolicy::AlwaysFresh.is_fresh());
    }
}
