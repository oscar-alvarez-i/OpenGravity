# Release Discipline

This document defines the minimum conditions required before creating a release tag for OpenGravity.

A release is valid only if runtime behavior, validation pipeline, and architectural consistency all remain intact.

---

# Release Scope

A release may represent:

- phase closure
- tagged stable checkpoint
- externally shareable version

A release must never be created only because code compiles.

---

# Mandatory Validation

## Full Validation Pipeline

```bash
./scripts/check_full.sh
````

Must pass completely.

This includes:

* `cargo fmt --check`
* `cargo clippy -- -D warnings`
* `cargo test`
* `cargo nextest run`
* `cargo tarpaulin --fail-under 95`
* `cargo audit`
* `cargo deny check`
* `cargo build --release`

---

# Code Safety Requirements

Before release:

* no forbidden `unwrap()`, `expect()`, or undocumented `panic!()`
* no `TODO` or `FIXME` in production paths
* no warnings in release build
* no dead code introduced without reason

---

# Security Requirements

Before release:

* no secrets exposed in logs
* environment variables explicitly validated
* tool boundaries remain deny-by-default
* no unsafe execution path introduced

---

# Architectural Consistency

Before release:

* architecture boundaries remain respected
* executor invariants preserved
* planner and executor semantics remain aligned
* no undocumented runtime shortcut added

Primary references:

* `docs/ARCHITECTURE.md`
* `docs/KNOWN_FRAGILITIES.md`

---

# Runtime Validation

Automated validation is not sufficient.

Before release, verify manually:

* real Telegram interaction
* multi-turn continuity
* tool execution path
* provider fallback if relevant

A release must survive real conversational execution.

---

# Phase Awareness

If a release closes a phase:

verify consistency with:

* `docs/PHASES.md`

A phase should not be tagged closed while known blocking instability remains.

---

# Fragility Awareness

Before release:

review current active fragilities in:

* `docs/KNOWN_FRAGILITIES.md`

A release may proceed with fragilities only if they are visible and bounded.

---

# Tagging Rule

Tag only after all checks pass.

Suggested pattern:

```text
phase4-stable
v0.4.0
```

Choose tags that reflect actual architectural meaning.

---

# Final Rule

A release is accepted only if:

runtime behavior remains predictable under real usage.
