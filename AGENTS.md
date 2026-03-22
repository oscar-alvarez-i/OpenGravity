# OpenGravity Project

AI-first local agent project written in Rust with Telegram as the primary transport.

## Project Structure

- `src/` - Main source code with domain/adapters/infrastructure layering
- `src/domain/` - Core domain models and interfaces
- `src/adapters/` - LLM adapters (OpenRouter, Groq)
- `src/infrastructure/` - Transport (Telegram), persistence (SQLite)
- `.agents/` - Agent configuration, rules, workflows, and decisions
- `scripts/` - Validation and build scripts

## Build, Lint, and Test Commands

### Default Validation (Iterative Development)
```bash
./scripts/check_fast.sh
```
Runs: `cargo fmt --check` + `cargo clippy -q --all-targets --all-features -- -D warnings` + `cargo test -q`

### Full Validation (Before Commit/Release)
```bash
./scripts/check_full.sh
```
Runs: `cargo fmt --check` + `cargo clippy` + `cargo test` + `cargo nextest run` + `cargo tarpaulin --fail-under 87` + `cargo audit` + `cargo deny check` + `cargo build --release`

### Single Test Commands
```bash
cargo test module_name -q           # Run tests for specific module
cargo test test_name                # Run specific test
cargo test -q                       # Run all tests quietly
```

### Cargo Aliases (from .cargo/config.toml)
```bash
cargo cf   # fmt --check
cargo ct   # test -q
cargo cc   # clippy --all-targets --all-features -- -D warnings
```

## Code Style Guidelines

### Imports
- Use absolute imports with `crate::` prefix for internal modules
- Group std, external crates, then internal modules
- Use `#[allow(unused_imports)]` sparingly - prefer removing unused imports

### Formatting
- Run `cargo fmt` before committing
- Uses default rustfmt configuration (no custom rustfmt.toml)
- 4-space indentation (Rust default)

### Types and Error Handling
- Use `anyhow::Result` for application code (flexible error contexts)
- Use `thiserror` for library code (custom error types)
- Never use `unwrap()` or `expect()` in production code except for documented invariant cases
- Return `Result<T, E>` for fallible operations
- Use `anyhow!` macro for context-rich errors

### Naming Conventions
- **Modules**: `snake_case` (e.g., `tools/registry.rs`)
- **Types/Structs**: `PascalCase` (e.g., `ToolCall`, `LlmOrchestrator`)
- **Functions/Methods**: `snake_case` (e.g., `parse_tool_call`, `execute_tool`)
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Traits**: `PascalCase` (e.g., `LlmProvider`)
- **Files**: `snake_case.rs`

### Testing
- Use inline `mod tests` for internal module logic
- Use `#[cfg(test)]` module gating
- Use `mockall` for mocking trait implementations
- Use `wiremock` for HTTP mocking
- Use `serial_test` for tests requiring sequential execution
- Test naming: `test_feature_description`

## Important Directives

### Engineering Rules
- **Zero Warnings**: Never introduce compilation or Clippy warnings
- **No Debt**: Never introduce TODOs, FIXMEs, or temporary implementations
- **Coverage**: Never drop below 87% test coverage
- **CI**: Never break the CI pipeline

### Security Policy
- **Zero Trust**: All tools must be explicitly registered with rigorous input validation
- **Deny by Default**: Future tools are denied unless explicitly authorized
- **No Shell Execution**: Never execute arbitrary shell commands
- **No Secrets Exposure**: Never log or print sensitive environment variables

### Validation Policy
- **Default Command**: `./scripts/check_fast.sh` for iterative development
- **Full Validation**: Only when feature complete, refactor complete, dependencies changed, or before commit
- **Targeted Checks**: Prefer `cargo test module_name -q` for isolated modules

### Workflow Rules
- Never run full validation repeatedly during small fix loops
- Prefer narrow-scope validation before global validation
- Minimize output verbosity

## Architecture Guidelines

- **Layering**: Domain must remain pure (no external dependencies)
- **Interface**: Bot (Telegram) as the primary transport
- **Security**: Mandatory checks before tool execution
- **Isolation**: LLM adapters must be isolated
- **Registry**: All agent tools must be registry-driven

## Dependencies

- **Runtime**: tokio, teloxide, reqwest, serde, rusqlite, tracing, dotenvy, anyhow, thiserror, zeroize, chrono, async-trait
- **Dev**: mockall, wiremock, serial_test

## Reference Files

- `@.agents/rules/architecture_rules.md` - Architecture boundaries and patterns
- `@.agents/rules/rust_quality_rules.md` - Rust-specific quality rules
- `@.agents/rules/validation_rules.md` - Validation workflow rules
- `@.agents/workflows/stabilize.md` - Full stabilization workflow
- `@.agents/decisions/current_state.md` - Current architectural decisions and risks
- `@.agents/skills/rust-validation-skill/SKILL.md` - Validation skill procedures
- `@.agents/skills/rust-test-skill/SKILL.md` - Testing skill procedures
