# OpenGravity Active State

## Prioridad documental máxima

Este documento tiene prioridad sobre cualquier otra fuente cuando describe estado actual.

---

# Estado actual

Phase 10 — Observability Layer (closed)

Phase 11 — Plugin Architecture (closed)

Phase 12 — Deep Observability (closed)

Phase 13 — Production Readiness (closed)

Phase 14 — Runtime Integrity & Functional Certification (CLOSED)

Phase 15 — IA Dev Protocol Hardening (CLOSED)

Phase 16 — Autonomous Agent Safety Layer (CLOSED)

Phase 17 — Functional Contract Audit v1.0 (CLOSED)

Phase 18 — Documentation Closure (CLOSED)

---

# Phase status

- Phase 3: cerrada
- Phase 4: cerrada
- Phase 5: cerrada
- Phase 6: cerrada
- Phase 7: cerrada
- Phase 8: cerrada
- Phase 9: cerrada
- Phase 10: closed (executor observability active)
- Phase 11: closed (Plugin Architecture complete)
- Phase 12: closed (Deep Observability)
- Phase 13: closed (Production Readiness)
- Phase 14: CLOSED (Runtime Integrity & Functional Certification)
- Phase 15: CLOSED (IA Dev Protocol Hardening)
- Phase 16: CLOSED (Autonomous Agent Safety Layer - bounded replay, branch-sensitive policy, invariants preserved)
- Phase 17: CLOSED (Functional Contract Audit v1.0 - runtime contracts documented)
- Phase 18: CLOSED (Documentation Closure - runtime contracts fully documented)

---

# ToolRegistry extensibility

ToolRegistry ahora permite extensión externa controlada:

- Almacena execution handlers directamente (fn pointer)
- register(name, freshness, handler) habilita herramientas externas
- Duplicate registration rechazada explícitamente (error, no overwrite)
- Built-in tools (get_current_time) no pueden ser sobreescritas

---

# Confirmado estable

- executor branch tracing activo
- timing básico por branch activo
- ToolRegistry handler execution tracing activo
- memory extraction activa
- memory persistence tracing activo (insert + overwrite)
- freshness decision tracing activo
- semantic overwrite de facts implementado
- assistant MEMORY_SET/MEMORY_UPDATE/MEMORY_DELETE enters same semantic pipeline ✓
- multi-key assistant memory persists correctly ✓
- planner multi-step activo
- pending_plan funcional
- SQLite persistencia estable
- runtime context compression activa
- residual accepted debt: raw assistant text still stored but retrieval filters semantic system memories

---

# Documented implicit behaviors (Phase 14 — closed)

## Branch execution priority

El executor evalúa en orden estricto (src/agent/executor.rs:230-410):

1. **pending_plan** — reanuda trabajo interrumpido
2. **skill (factual fragment)** — extrae hecho de mensaje multi-step
3. **skill (full message)** — skill con trigger pattern
4. **planner** — crea plan multi-step
5. **LLM** — generación directa
6. **tool** — ejecución de herramienta

**B1 sin memory_updates:** Si skill factual extrae pero no emite memory_updates, no se crea pending_plan, flujo continúa a LLM.

## MAX_LOOP_ITERATIONS = 4

Límite hardcoded en agent loop (src/agent/loop.rs:47):
- Si se alcanza sin resolución: error "Agent loop max iterations (4) reached without final resolution"
- get_current_time (AlwaysFresh) puede alcanzar este límite

## AlwaysFresh bypass

get_current_time tiene FreshnessPolicy::AlwaysFresh (src/tools/registry.rs:21-26):
- Siempre ejecuta, ignora duplicate prevention
- Otras herramientas usan Cacheable (default) y sí previenen duplicates
- Executor replay guard:
  - `should_block_identical_replay` usa `last_tool_executed` para bloquear replay inmediato de la misma tool
  - El estado se resetea solo cuando downstream skill retorna content (src/agent/executor.rs:549)

## Reasoning stripping

Assistant messages que preceden tool call no se persist en DB (src/agent/executor.rs:422-425):
- Solo se persiste: User → Tool → Assistant(final)
- Reasoning "Voy a llamar la herramienta" se descarta

## Tool protocol strictness

src/tools/registry.rs:54-77:
- TOOL:tool_name debe estar en última línea no-vacía
- Contenido después de línea TOOL causa rechazo del tool call
- Tool inválido retorna "Tool implementation not found or unauthorized"

## Memory key overwrite semantics

src/db/sqlite.rs:88-102:
- save_memory_update verifica existencia de fact_key
- Si existe: UPDATE, si no: INSERT
- SQL LIKE pattern matching: `%MEMORY_SET:{key}=%` y `%MEMORY_UPDATE:{key}=%`
- Last-write-wins policy

## Memory duplicate suppression

src/skills/memory.rs:256-264:
- Si fact_key ya existe con valor idéntico (case-insensitive): no se emite UPDATE
- Previene UPDATE redundante cuando valor no cambia

## Transient fact filtering

