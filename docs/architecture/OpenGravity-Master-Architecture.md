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

## ToolRegistry

ToolRegistry es autoridad única para:

- metadata de tools
- freshness policy por tool
- execution dispatch

Executor no contiene dispatch por tool.

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

# Runtime Context Compression

Antes de enviar contexto al modelo, executor aplica compresión semántica controlada.

## Tool compression

Bloques consecutivos de mensajes `Tool` conservan solo el último resultado.

## Memory compression

Para mensajes:

* MEMORY_SET
* MEMORY_UPDATE
* MEMORY_DELETE

se conserva solo la última ocurrencia por key.

## Garantía

La compresión nunca altera ordering lógico entre:

* user
* assistant
* tool
* system

excepto remoción de estados obsoletos.

## Restricción

Compression no puede introducir reinterpretación semántica.

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

Facts equivalentes deben actualizarse semánticamente cuando corresponda.

### Semantic overwrite

Cuando el mismo fact_key se emite nuevamente (ej: "mi color favorito es verde" reingresado):

* Se detecta fact_key existente en storage
* Se actualiza el valor existente (last-write-wins)
* No se crea duplicado

Implementación: save_memory_before_update verifica fact_key antes de INSERT/UPDATE.

### Bounded Retrieval

Retrieval usa budgetes separados determinísticos:

- conversation: últimos 6 mensajes (fetch_conversation_only)
- persistent memory: scan 20 → filter MEMORY_* → take 4 (fetch_memories_only)

Merge order: memories primero, luego conversation.

Esta separación evita que conversation history desplace memories recientes del prompt.

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
