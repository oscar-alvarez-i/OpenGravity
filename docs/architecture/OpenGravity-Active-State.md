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
- Phase 2.4: CLOSED (observability)
- Phase 2.5: CLOSED (read_local_notes)
- Phase 2.6: PENDING (Filesystem IO Contract Closure)
- Phase 2.7: PENDING (Tool Execution Path Cleanup)
- Phase 2.8: PENDING (Local Tool Error Contract)
- Phase 2.9: PENDING (HITO Certification)

Tools disponibles:
  - write_local_note
  - read_local_notes
- Acceso local completo: write + read sobre archivo sandboxed
- Branch actual: main (workflow mergeado)
- Next: Phase 2.6

---

## Phase 2.6 — Filesystem IO Contract Closure (PENDING)

## Objetivo

Unificar acceso IO a filesystem en único punto interno.

## Scope

- Factorización de helpers internos
- Reutilizar resolve_note_path() y validate_note_path()

## Acceptance

- Sin duplicación
- tests green
- validación mínima pasa

---

## Phase 2.7 — Tool Execution Path Cleanup (PENDING)

## Objetivo

Eliminar execute_tool() legacy, usar solo ToolRegistry::execute()

## Acceptance

- Single execution path
- sin regresión

---

## Phase 2.8 — Local Tool Error Contract (PENDING)

## Objetivo

Formalizar contrato de errores de tools locales.

## Scope

- archivo inexistente
- input inválido
- error de IO

## Acceptance

- errores documentados
- tests de error coverage

---

## Phase 2.9 — HITO 2 Certification (PENDING)

## Objetivo

Cierre formal del HITO 2.

## Scope

- documentación de contratos finales
- explicitación de limitaciones
- tests faltantes

## Acceptance

- sin deuda implícita
- sistema auditado
- listo para cierre

---

## Phase 2.2 — Tool Execution Layer (CLOSED)

---

# Runtime invariants

## Branch execution order

1. pending_plan → 2. skill (factual) → 3. skill (full) → 4. planner → 5. LLM → 6. tool

## MAX_LOOP_ITERATIONS = 4

## Freshness semantics

- AlwaysFresh: get_current_time (ignora duplicate prevention)
- Cacheable (default): otras tools previenen duplicates

## Tool protocol

- TOOL:tool_name en última línea no-vacía
- Contenido después de TOOL causa rechazo

## Memory semantics

- Last-write-wins por key
- Duplicate suppression activo
- Transient facts filtrados (hoy, ahora, etc.)

---

# Confirmado estable

- executor branch tracing activo
- memory extraction + persistence activo
- freshness decision tracing activo
- semantic overwrite de facts funcionando
- planner multi-step activo
- SQLite persistencia estable
- runtime context compression activo

---

# Runtime validado esperado

Input: "Mi color favorito es verde y después decime la hora"

Output esperado:
1. memory update
2. pending_plan
3. tool fresh execution
4. respuesta final ≤3 iteraciones

---

# Restricciones activas

## get_current_time

Siempre fresh, no reutilizar resultado stale.

## write_local_note

- Path fijo: ./local_notes.txt
- Append-only, single-line input
- Freshness: Cacheable

---

# Objetivo inmediato de trabajo

Phase 2.2 (próxima)

---

# Scope prohibido

- refactor estructural amplio
- planner rewrite
- provider redesign
- autonomous operations

sin gate documental previo.

---

# Repository Governance

- protected `main` branch
- required CI: quality, security, extended
- squash merge policy
- branch + PR workflow obligatorio

---