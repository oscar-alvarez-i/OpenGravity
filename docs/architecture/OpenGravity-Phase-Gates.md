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

## Objetivo

Resolver persistencia semántica correcta de facts equivalentes.

## Acceptance

* overwrite correcto entre facts equivalentes
* no duplicación semántica
* conflict resolution explícita
* tests de actualización persistente

---

# Phase 8 — Context Compression

## Objetivo

Reducir crecimiento de contexto.

## Acceptance

* resumen útil sin pérdida crítica

---

# Phase 9 — Multi-Intent Planning

## Objetivo

Planner con dependencias simples.

## Acceptance

* secuencia correcta
* branching simple controlado

---

# Phase 10 — Plugin Architecture

## Objetivo

Extensión externa controlada.

## Acceptance

* plugins aislados
* core intacto

---

# Phase 11 — Observability Layer

## Objetivo

Tracing profundo.

## Acceptance

* logs suficientes para reconstrucción total

---

# Phase 12 — Production Readiness

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