src/skills/memory.rs:131-159:
- Facts con indicadores transitorios (hoy, ahora, qué hora, el clima) son ignorados
- No se persisten facts de consulta momentánea

## Planner tool whitelist

src/skills/planner.rs:77,102:
- Solo tools reconocidas: get_current_time, get_weather
- Texto residual no-tool se convierte en PlanStep::Direct

## B1 skill miss fallback

src/agent/executor.rs:300-346 (B1) + 348-387 (B2):
- Si factual fragment no matchea skill: continúa a B2 (full message skill)
- Si B2 no matchea: continúa a D (planner) → E (LLM)

## PlanStep::Direct runtime contract

src/skills/planner.rs:44-72 y src/agent/executor.rs:281-293:

Cuando el planner recibe texto residual que no matchea tool conocido (get_current_time, get_weather):

1. **planner emits PlanStep::Direct(text)** — parser crea PlanStep::Direct para texto no-reconocible como tool
2. **stored in pending_plan** — almacenado en pending_plan del executor via set_pending_plan()
3. **consumed next executor cycle** — Branch A (pending_plan) consume el step en siguiente turno
4. **emits no messages** — PlanStep::Direct produce StepResult con vec![] (messages.is_empty)
5. **forces continuation toward LLM branch** — should_continue=true, fluye hacia Branch E (LLM)

**Caso ejemplo:**

- Input: "Mi color favorito es azul y después contame un chiste"
- Turn 1: Memory extrae (favorite_color=azul), pending_plan = Direct("contame un chiste")
- Turn 2: pending_plan consume Direct step, retorna vec![], should_continue=true
- Flow alcanza Branch E (LLM) para generar respuesta al chiste

**Rol semántico:** deferred conversational continuation preserving post-memory user intent.

---

# Runtime validado esperado

Caso:

Mi color favorito es verde y después decime la hora

Debe producir:

1 memory update
2 pending_plan
3 tool fresh execution
4 respuesta final <=3 iteraciones

---

# Runtime compression activa

## Reglas activas

### Tool compression

Bloques consecutivos de tool output conservan solo el último resultado.

### Memory compression

Para:

- MEMORY_SET
- MEMORY_UPDATE
- MEMORY_DELETE

se conserva solo la última ocurrencia por key.

- compact_memory_updates (src/agent/planner.rs:234-269) elimina updates superseded por key
- Preserva orden original de mensajes no-memory

---

# Garantía actual

La compresión no altera ordering lógico entre:

- user
- assistant
- tool
- system

excepto remoción de estados obsoletos.

---

# Bounded Persistent Memory Retrieval

## Problema resuelto

En sesiones largas, múltiples memories persistidas podían saturar el contexto porque el límite global (10) era compartido entre conversation history y persistent memories.

## Solución implementada

Separación determinística de retrieval en budgetes independientes:

- `fetch_conversation_only(6)` — últimos 6 mensajes de conversación
- `fetch_memories_only(20, 4)` — scan 20 últimos registros, filtra MEMORY_*, toma 4 más recientes

## Merge order

Persistent memories se inyectan primero como bloque estable, luego conversation reciente:

```
[mem1, mem2, mem3, mem4, conv1, conv2, conv3, conv4, conv5, conv6]
```

## Invariante mantenida

- bounded retrieval: máximo 10 items en prompt
- prioridad estable: memories recientes primero
- no rompe executor ordering
- no duplica lógicamente (compact_memory_updates sigue funcionando)

---

# Restricciones activas actuales

## get_current_time

Debe ejecutar fresh siempre.

No reutilizar resultado stale.

---

# Objetivo inmediato de trabajo

Phase 19 — Regression Closure (CLOSED):

Phase 20 — Loop State Hardening (CLOSED):

# Scope no permitido inmediato

No abrir:

- refactor estructural amplio
- planner rewrite
- provider redesign

sin gate documental previo.

---

# Regla operativa para nuevos chats

Antes de proponer cambios:

1 identificar módulo exacto afectado
2 verificar fase real actual
3 validar impacto runtime antes de tocar arquitectura

---

# Next closure phases

### Phase 19 — Regression Closure

Test-only hardening of documented runtime contracts:

- PlanStep::Direct dedicated regression
- B1 factual fragment miss fallback regression

### Phase 20 — Loop State Hardening

CLOSED

State-sensitive runtime validation before release hardening:

- skill_just_ran lifecycle contract:
  - persists intra-loop (between iterations within same run)
  - resets inter-run (between new AgentLoop::run() calls)
- reset via executor.reset_loop_state() called at run() entry
- reset limited exclusively to skill_just_ran (no collateral state mutation)
- stale loop-sensitive executor state reset validated
- same-instance regression test validated: test_executor_skill_not_stale_across_runs uses same AgentLoop for two consecutive run() calls

---

# Repository Governance Baseline (post v1.0.0)

- protected `main` branch
- required CI checks: `quality`, `security`, `extended`
- squash merge policy
- CODEOWNERS enabled
- PR / issue templates active

## Operative rule

All normal changes enter via branch + PR. Direct commits to `main` are exceptional only.
