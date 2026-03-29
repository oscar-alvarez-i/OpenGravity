# OpenGravity Debug Discipline

## Propósito

Definir cómo diagnosticar bugs en OpenGravity sin expandir scope innecesariamente.

---

# Regla principal

Nunca asumir causa raíz antes de identificar:

1 síntoma observable exacto
2 módulo exacto afectado
3 invariante violada

---

# Orden obligatorio de diagnóstico

## Paso 1

Describir comportamiento observable exacto.

No interpretar todavía.

## Paso 2

Ubicar módulo responsable probable.

## Paso 3

Verificar si el bug es:

* lógica local
* efecto lateral de estado
* consecuencia de ordering

## Paso 4

Solo entonces formular hipótesis principal.

---

# Regla de hipótesis

Elegir una hipótesis principal.

No abrir múltiples ramas especulativas salvo evidencia fuerte.

---

# Prioridad de evidencia

## prioridad 1

logs reales

## prioridad 2

código exacto actual

## prioridad 3

tests reproducibles

## prioridad 4

explicaciones previas

---

# Regla sobre logs

Si log contradice teoría:

descartar teoría.

Nunca defender hipótesis contra evidencia.

---

# Regla sobre fixes

Siempre preferir:

patch mínimo

antes que:

refactor correctivo amplio

---

# Señales de bug estructural

Si un fix exige:

* aumentar iteraciones
* agregar bypass
* duplicar estado

probablemente la causa raíz no fue encontrada.

---

# Regla executor

Cuando el bug ocurre en flujo conversacional:

primero verificar orden:

1 skill
2 pending_plan
3 planner
4 llm
5 tool execution

---

# Regla tools

Cuando falla tool behavior:

verificar primero:

* freshness
* duplicate prevention
* tool result propagation

antes de tocar planner o llm.

---

# Regla memory

Cuando falla persistencia:

verificar primero:

* extraction
* storage
* retrieval

antes de asumir error semántico.

---

# Regla anti-overengineering

No introducir capas nuevas para resolver bug local.

---

# Formato obligatorio para diagnóstico en chats nuevos

Siempre empezar por:

## síntoma
## evidencia
## módulo probable
## hipótesis principal

---

# Regla final

Si todavía no puede explicarse el bug en una frase clara:

todavía no está bien diagnosticado.

---

# Development Protocol (Phase 15)

## Change Classification

Clasificar todo cambio antes de tocar código:

* local bugfix
* runtime hardening
* test-only coverage
* documentation sync
* architectural change

## Scope Gate

Si patch toca más de 2 módulos:
requiere justificar causa raíz única y por qué patch menor no alcanza.

## Definition of Done por tipo de cambio

* **bugfix local**: 1 archivo, test de regresión existente pasa
* **hardening**: coverage aumentado, validaciones pasan
* **docs sync**: diff mínimo, no reescribir masivamente

## Known Limitations Visibility

Toda deuda aceptada debe quedar explícita en docs.

## Closure Rule

Una phase no cierra si:

* hay deuda no visible
* hay test gap conocido no explicitado
* requiere explicación defensiva