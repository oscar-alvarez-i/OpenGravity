# OpenGravity Phase Gates

## Propósito

Definir alcance permitido y criterio de cierre por phase.

---

# Regla global

No abrir nueva phase si la actual conserva deuda estructural crítica.

---

# Acceptance global obligatorio

Siempre debe cumplirse:

* runtime correcto
* arquitectura consistente
* tests correctos
* sin hacks activos

---

# Validación mínima obligatoria

cargo fmt

cargo clippy --all-targets --all-features -- -D warnings

cargo test

---

# Phase 4 — Executor Hardening

## Objetivo

Eliminar fragilidad residual del loop principal.

## Scope permitido

* max_iterations control
* duplicate prevention correcta
* freshness por tipo de tool
* cleanup de flujo
* reducción de pseudo-state

## Acceptance

Debe cumplir:

* loop estable
* no stale tool reuse incorrecto
* retries explícitos
* contexto consistente de tool results

## No aceptable

* subir iteraciones para ocultar defectos
* hacks de bypass
* loops ambiguos

---

# Phase 5 — Skill System Expansion

## Objetivo

Formalizar framework extensible de skills.

## Scope permitido

Nueva interfaz skill:

* trigger
* evaluate
* apply

## Acceptance

* skill selection determinística
* múltiples skills coexistiendo
* orden explícito validado por tests

Primero debe eliminarse cualquier dependencia de orden accidental de HashMap antes de validar múltiples skills.

## No aceptable

* skill order implícito
* skill side effects ocultos

---

PHASE 6 — Provider Abstraction
Status: CLOSED

Coverage reached: 87.25%
Total tests: 187
Provider adapters covered:
✓ groq
✓ openrouter
✓ fallback paths
✓ network and HTTP error paths

Acceptance:
✓ Orchestrator depends only on trait objects
✓ Concrete provider names removed from orchestrator core
✓ Sequential fallback preserved
✓ Full suite green
✓ Coverage >=87%

---

# Phase 7 — Advanced Memory Semantics
Status: CLOSED

## Objetivo

Resolver persistencia semántica correcta de facts equivalentes.

## Acceptance

* overwrite correcto entre facts equivalentes ✓
* no duplicación semántica ✓
* conflict resolution explícita ✓
* tests de actualización persistente ✓

Implementation:
* find_memory_by_key y update_memory_by_key en Db layer
* save_memory_update ahora verifica fact_key existente antes de persistir
* last-write-wins policy
* SQL LIKE pattern corregido para evitar prefix collision

---

# Phase 8 — Context Compression

## Objetivo

Reducir crecimiento de contexto.

## Acceptance

* resumen útil sin pérdida crítica

---

# Phase 9 — Bounded Persistent Memory Retrieval

## Objetivo

Separar retrieval de conversation history vs. persistent memories con budgets determinísticos.

## Scope permitido

* fetch_conversation_only(6) — sin MEMORY_*
* fetch_memories_only(20, 4) — scan 20, filtra, toma 4
* merge: memories primero, conversation después

## Acceptance

* bounded retrieval: máximo 10 items
* memories recientes guaranteed en prompt
* no rompe executor ordering
* tests green

---

# Phase 10 — Observability Layer
Status: CLOSED

## Objetivo

Consolidar logging y tracing existente sin alterar comportamiento runtime.

## Scope permitido

* execution flow traces (entry/exit cada stage)
* timing básico por step
* no modificar comportamiento

## Acceptance

* tests green ✓
* logging funcional ✓
* sin regresión funcional ✓

Implementation:
* branch tracing implementado en executor.rs
* timing básico por branch usando Instant
* observability mínima sin cambio semántico runtime

---

# Phase 11 — Plugin Architecture
Status: CLOSED

## Objetivo

Extensión externa controlada.

## Implementation

- ToolDefinition ahora almacena: freshness + handler (fn pointer)
- Registry::register(name, freshness, handler) público
- Duplicate registration retorna error explícito
- Built-in tools protegidas contra override

## Acceptance

- external registration habilitado ✓
- duplicate protection cerrada ✓
- built-in override prevention closed ✓
- executor no modificado ✓
- tests green ✓
- external registration boundary covered ✓

---

# Phase 12 — Deep Observability
Status: CLOSED

## Objetivo

Tracing profundo para debugging detallado.

## Implementation

- Executor branch tracing: entry/exit/timing por branch
- Tool handler execution tracing: result success/failure visible
- Memory extraction skill tracing activo
- Memory persistence tracing: insert + overwrite visible
- Freshness decision tracing: executor duplicate gate visible ✓

## Acceptance

- executor branches fully visible ✓
- tool boundary visible ✓
- memory layer visible ✓
- freshness decisions visible ✓
- tests green ✓

# Phase 13 — Production Readiness

## Objetivo

Cerrar arquitectura estable.

## Acceptance

* invariantes preservados
* sesiones largas correctas

---

# Regla final de cierre

Una phase solo cierra si no necesita explicación defensiva.

Si requiere decir:

"funciona pero después limpiamos"

la phase sigue abierta.