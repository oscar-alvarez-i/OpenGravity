# E2E Test Scenarios — Phase 2

Behavioral contract for end-to-end agent execution in OpenGravity.

---

## Escenario 1 — Write + Read básico

Status: PASS

Input:
- Guardá "hola mundo"
- Leé las notas

Expected:
- Primera ejecución persiste contenido en archivo local
- Segunda ejecución retorna contenido completo del archivo
- Ambas respuestas son correctas y consistentes

---

## Escenario 2 — Escritura duplicada (idempotencia)

Status: PASS

Input:
- Guardá "hola mundo"
- Guardá "hola mundo"

Expected:
- Primera ejecución escribe contenido en archivo
- Segunda ejecución es bloqueada por idempotencia
- No se duplica contenido en archivo
- Sistema retorna mensaje de idempotencia al usuario

---

## Escenario 3 — Escritura con input distinto

Status: PASS

Input:
- Guardá "hola mundo"
- Guardá "hola"

Expected:
- Ambas ejecuciones deben ejecutarse
- Ambas notas deben persistirse en archivo
- No debe activarse idempotencia (inputs diferentes)

---

## Escenario 4 — Tool AlwaysFresh (hora)

Status: PASS

Input:
- ¿Qué hora es?
- ¿Qué hora es?

Expected:
- get_current_time se ejecuta en ambas peticiones
- Nunca se bloquea por idempotencia
- Siempre retorna valor actualizado (no reutiliza resultado previo)

---

## Escenario 5 — Loop interno no duplica tool

Status: PASS

Input:
- Guardá "hola mundo"

Expected:
- write_local_note se ejecuta una sola vez dentro del loop
- Iteración siguiente consume resultado de tool
- No re-ejecuta tool en misma conversación

---

## Escenario 6 — Tool failure handling

Status: PASS

Input:
- Guardá "" (input vacío inválido)

Expected:
- Sistema retorna error explícito de validación
- Loop no continúa
- No se reporta success falso

---

## Escenario 7 — Regresión de contexto (bug detectado)

Status: PASS

Input:
- Guardá "hola mundo"
- Guardá "hola"

Expected:
- Segunda ejecución debe disparar write_local_note
- NO debe responder con fallback genérico
- NO debe perder intención de tool por contexto incompleto

---

## Escenario 8 — Guardrail AlwaysFresh

Status: PASS

Input:
- ¿Qué hora es? (LLM no emite TOOL call)

Expected:
- Sistema fuerza ejecución de get_current_time
- No depende del comportamiento del LLM para invocar tool
- Loop continúa normalmente con resultado de tool

---

## Purpose

These scenarios define the expected end-to-end behavior of the agent loop and tool execution system.

They are used as:
- Regression safety net
- Behavioral contract for future phases
- Reference for integration test coverage

---

*Document Version: 1.0*
*Phase: 2.10 Closure (HITO 2)*
