# OpenGravity Patch Task

## Proyecto

OpenGravity (Rust)

Arquitectura deterministic-first.

## Phase activa

[REEMPLAZAR: phase actual exacta]

## Regla arquitectónica obligatoria

No alterar invariantes globales:

1 skill
2 pending_plan
3 planner
4 llm
5 tool execution

No introducir refactor masivo.

No abrir features nuevas.

Solo resolver el problema indicado con patch mínimo.

---

# Objetivo puntual

[REEMPLAZAR: describir en una sola frase el bug o comportamiento a corregir]

---

# Problema observable

[REEMPLAZAR: describir síntoma observable exacto]

---

# Caso reproducible mínimo

[REEMPLAZAR: input real reproducible]

Esperado:

1 [resultado esperado]
2 [resultado esperado]
3 [resultado esperado]

Actual:

[resultado incorrecto actual]

---

# Restricción de cambio

Modificar solo archivos estrictamente necesarios.

Si detectás varias opciones:

elegir la de menor superficie de cambio.

No mover responsabilidades entre módulos salvo necesidad técnica clara.

---

# Proceso obligatorio antes de proponer patch

1 identificar módulo exacto responsable
2 identificar causa raíz concreta
3 verificar que el problema no sea efecto secundario de otro módulo

No asumir.

No expandir scope.

---

# Entregable obligatorio

Responder exactamente en este formato:

## Root cause

(explicación breve y concreta)

## Files to modify

(lista exacta)

## Minimal patch strategy

(pasos concretos)

## Side effects risk

(si existe alguno)

---

# Restricciones de respuesta

No escribir refactor opcional.

No proponer mejoras futuras.

No reestructurar arquitectura.

Solo resolver este caso.

---

# Validación obligatoria

El resultado final debe pasar:

cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test

---

# Si aparece ambigüedad

Elegir hipótesis principal y justificar por qué es la más probable.

No abrir múltiples ramas especulativas.