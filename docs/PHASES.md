# Phases

This document records the technical evolution of OpenGravity through implementation phases.

A phase represents a bounded architectural milestone.

It is not equivalent to a release.

Each phase closes only when runtime behavior becomes stable enough to support the next structural step.

---

# Phase Model

Each phase aims to:

- solve one architectural layer at a time
- avoid simultaneous redesign of multiple boundaries
- preserve runtime continuity
- reduce hidden complexity

---

# Phase 1 — Runtime Foundation

## Goal

Establish the first deterministic conversational runtime.

## Main Outcomes

- initial Rust project structure
- base conversation loop
- first message handling model
- explicit runtime ownership inside Rust

## Architectural Impact

This phase established the principle that orchestration authority belongs to the runtime, not the language model.

## Main Constraint Introduced

Execution must remain explicit.

---

# Phase 2 — Planning Introduction

## Goal

Separate response generation from execution intent.

## Main Outcomes

- planner introduced
- pending plan concept created
- first explicit execution staging

## Architectural Impact

The runtime gained a formal distinction between:

- deciding what to do
- doing it

## Main Constraint Introduced

Planning must remain lightweight.

---

# Phase 3 — Execution Stabilization

## Goal

Make plan execution operationally reliable.

## Main Outcomes

- executor became a distinct runtime boundary
- step-by-step execution hardened
- tool invocation integrated into execution flow
- memory continuation became stable enough for practical use

## Architectural Impact

The system moved from experimental orchestration to bounded execution.

## Main Constraint Introduced

Executor may execute only the active step.

---

# Phase 4 — Executor Hardening

## Goal

Reduce semantic drift during execution and strengthen runtime safety.

## Main Outcomes

- stricter step consumption behavior
- improved tool freshness handling
- duplicate execution prevention
- tighter planner / executor continuity

## Architectural Impact

Execution became less tolerant to ambiguity and more deterministic.

## Main Constraint Introduced

Executor must not silently reinterpret completed intent.

## Known Pressure Left Open

Skill formalization remained incomplete.

---

# Phase 5 — Skill Formalization (Closed)

## Goal

Move repeated execution patterns into explicit reusable skills.

## Main Outcomes

- skill framework with deterministic ordering
- registry with auto-registration
- echo skill as second working skill
- planner context hygiene filters:
  - filter_closed_tool_cycles: remove stale tool triplets
  - trim_stale_user_turns: keep only latest user turn
- executor same-turn tool blocking: prevents loop reopening

## Architectural Impact

Skills added without weakening executor authority. Context trimming preserves determinism.

## Main Constraint Preserved

A skill must never become an implicit planner.

## Known Limitations Remaining

- skill selection uses single path (first match)
- no priority weighting system

---

# Future Phase Direction

Likely future areas include:

- stronger provider abstraction
- richer execution diagnostics
- stricter tool identity contracts
- expanded runtime observability

Future phases should continue preserving:

- explicit execution
- bounded state transitions
- deterministic orchestration

---

# Maintenance Rule

A phase should be updated only when:

- the milestone is technically closed
- runtime behavior has stabilized
- architectural consequences are understood

Do not use this document for speculative work.

Speculative work belongs in `ROADMAP.md`.