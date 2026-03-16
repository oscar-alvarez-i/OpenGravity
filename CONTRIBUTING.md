# CONTRIBUTING.md

# Contributing to OpenGravity

Thank you for contributing to OpenGravity.

This project is being developed as a controlled long-term agent platform, with emphasis on deterministic behavior, architectural clarity, regression safety, and production-grade evolution.

OpenGravity is intentionally built under strict engineering discipline:

> **No new capability is accepted unless current behavior remains measurable, testable, and observable.**

This document defines how contributions must be prepared, validated, and proposed.

---

# Core Engineering Principles

## 1. Preserve deterministic baseline behavior

Before adding any feature, assume the current baseline is fragile until proven otherwise.

Every change must preserve:

* deterministic loop execution
* stable message contracts
* tool execution boundaries
* memory integrity
* transport isolation

If behavior changes, the change must be explicit and documented.

---

## 2. Never mix refactor and feature work

Each contribution must have a single dominant purpose:

* feature
* refactor
* test hardening
* observability
* architecture boundary

Avoid mixed pull requests.

Mixed changes make regressions difficult to isolate.

---

## 3. Tests are mandatory for behavioral changes

Any behavioral modification must include tests.

Minimum expectation:

* unit test if local logic changes
* integration test if loop behavior changes
* regression test if message flow changes

No exception.

---

## 4. Observability must improve or remain equal

A contribution must never reduce debugging capability.

Logging policy:

### info!

Use only for:

* user input received
* tool selected
* tool executed
* skill selected
* loop boundaries
* final answer emitted

### debug!

Use for:

* raw LLM output
* parsing internals
* context snapshots
* internal skill inputs

### error!

Use only for actual failures.

Avoid noisy logs.

---

# Repository Working Rules

## Required validation before proposing changes

Run:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
./scripts/test-baseline.sh
cargo test -- --nocapture
```

If any command fails, contribution is not ready.

---

## Baseline command

Preferred validation shortcut:

```bash
make baseline
```

This must remain green.

---

# Architectural Boundaries

Respect current module responsibilities.

## agent/

Contains deterministic execution loop.

Avoid placing feature logic directly here.

---

## memory/

Contains persistence and memory retrieval responsibilities.

Memory semantics must remain explicit.

---

## skills/

Contains reusable cognitive units.

Skills must remain isolated.

No hidden persistence.

---

## workflows/

Contains explicit execution chains.

Do not bypass workflow boundaries once introduced.

---

## prompts/

Prompt fragments must remain externalized.

Never reintroduce long inline prompt literals unless justified.

---

## observability/

Future tracing and metrics boundaries belong here.

---

# Message Contract Rules

Current message contract is critical.

## Assistant reasoning before tool execution

Assistant reasoning that leads to a tool call:

* must not persist to DB
* must not re-enter active context
* must not become memory input

---

## Tool messages

Tool outputs:

* persist when required by current baseline
* must remain distinguishable from assistant content

---

## User commands

Transport commands such as `/start`:

* must remain excluded from memory persistence
* must remain excluded from active context

---

# Testing Philosophy

## Add exact tests for exact behavior

Avoid vague tests.

Good test example:

* test_memory_extracts_stable_fact_only

Weak test example:

* test_memory_behavior

---

## Prefer deterministic fixtures

Use fixtures when possible:

```text
/tests/fixtures/
```

Avoid duplicating raw long strings across tests.

---

## Mock LLM whenever possible

Do not consume real provider tokens for core tests.

All skill tests must be deterministic.

---

## Regression tests protect production behavior

If a bug was fixed once, regression coverage should exist.

---

# Prompt Contribution Rules

Prompt changes are architecture changes.

Treat them carefully.

If modifying prompts:

* preserve role separation
* preserve tool instructions
* preserve memory rules
* validate behavior manually after change

---

# Documentation Rules

Any structural change must update docs when relevant.

Examples:

* ADR update
* roadmap adjustment
* execution contract clarification

---

# Commit Discipline

Use conventional commit style.

Examples:

```bash
feat(skills): add deterministic memory extraction skill
fix(memory): prevent duplicate stable fact persistence
chore(tests): extend regression coverage for skill dispatch
refactor(loop): isolate skill decision gate
```

---

## Avoid premature commit fragmentation

When closing a phase:

Prefer coherent final commit after green validation.

---

# Before Closing Any Phase

Mandatory checklist:

* tests green
* baseline green
* manual Telegram validation performed
* logs reviewed
* no hidden context drift

---

# Manual Validation Sequence

Minimum production validation:

1. Hola
2. Qué modelo estás usando?
3. Mi color favorito es azul
4. Qué hora es?
5. Cuál es mi color favorito?

Expected:

* no reasoning visible
* tool works once
* memory persists
* no duplicated tool loop

---

# Strategic Rule

OpenGravity must grow as a controlled platform.

Not as accumulated features.

If a change increases power but weakens predictability, it is incomplete.

---

# If Unsure

Prefer:

* smaller change
* stronger test
* clearer boundary

Over:

* hidden cleverness
* speculative abstraction
* premature complexity
