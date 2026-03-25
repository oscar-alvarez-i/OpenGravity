# OpenGravity Active State

## Prioridad documental máxima

Este documento tiene prioridad sobre cualquier otra fuente cuando describe estado actual.

---

# Estado actual

Phase activa:

Phase 7 — Advanced Memory Semantics
---

# Phase status

* Phase 3: cerrada
* Phase 4: cerrada
* Phase 5: cerrada
* Phase 6: cerrada
* Phase 7: activa

---

## Confirmado estable

* memory extraction activa
* planner multi-step activo
* pending_plan funcional
* SQLite persistencia estable

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

# Restricciones activas actuales

## get_current_time

Debe ejecutar fresh siempre.

No reutilizar resultado stale.

# Objetivo inmediato de trabajo

Resolver semántica real de memoria persistente.

---

# Scope permitido actual

Se permite:

* overwrite semántico explícito
* conflict resolution mínima
* update controlado de facts persistentes
* tests semánticos por provider y memoria

---

# Scope no permitido actual

No abrir:

* context compression
* planner expansion
* refactor estructural amplio

hasta estabilizar semántica de memoria

---

# Señales de cierre necesarias para avanzar

Debe confirmarse:

* max_iterations estable
* duplicate prevention correcta
* cacheable vs non-cacheable validado
* runtime manual correcto
* sin hacks temporales

---

# Regla operativa para nuevos chats

Antes de proponer cambios:

1 identificar módulo exacto afectado
2 verificar si pertenece a Phase 6
3 validar primero si el cambio pertenece a memoria semántica real