---
name: rust-test-skill
description: Procedures for testing Rust modules in OpenGravity.
---
# Rust Test Skill
- **Logic**: Use inline unit tests (`mod tests`) for internal module logic.
- **Flow**: Use integration tests in `tests/` for cross-module or complex flows.
- **Assertions**: Prefer branch-specific assertions over trivial property checks.
