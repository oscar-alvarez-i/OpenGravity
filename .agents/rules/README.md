# Rules Index

Centralized rules for OpenGravity agent operations.

## Architecture Rules (`architecture_rules.md`)

- **Layering**: Domain must remain pure.
- **Interface**: Bot as the primary transport.
- **Security**: Mandatory checks before tool execution.
- **Isolation**: LLM adapters must be isolated.
- **Registry**: All agent tools must be registry-driven.
- **Refactoring**: Preserve current boundaries and contracts before refactoring.

## Quality Rules (`rust_quality_rules.md`)

- **Zero Warnings**: No compilation or Clippy warnings allowed.
- **Clippy Rules**: All Clippy warnings are considered failures.
- **Hygiene**: No `TODO` or `FIXME` comments in production code.
- **Dependencies**: No unnecessary crates; justify every new one.
- **Safety**: No `unwrap()` or `expect()` outside of documented invariant cases.

## Validation Rules (`validation.md`)

- **Iterative Dev**: Only use `check_fast.sh` (or `cargo ct` / `cargo cc`) for small changes.
- **Scope First**: Validate only impacted modules before global checks.
- **Avoid Coverage**: Never run `tarpaulin` during micro-fix loops.
- **Full Validation**: Mandatory only for stabilization, dependency changes, or before commit.
- **Quiet Mode**: Prefer quiet flags (`-q`) and aliases to minimize terminal output.
- **Iterative Default**: Use `./scripts/check_fast.sh` for routine iterations.
- **Security Check**: Run `audit`/`deny` only when `Cargo.toml` changed.

---

For workflows, see `../workflows/`.
For skills, see `../skills/`.
