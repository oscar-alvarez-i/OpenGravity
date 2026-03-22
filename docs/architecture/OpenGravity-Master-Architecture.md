# OpenGravity Master Architecture

## Proyecto

OpenGravity es un agente conversacional en Rust con arquitectura deterministic-first.

Objetivo principal:

construir un agente extensible, auditable y controlado, evitando comportamiento opaco típico de agentes puramente LLM-driven.

---

# Principio arquitectónico central

Toda lógica crítica debe resolverse primero por componentes determinísticos.

LLM solo interviene cuando:

* ninguna skill resolvió completamente
* no existe pending_plan ejecutable
* planner no generó resolución suficiente

---

# Orden obligatorio del executor

El orden de ejecución es invariante arquitectónica:

1 skill
2 pending_plan
3 planner
4 llm
5 tool execution

Este orden no debe alterarse sin justificación arquitectónica fuerte.

---

# Skills

Las skills son módulos deterministic-first pre-LLM.

## Propósito

Resolver patrones de intención previsibles sin delegar al modelo.

## Reglas

* skill nunca debe bloquear planner
* skill nunca debe alterar tool execution directamente
* skill debe producir outputs explícitos

## Estado actual base

### MemoryExtractionSkill

Responsabilidades:

* detectar facts persistentes
* ignorar transient facts
* emitir MemoryOperation::Set

---

# Planner

Planner debe detectar intención multi-step.

## Responsabilidades

* fragmentar intención
* generar pasos ejecutables
* persistir pending_plan mínimo

## Restricción

pending_plan solo puede contener remaining executable steps.

Nunca pseudo-steps artificiales.

---

# pending_plan

pending_plan es estado transitorio mínimo.

## Debe contener

solo próximos pasos realmente ejecutables.

## No aceptable

* estado decorativo
* pasos duplicados
* fragmentación innecesaria

---

# Tool execution

Tool execution es determinístico.

## Regla

Las tools nunca deben ejecutarse libremente desde texto no controlado.

## Duplicate prevention permitido

pero distinguiendo:

### cacheable tools

pueden reutilizar resultado bajo reglas explícitas

### non-cacheable tools

deben ejecutar fresh siempre

## non-cacheable obligatorio actual

* get_current_time

---

# Memory persistence

Backend oficial actual:

SQLite

## Restricciones

* persistencia estable entre sesiones
* evitar cambios de storage sin necesidad fuerte

## Memory policy

Persistir facts útiles de largo plazo.

No persistir transient user requests.

---

# LLM policy

LLM no define arquitectura.

LLM complementa resolución cuando deterministic layer no resuelve suficiente.

## No aceptable

* usar LLM para bypass de executor
* usar LLM para simular planner
* usar LLM para compensar bugs estructurales

---

# Seguridad de cambios

Antes de modificar código:

1 identificar módulo exacto
2 minimizar superficie de cambio
3 evitar refactors innecesarios

---

# Testing obligatorio

Siempre ejecutar:

cargo fmt

cargo clippy --all-targets --all-features -- -D warnings

cargo test

---

# Regla de evidencia

Cuando logs contradicen explicación previa:

logs tienen prioridad.

Cuando código contradice hipótesis:

código tiene prioridad.

---

# Política de evolución

Aceptable:

* hardening
* cleanup
* reducción de estado artificial
* mejor separación determinística

No aceptable sin aprobación explícita:

* refactor masivo
* cambio de executor order
* reemplazo de planner
* cambio de backend memory

---

# Regla documental

En nuevos chats:

1 primero inferir phase activa desde Active State
2 luego validar contra esta arquitectura
3 nunca asumir comportamiento no confirmado por código o logs