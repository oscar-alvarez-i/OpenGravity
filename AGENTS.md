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
cargo test module_name -q
cargo test test_name
cargo clippy --lib -q
```

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

# Code Style

## Imports

* use `crate::` absolute imports for internal modules
* group std / external / internal imports

## Error Handling

* use `anyhow::Result` in application code
* use `thiserror` for library-style typed errors
* avoid `unwrap()` and `expect()` outside documented invariants

## Naming

* modules: `snake_case`
* structs / enums / traits: `PascalCase`
* functions: `snake_case`
* constants: `SCREAMING_SNAKE_CASE`

---

# Architecture References

Primary technical references:

* `docs/ARCHITECTURE.md`
* `docs/KNOWN_FRAGILITIES.md`
* `docs/PHASES.md`

---

# Internal Agent References

Detailed internal rules remain in:

* `@.agents/rules/architecture_rules.md`
* `@.agents/rules/rust_quality_rules.md`
* `@.agents/rules/validation_rules.md`
* `@.agents/workflows/stabilize.md`
* `@.agents/decisions/current_state.md`

---

# Important Constraint

Skills, tools, and future architectural extensions must preserve executor authority.

No component should silently become an autonomous planner.
