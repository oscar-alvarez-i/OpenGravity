# OpenGravity Active State

## Prioridad documental máxima

Este documento tiene prioridad sobre cualquier otra fuente cuando describe estado actual.

---

# Estado actual

## HITO 1 — Autonomous Runtime Foundation (v1.0.0 CLOSED)

Legacy closure summary:

- Phase 1–5: core runtime foundation
- Phase 6–10: planner stabilization
- Phase 11–15: safety + bounded execution
- Phase 16–20: replay discipline + documentation closure

Detail maintained in: `OpenGravity-Phase-Gates.md`

---

## HITO 2 — Personal Execution Layer (ACTIVE)

- Phase 2.1: CLOSED (write_local_note)
- Phase 2.2: CLOSED (Tool Execution Layer)
- Phase 2.3: CLOSED (Skill Execution Ordering)
- Phase 2.4: CLOSED (observability)
- Phase 2.5: CLOSED (read_local_notes)
- Phase 2.6: CLOSED (Filesystem IO Contract Closure)
- Phase 2.7: CLOSED (Tool Execution Path Cleanup)
- Phase 2.8: CLOSED (Local Tool Error Contract)
- Phase 2.9: CLOSED (Certification)
- Phase 2.10: ACTIVE (E2E Validation & Hardening)

### Tooling Layer

Tools registradas:
- write_local_note (write)
- read_local_notes (read)

Capabilities:
- Persistencia local (filesystem sandbox)
- Lectura completa del histórico de notas
- Enforcement de invariantes de tool execution (executor-level)

### write_local_note

- Persistencia: filesystem (`local_notes.txt`)
- Input constraint: single-line only (enforced at executor level)
- Idempotencia: in-memory (executor `last_tool_input`)
- Input validation: enforced against original user message (pre-LLM normalization)

#### Invariants
- Multiline user input → tool MUST NOT execute
- Same tool + same input (same runtime instance) → MUST NOT execute twice

#### Limitations
- Idempotencia NO es persistente (se pierde entre reinicios)
- Duplicados posibles a nivel storage

#### Planned
- Migración a DB en fases futuras (ver Roadmap)

---

# Repository Governance

- protected `main` branch
- required CI: quality, security, extended
- squash merge policy
- branch + PR workflow obligatorio

---