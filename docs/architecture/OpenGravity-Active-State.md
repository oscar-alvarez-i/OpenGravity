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

## HITO 2 — Personal Execution Layer (CLOSED)

- Phase 2.1: CLOSED (write_local_note)
- Phase 2.2: CLOSED (Tool Execution Layer)
- Phase 2.3: CLOSED (Skill Execution Ordering)
- Phase 2.4: CLOSED (observability)
- Phase 2.5: CLOSED (read_local_notes)
- Phase 2.6: CLOSED (Filesystem IO Contract Closure)
- Phase 2.7: CLOSED (Tool Execution Path Cleanup)
- Phase 2.8: CLOSED (Local Tool Error Contract)
- Phase 2.9: CLOSED (Certification)
- Phase 2.10: CLOSED (Idempotency Enforcement)

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
- Idempotencia: in-memory via full message history
- Input validation: enforced against original user message (pre-LLM normalization)

#### Idempotency Scope (Phase 2.10)

- **Implementation**: Full unfiltered message history scanning
- **Source of truth**: Tool messages with format `"Tool result available: <tool>:<input>; <output>"`
- **Matching**: Strict parsing (exact match on `<tool>:<input>`)
- **Exemption**: AlwaysFresh tools (`get_current_time`) bypass idempotency

#### Guarantees

- No duplicate execution for same tool + input within a runtime session
- Cross-turn protection inside agent loop
- Deterministic behavior (no fuzzy matching)

#### Limitations

- NOT persisted across process restarts
- NOT shared across distributed instances
- Depends on in-memory history (AgentLoop)

#### Invariants
- Multiline user input → tool MUST NOT execute
- Same tool + same input (same runtime instance) → MUST NOT execute twice
- AlwaysFresh tool → always executes (no idempotency check)

#### E2E Validation Scenarios

**Scenario 1 — First execution**
```
User: "Guardá hola"
→ Tool executes
→ Tool message stored: "Tool result available: write_local_note:hola; nota guardada"
```

**Scenario 2 — Different input**
```
User: "Guardá chau"
→ Tool executes (input differs)
→ Tool message stored: "Tool result available: write_local_note:chau; nota guardada"
```

**Scenario 3 — Same input (IDEMPOTENCY)**
```
User: "Guardá hola"
→ Tool executes
→ Tool message stored

User: "Guardá hola"
→ Idempotency detected (name + input match)
→ Tool MUST NOT execute
→ Assistant returns idempotency message
→ local_notes.txt has ONE entry for "hola"
```

**Scenario 4 — AlwaysFresh tool**
```
User: "¿Qué hora es?"
→ Tool executes every time
→ No idempotency check applied
→ get_current_time alwaysFresh
```

#### Planned
- Persistence to DB in future phases (see Roadmap)

---

# Repository Governance

- protected `main` branch
- required CI: quality, security, extended
- squash merge policy
- branch + PR workflow obligatorio

---