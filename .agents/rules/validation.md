# Validation Discipline Rule

Optimizes development cycles and reduces token consumption by enforcing a 3-tier validation model.

## Rules

- **Iterative Dev**: Only use `check_fast.sh` (or `cargo ct` / `cargo cc`) for small changes.
- **Scope First**: Validate only impacted modules before global checks.
- **Avoid Coverage**: Never run `tarpaulin` during micro-fix loops.
- **Full Validation**: Mandatory only for stabilization, dependency changes, or before commit.
- **Quiet Mode**: Prefer quiet flags (`-q`) and aliases to minimize terminal output.
- **Iterative Default**: Use `./scripts/check_fast.sh` for routine iterations.
- **Security Check**: Run `audit`/`deny` only when `Cargo.toml` changed.
- **Process**: Prefer targeted module validation before global validation.

## Preferred Commands

- Fast: `./scripts/check_fast.sh`
- Clippy: `cargo cc`
- Tests: `cargo ct`
- Formatting: `cargo cf`
