# OpenGravity Roadmap

## Propósito

Definir evolución de OpenGravity después de v1.0 manteniendo:

- deterministic-first architecture
- bounded runtime state
- low-resource execution
- auditability
- minimal hidden behavior

OpenGravity debe seguir siendo ejecutable en hardware reducido (ejemplo: Raspberry Pi 4) sin depender de infraestructura pesada.

---

# Regla estratégica global

Toda nueva capacidad debe cumplir:

1. no romper executor invariants
2. no introducir subsistemas opacos
3. no exigir always-on heavy inference
4. mantener degradación controlada en hardware pequeño

---

# Estado actual

## Version actual estable

v1.0.x

## Base cerrada

- deterministic executor estable
- tool registry estable
- memory persistence estable
- planner mínimo estable
- observability mínima activa
- plugin boundary estable

v1.x queda cerrada como núcleo confiable.

---

# Versioning policy

## Patch version (1.0.x)

Usar cuando:

- bugfix local
- regression fix
- runtime hardening sin nueva capacidad
- documentation sync

## Minor version (1.x.0)

Usar cuando:

- nueva capacidad controlada
- nuevo módulo bounded
- nueva tool family
- nueva observability layer
- nueva integración externa estable

## Major version (2.0.0)

Usar solo cuando:

- cambia el alcance funcional del agente
- OpenGravity pasa de agente core a agente operativo personal multi-domain

---

# Roadmap estructurado

---

# HITO 2 — Personal Execution Layer

## Objetivo

Permitir que OpenGravity ejecute tareas personales reales localmente.

## Phase actual

Phase 2.1: CLOSED (write_local_note tool)

## Capacidades objetivo

- calendar actions
- reminders
- cron orchestration
- local file tasks
- safe script execution

## Ejemplos

- crear evento de calendario
- invitar contactos conocidos automáticamente
- disparar tareas periódicas locales

## Restricción

No introducir autonomía libre.

Toda ejecución debe pasar por tools explícitas.

## Versión esperada

1.1.x

---

# HITO 3 — File Intelligence Layer

## Objetivo

Organización local de contenido personal.

## Capacidades objetivo

- ordenar fotos
- ordenar videos
- detectar duplicados
- clasificar música
- clasificar películas y series

## Fuentes soportadas

- discos locales
- NAS
- Google Photos exportado
- librerías mixtas

## Restricción

Primero metadata.

No visión pesada como dependencia base.

## Versión esperada

1.2.x

---

# HITO 4 — Safe Code Generation Layer

## Objetivo

Permitir que OpenGravity construya utilidades locales pequeñas.

## Capacidades objetivo

- generar scripts
- ejecutar scripts sandboxed
- validar output
- registrar artifacts

## Ejemplos

- script para renombrar archivos
- script para consolidar backups
- script para convertir formatos

## Restricción

Todo código generado debe ser:

- auditable
- bounded
- reversible

## Versión esperada

1.3.x

---

# HITO 5 — Observability Full Runtime

## Objetivo

Dashboard operativo local.

## Capacidades objetivo

- CPU
- RAM
- tokens
- tool latency
- active jobs
- progress tracking

## Restricción

Dashboard liviano.

No dependencia cloud obligatoria.

## Versión esperada

1.4.x

---

# HITO 6 — Multi-Agent Controlled Audit

## Objetivo

Subagentes auditores paralelos.

## Capacidades objetivo

- auditor de plan
- auditor de tool result
- auditor de código generado

## Modelo

Agentes secundarios nunca ejecutan.

Solo validan.

## Restricción

No crear loops autónomos entre agentes.

## Versión esperada

1.5.x

---

# HITO 7 — Personal Semantic Memory

## Objetivo

Memoria personal estructurada real.

## Capacidades objetivo

- relaciones familiares
- preferencias recurrentes
- reglas sociales
- patrones personales

## Ejemplo

"turno médico de mi hijo" →

- detectar hijo
- detectar madre
- invitar automáticamente a esposa

## Restricción

Memory con keys explícitas.

No memoria libre no auditada.

## Versión esperada

1.6.x

---

# HITO 8 — External Connectors Stable

## Objetivo

Integraciones reales externas.

## Conectores objetivo

- Google Calendar
- Google Photos
- Drive
- local mail
- messaging

## Restricción

Conector = tool bounded.

Nunca acceso libre directo.

## Versión esperada

1.7.x

---

# HITO 9 — Personal Autonomous Operations

## Objetivo

Delegación real controlada.

## Capacidades

- jobs nocturnos
- housekeeping automático
- revisión periódica

## Restricción

Siempre:

- bounded
- observable
- interruptible

## Versión esperada

2.0.0

---

# Regla crítica para todos los hitos

Cada hito se abre como:

Phase 1 nueva

y se cierra phase por phase hasta completar acceptance total.

Nunca abrir dos hitos simultáneamente.

---

# Criterio de prioridad futura

Orden recomendado inmediato:

1. Personal Execution Layer
2. File Intelligence Layer
3. Safe Code Generation Layer

porque construyen utilidad real inmediata sin romper simplicidad.

---

# Scope prohibido mientras tanto

No abrir todavía:

- vision heavy inference local
- voice always-on
- distributed orchestration
- autonomous internet browsing libre
- heavy vector infra obligatoria

---

# Principio final

OpenGravity no busca competir por complejidad.

Busca:

máxima confiabilidad con mínima arquitectura.