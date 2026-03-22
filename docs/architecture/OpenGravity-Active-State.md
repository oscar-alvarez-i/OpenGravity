# OpenGravity Active State

## Prioridad documental máxima

Este documento tiene prioridad sobre cualquier otra fuente cuando describe estado actual.

---

# Estado actual

Phase activa:

Phase 4 — Executor Hardening

---

# Phase 3 estado real

Phase 3 funcionalmente cerrada pero con hardening inmediato necesario.

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

---

# Objetivo inmediato de trabajo

Consolidar invariantes del executor antes de expandir features.

---

# Scope permitido actual

Se permite:

* hardening del executor
* control fino de iteraciones
* duplicate prevention correcta
* freshness policy explícita
* cleanup estructural menor

---

# Scope no permitido actual

No abrir:

* nuevas skills
* provider abstraction
* memory semantic redesign

hasta cerrar invariantes de Phase 4.

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
2 verificar si pertenece a Phase 4
3 evitar expansión lateral innecesaria