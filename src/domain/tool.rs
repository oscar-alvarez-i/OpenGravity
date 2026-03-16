use serde::{Deserialize, Serialize};

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
}
