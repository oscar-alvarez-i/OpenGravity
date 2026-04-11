use open_gravity::security::whitelist::Whitelist;
use open_gravity::tools::registry::{Registry, ToolExecutionRequest};

#[test]
fn test_security_whitelist_bypass_attempt() {
    let wl = Whitelist::new(vec![42, 1337]);

    // Explicit bypass attempts
    assert!(!wl.is_allowed(0));
    assert!(!wl.is_allowed(5));

    // Valid indices should be allowed
    assert!(wl.is_allowed(42));
    assert!(wl.is_allowed(1337));
}

#[test]
fn test_execute_tool_rejects_unknown_tools() {
    let registry = Registry::new();

    // Attempting to simulate an LLM hallucinating a dangerous tool
    let request = ToolExecutionRequest {
        tool_name: "execute_shell_command".to_string(),
        input: "rm -rf /".to_string(),
    };

    let result = registry.execute(request);

    // Deny by default mechanism must reject it
    assert!(!result.success);
    assert_eq!(
        result.error.unwrap(),
        "Tool implementation not found or unauthorized"
    );
}

#[test]
fn test_security_arbitrary_tool_execution_denied() {
    let registry = Registry::new();

    // Attempting to simulate an LLM hallucinating a dangerous tool
    let request = ToolExecutionRequest {
        tool_name: "arbitrary_execution".to_string(),
        input: "any input".to_string(),
    };

    let result = registry.execute(request);

    // Deny by default mechanism must reject it
    assert!(!result.success);
    assert!(result.error.is_some());
}
