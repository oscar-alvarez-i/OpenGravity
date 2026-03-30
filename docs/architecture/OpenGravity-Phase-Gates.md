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
Status: CLOSED

## Objetivo

Cerrar arquitectura estable.

## Implementation

- Assistant MEMORY_SET/MEMORY_UPDATE/MEMORY_DELETE now enters semantic save_memory_update pipeline
- Multi-key assistant memory persistence: "key1=val1, key2=val2" parsed and persisted
- Semantic overwrite validated: user fact overwritten by assistant directive works correctly

## Acceptance

- assistant memory directives enter same pipeline as user-derived memory ✓
- semantic overwrite works for assistant → user facts ✓
- multi-key assistant memory persists correctly ✓
- 8 tests added and green ✓
- residual accepted debt: raw assistant text still stored in DB but retrieval filters to semantic system memories only ✓
- invariantes preservados ✓
- sesiones largas correctas ✓

---

# Phase 14 — Runtime Integrity & Functional Certification
Status: CLOSED

## Objetivo

Cerrar certificación funcional mínima del runtime actual mediante documentación de comportamientos implícitos y tests funcionales faltantes.

## Scope permitido

1. Documentación de comportamientos implícitos existentes:
   - Branch execution priority order
   - MAX_LOOP_ITERATIONS = 4
   - AlwaysFresh bypass
   - Reasoning stripping
   - Tool protocol strictness
   - Memory key overwrite semantics

2. Tests funcionales faltantes:
   - MEMORY_DELETE end-to-end
   - Echo skill en conversación
   - Combinación: memory overwrite + fresh tool execution

## Acceptance

- Documentación actualizada en Phase-Gates.md y Active-State.md ✓
- Tests funcionales agregados y verdes ✓
- Validación mínima obligatoria pasa:
  - cargo fmt ✓
  - cargo clippy --all-targets --all-features -- -D warnings ✓
  - cargo test ✓
- Invariantes globales preservadas ✓

## Implementation

### Documented implicit behaviors

1. **Branch execution priority** (src/agent/executor.rs:230-410):
   - pending_plan → skill(factual fragment) → skill(full message) → planner → LLM → tool

2. **MAX_LOOP_ITERATIONS = 4** (src/agent/loop.rs:47):
   - Límite hardcoded en agente loop
   - Error: "Agent loop max iterations (4) reached without final resolution"

3. **AlwaysFresh bypass** (src/agent/executor.rs:108-114):
   - get_current_time siempre ejecuta, ignora duplicate prevention
   - Otras herramientas con Cacheable (default) sí preven duplicates

4. **Reasoning stripping** (src/agent/executor.rs:422-425):
   - Assistant messages que preceden tool call no se persist en DB
   - Solo se persiste: User → Tool → Assistant(final)

5. **Tool protocol strictness** (src/tools/registry.rs:54-77):
   - TOOL:tool_name debe estar en última línea no-vacía
   - Contenido después de línea TOOL causa rechazo del tool call

6. **Memory key overwrite semantics** (src/db/sqlite.rs:88-102):
   - save_memory_update verifica existencia de fact_key
   - Si existe: UPDATE, si no: INSERT
   - SQL LIKE pattern matching para encontrar keys

### Tests added

- test_memory_delete_end_to_end: MEMORY_DELETE funcional
- test_echo_skill_in_conversation: echo skill en flow
- test_memory_overwrite_pending_plan_fresh_tool: combinacion memory overwrite + fresh tool execution

---

# Phase 15 — IA Dev Protocol Hardening
Status: CLOSED

## Objetivo

Formalizar protocolo operativo explícito para clasificación de cambios, control de scope y definición de cierre.

Protocolo completo documentado en: `OpenGravity-Debug-Discipline.md` → sección "Development Protocol (Phase 15)"

---

# Phase 16 — Autonomous Agent Safety Layer
Status: CLOSED

## Objetivo

Formalizar reglas mínimas de seguridad de ejecución autónoma antes de v1.0.

## Scope permitido

- executor-level repetition detection
- bounded identical tool replay protection
- planner step repetition guard
- explicit autonomous termination criteria
- deterministic execution fences sin alterar ordering

## Scope prohibido

- no new subsystem
- no planner redesign
- no memory semantics change
- no ToolRegistry ownership change
- no architectural reordering

## Acceptance mínimo

- repeated autonomous branch detectable ✓
- identical replay bounded ✓
- stop reason observable ✓
- invariantes preservadas (1 skill, 2 pending_plan, 3 planner, 4 llm, 5 tool) ✓
- tests requirement: coverage for bounded repetition scenarios ✓

## Implementation

- last_tool_executed state para bounded identical replay
- should_block_identical_replay helper con reset en assistant response
- pending_plan bypass para permitir step repetitions en planes explícitos
- branch-sensitive replay policy: pending_plan sin restricción, planner y tool con protección

---

# Phase 17 — Functional Contract Audit v1.0
Status: OPEN

## Objetivo

Derivar desde el prompt original una matriz exacta de cumplimiento funcional v1.0.

---

# Regla final de cierre

Una phase solo cierra si no necesita explicación defensiva.

Si requiere decir:

"funciona pero después limpiamos"

la phase sigue abierta.