# OpenGravity Agent Guide

OpenGravity is a local-first conversational AI agent written in Rust.

The project prioritizes explicit execution control, deterministic orchestration, and bounded tool usage.

Telegram is currently the primary transport.

---

# Project Structure

- `src/` — main source tree
- `src/domain/` — core domain models and interfaces
- `src/adapters/` — LLM adapters and providers
- `src/infrastructure/` — Telegram transport, SQLite persistence
- `.agents/` — internal rules, workflows, skills, decisions
- `docs/` — architecture and project evolution documents
- `scripts/` — validation scripts

---

# Validation Commands

## Iterative Development

```bash
./scripts/check_fast.sh
````

Runs:

* `cargo fmt --check`
* `cargo clippy -q --all-targets --all-features -- -D warnings`
* `cargo test -q`

## Full Validation

```bash
./scripts/check_full.sh
```

Runs:

* `cargo fmt --check`
* `cargo clippy`
* `cargo test`
* `cargo nextest run`
* `cargo tarpaulin --fail-under 85`
* `cargo audit`
* `cargo deny check`
* `cargo build --release`

## Targeted Validation

```bash
# Run all tests
cargo test

# Run tests matching a pattern
cargo test module_name
cargo test test_name_here

# Run a specific test function
cargo test -- test_function_name -q

# Run clippy on library code only
cargo clippy --lib -q

# Run clippy on a specific file
cargo clippy -- --cap-lints warn src/path/to/file.rs
```

## Useful Cargo Aliases (defined in .cargo/config.toml)

```bash
cargo cf   # cargo fmt --check
cargo ct   # cargo test -q
cargo cc   # cargo clippy --all-targets --all-features -- -D warnings
```

---

# Patch Task Workflow

Before any patch task:

1. Read `docs/architecture/OpenGravity-Active-State.md` first
2. Infer active phase explicitly
3. Reject scope if phase unclear
4. Use `docs/architecture/OpenGravity-Patch-Template.md` as mandatory task frame

---

# Engineering Invariants

* never introduce compilation warnings
* never introduce clippy warnings
* never introduce TODOs, FIXMEs, or temporary placeholders
* never reduce test coverage below required threshold
* never break CI

---

# Security Constraints

* tool execution is deny-by-default
* every tool must be explicitly registered
* all tool inputs must be validated
* arbitrary shell execution is forbidden
* sensitive environment variables must never be logged

---

# LLM Error Handling

* **Timeouts**: Implement reasonable timeouts per provider (default: 30s for OpenRouter, 60s for streaming)
* **Retries**: Apply exponential backoff for transient failures (max 3 retries)
* **Fallback**: Support multiple LLM providers with automatic fallback on failure
* **Rate Limits**: Respect provider rate limits; implement request queuing if needed
* **Logging**: Log LLM errors with context (model, provider, token count) but never log API keys
* **Graceful Degradation**: If all LLMs fail, respond with a helpful error message instead of crashing

---

# Code Style

## Formatting

* Uses standard rustfmt defaults (no custom rustfmt.toml)
* Run `cargo fmt` before committing
* 4-space indentation (default)
* 100 char line length recommended (soft limit)

## Imports

* use `crate::` absolute imports for internal modules
* group imports in order: std -> external -> internal
* avoid wildcard imports (`use crate::module::*`)

## Types

* prefer explicit type annotations for public API
* use `Arc<T>` for shared ownership, `Rc<T>` rarely
* use `Box<dyn Trait>` for trait objects
* prefer `&str` over `&String` in function signatures
* use `Vec<T>` for dynamically-sized collections

## Error Handling

* use `anyhow::Result` in application code (binaries, entry points)
* use `thiserror` for library-style typed errors (src/lib)
* avoid `unwrap()` and `expect()` outside documented invariants
* propagate errors with `?` operator
* provide context with `anyhow::Context`

## Naming

* modules: `snake_case`
* structs / enums / traits: `PascalCase`
* functions: `snake_case`
* constants: `SCREAMING_SNAKE_CASE`
* variables: `snake_case`
* field names: `snake_case`

## Documentation

* Document public APIs with doc comments (`///`)
* Include usage examples where helpful
* Document invariants in code comments when using `unwrap()`

---

# Architecture References

Primary technical references:

* `docs/ARCHITECTURE.md`
* `docs/KNOWN_FRAGILITIES.md`
* `docs/PHASES.md`

---

# Internal Agent References

Detailed internal rules remain in:

* `@.agents/rules/README.md` — Index of all rules
* `@.agents/rules/architecture_rules.md`
* `@.agents/rules/rust_quality_rules.md`
* `@.agents/rules/validation.md`
* `@.agents/workflows/stabilize.md`
* `@.agents/decisions/current_state.md`

## Available Skills

* `rust-validation-skill` — Validation procedures
* `rust-test-skill` — Testing procedures
* `production-audit-skill` — Production readiness audits

---

# Important Constraint

Skills, tools, and future architectural extensions must preserve executor authority.

No component should silently become an autonomous planner.
