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

Phase 16 — Autonomous Agent Safety Layer (OPEN - implementation covered / observation pending)

Phase 17 — Functional Contract Audit v1.0 (OPEN)

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
- Phase 16: OPEN (Autonomous Agent Safety Layer - scope definido, implementación no iniciada)
- Phase 17: OPEN (Functional Contract Audit v1.0)

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

## MAX_LOOP_ITERATIONS = 4

Límite hardcoded en agent loop (src/agent/loop.rs:47):
- Si se alcanza sin resolución: error "Agent loop max iterations (4) reached without final resolution"
- get_current_time (AlwaysFresh) puede alcanzar este límite

## AlwaysFresh bypass

get_current_time tiene FreshnessPolicy::AlwaysFresh (src/tools/registry.rs:21-26):
- Siempre ejecuta, ignora duplicate prevention
- Otras herramientas usan Cacheable (default) y sí previenen duplicates

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

Phase 13 — Production Readiness (CLOSED):

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
