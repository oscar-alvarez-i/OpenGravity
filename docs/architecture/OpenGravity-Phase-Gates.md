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
Status: CLOSED

## Objetivo

Derivar desde el prompt original una matriz exacta de cumplimiento funcional v1.0.

## Acceptance

- runtime contracts documented ✓
- no runtime behavior changed ✓
- no test debt introduced ✓

## Implementation

- PlanStep::Direct runtime contract documented in Active-State
- functional branch contracts documented in Active-State

---

# Phase 19 — Regression Closure
Status: CLOSED

## Objetivo

Test-only coverage hardening de contratos runtime documentados en Phase 17.

## Scope permitido

- regression tests para contratos documentados
- test-only, sin cambios de runtime
- no refactor estructural
- no nueva arquitectura

## Acceptance

- tests green ✓
- no runtime behavior changed ✓
- no architecture change ✓
- invariants preserved ✓

## Implementation

- test_executor_direct_step_from_factual_continues: PlanStep::Direct regression
- test_executor_b1_factual_miss_continues: B1 factual miss fallback regression

---

# Phase 20 — Loop State Hardening
Status: CLOSED

## Objetivo

Validar que el estado sensible del loop no persista stale entre conversaciones nuevas.

## Scope permitido

- skill_just_ran flag lifecycle validation
- reset en boundary de nueva conversación (AgentLoop::run())
- test de regresión para stale state prevention
- mínimo: reset via método público en Executor

## Scope prohibido

- no reset внутри loop iterations (rompería intra-loop behavior)
- no refactor de branches
- no nueva arquitectura de estado

## Acceptance mínimo

- skill_just_ran persists within loop (test_loop_resolves passes) ✓
- skill_just_ran resets between run() calls (test_executor_skill_not_stale_across_runs passes) ✓
- executor.reset_loop_state() público mínimo ✓
- loop.rs invoca reset en run() entry ✓
- tests green ✓
- validación obligatoria pasa ✓
- sensitive executor state proven non-stale across sequential runs ✓
- same executor instance validated ✓
- no branch-order regression ✓
- no collateral state mutation (reset limited to skill_just_ran only) ✓

## Implementation

- Executor::reset_loop_state() método público (src/agent/executor.rs:60-62) resetea solo skill_just_ran
- AgentLoop::run() invoca reset al inicio (src/agent/loop.rs:80)
- test_executor_skill_not_stale_across_runs usa el MISMO AgentLoop para dos run() consecutivos

---

# Phase 2.2 — Tool Execution Layer
Status: CLOSED

## Objetivo

Introducir un contrato explícito de Tool Execution sin alterar comportamiento runtime.

El objetivo es formalizar cómo se ejecutan las tools, no cambiar semántica de ejecución.

---

## Scope permitido

- Introducir struct ToolExecutionRequest
- Introducir struct ToolExecutionResult
- Centralizar entry point de ejecución en ToolRegistry
- Normalizar manejo de output de tools
- Preservar modelo de handlers existente (fn pointer)

---

## Scope prohibido

- No refactor de executor
- No cambios en planner
- No cambios en skill system
- No async ni modelo de concurrencia
- No nuevos traits o capas de abstracción
- No cambios en firma de handlers
- No lógica de retry
- No orquestación de ejecución
- No nuevos subsistemas

---

## Acceptance

- Tool execution accesible vía método explícito único
- Comportamiento existente permanece idéntico
- Sin cambio en orden de ejecución
- Sin regresión en outputs de tools
- Tests validan paths de éxito y failure
- Restricciones de seguridad de Phase 2.1 intactas

---

## Constraints

- Diff mínimo
- Sin expansión arquitectural
- Comportamiento determinístico preservado

---

## Implementation

- ToolExecutionRequest struct introduced
- ToolExecutionResult struct introduced
- ToolRegistry::execute() implemented as single execution entry point
- Executor migrated to use ToolExecutionResult
- Legacy execute_tool() remains temporarily for compatibility

---

## Known Limitations

- Legacy function execute_tool() still present
- Temporary dual execution path exists
- Must be removed in Phase 2.3

---

# Phase 2.4 — Tool Execution Observability
Status: CLOSED

## Objetivo

Introducir logging mínimo en el Tool Execution Layer para mejorar auditabilidad sin alterar comportamiento runtime.

## Scope permitido

- logging en ToolRegistry::execute()
- log de:
  - tool_name
  - success/failure
- no logging de payload completo
- no cambio de structs
- no cambio de ejecución

## Scope prohibido

- no tracing complejo
- no métricas
- no async logging
- no cambios en executor

## Acceptance

- logs visibles en ejecución real
- sin impacto en tests
- sin cambio semántico
- overhead mínimo

---

## Implementation

- logging added in ToolRegistry::execute()
- events:
  - tool.execute.start
  - tool.execute.success
  - tool.execute.failure
- implemented using tracing macros (debug, info, warn)
- no runtime semantic changes
- no impact on tests
- minimal diff
- aligned with existing logging infrastructure

---

# Phase 2.1 — Safe Local Tool Contract
Status: CLOSED

## Objetivo

Validar contrato de ejecución personal local con side-effect real mínimo y seguro.

## Scope permitido

- Nueva tool write_local_note
- Path fijo sandboxed (./local_notes.txt)
- Append-only, sin overwrite
- Input validation: no empty, no multiline

## Scope prohibido

- No scheduler
- No cron
- No background jobs
- No external connectors
- No custom path input
- No delete/update semantics

## Acceptance

- write_local_note ejecuta correctamente ✓
- archivo local se crea o append correctamente ✓
- FreshnessPolicy::Cacheable (compatible con duplicate prevention) ✓
- tests green (223 total) ✓
- validación obligatoria pasa ✓
- single-line enforcement ✓
- deterministic local file contract ✓

## Implementation

- src/tools/local.rs: write_to_path() factorizada, execute() con validación
- src/tools/mod.rs: pub mod local agregado
- src/tools/registry.rs: write_local_note registrado con Cacheable

---

# Regla final de cierre

Una phase solo cierra si no necesita explicación defensiva.

Si requiere decir:

"funciona pero después limpiamos"

la phase sigue abierta.