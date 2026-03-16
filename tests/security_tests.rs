use open_gravity::domain::tool::ToolCall;
use open_gravity::security::whitelist::Whitelist;
use open_gravity::tools::registry::Registry;

#[test]
fn test_security_whitelist_bypass_attempt() {
    let wl = Whitelist::new(vec![42, 1337]);

    // Explicit bypass attempts
    assert!(!wl.is_allowed(0));
    assert!(!wl.is_allowed(9999999999999));
    assert!(!wl.is_allowed(43));

    // Safe
    assert!(wl.is_allowed(42));
}

#[test]
fn test_security_arbitrary_tool_execution_denied() {
    let registry = Registry::new();

    // Attempting to simulate an LLM hallucinating a dangerous tool
    let malicious_call = ToolCall {
        name: "execute_shell_command".to_string(),
        input: "rm -rf /".to_string(),
    };

    let result = registry.execute_tool(&malicious_call);

    // Deny by default mechanism must reject it
    assert!(result.output.is_err());
    assert_eq!(
        result.output.unwrap_err(),
        "Tool implementation not found or unauthorized"
    );
}
