# OpenGravity Active State

## Prioridad documental máxima

Este documento tiene prioridad sobre cualquier otra fuente cuando describe estado actual.

---

# Estado actual

Phase activa:

Phase 8 — Context Compression (closed)

---

# Phase status

- Phase 3: cerrada
- Phase 4: cerrada
- Phase 5: cerrada
- Phase 6: cerrada
- Phase 7: cerrada
- Phase 8: cerrada

---

# Confirmado estable

- memory extraction activa
- semantic overwrite de facts implementado
- planner multi-step activo
- pending_plan funcional
- SQLite persistencia estable
- runtime context compression activa

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

# Restricciones activas actuales

## get_current_time

Debe ejecutar fresh siempre.

No reutilizar resultado stale.

---

# Objetivo inmediato de trabajo

Preparar siguiente fase sin romper:

- determinismo runtime
- semantic continuity
- compression guarantees

---

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
