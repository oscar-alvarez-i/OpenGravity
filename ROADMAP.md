# OpenGravity Roadmap

This document tracks forward-looking architectural direction only.

Completed milestones are recorded in:

- `docs/PHASES.md`

Current runtime architecture is documented in:

- `docs/ARCHITECTURE.md`

Known architectural pressure points remain visible in:

- `docs/KNOWN_FRAGILITIES.md`

---

# Strategic Principle

OpenGravity should evolve as a controlled execution platform, not as accumulated feature growth.

Every new capability must satisfy:

- explicit boundaries
- measurable behavior
- regression safety
- removability if necessary

No capability should be added unless current behavior remains protected.

---

# Current Active Direction

## Phase 5 — Skill Formalization

## Goal

Introduce explicit reusable skills without weakening executor determinism.

## Focus Areas

- stable skill contracts
- skill registry design
- executor-controlled skill invocation
- output normalization

## Constraint

A skill must never become an implicit planner.

## Expected Result

Repeated execution patterns should move out of ad hoc executor logic while preserving explicit runtime control.

---

# Next Direction

## Phase 6 — Stronger Execution Contracts

## Goal

Strengthen execution semantics under growing complexity.

## Focus Areas

- richer step identity
- stronger tool freshness semantics
- safer multi-step continuity
- reduced semantic ambiguity between planner and executor

## Constraint

Execution must remain step-bounded.

---

# Medium-Term Direction

## Phase 7 — Long-Term Memory Evolution

## Goal

Expand durable memory without weakening runtime boundaries.

## Focus Areas

- retrieval ranking
- contradiction handling
- memory aging strategy
- stronger persistence semantics

## Constraint

Memory must remain post-execution authority, not execution authority.

---

# Later Direction

## Phase 8 — Observability Expansion

## Goal

Increase runtime introspection without adding hidden complexity.

## Focus Areas

- structured metrics
- execution tracing
- latency visibility
- failure classification

---

# Future Interfaces

## Phase 9 — External Runtime Interfaces

Possible future directions:

- CLI mode
- API layer
- web interface
- controlled remote execution

These interfaces must preserve the same runtime invariants already enforced in Telegram mode.

---

# Permanent Validation Rule

Before closing any future phase:

- tests green
- manual real runtime validation
- logs reviewed
- no hidden execution drift

---

# Architectural Priority Order

When tradeoffs appear, priority remains:

1. determinism
2. safety
3. clarity
4. extensibility

Never reverse this order.