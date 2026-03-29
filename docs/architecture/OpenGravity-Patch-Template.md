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

# Verificación previa obligatoria

Antes de crear el patch:

1 **Verificar tests existentes**

- Ejecutar `cargo test` para ver coverage actual
- Si el módulo tiene tests,必须在现有测试基础上添加新测试
- No crear tests nuevos si los existentes ya cubren el escenario

2 **Verificar estado nuevo**

- Si el patch introduce estado nuevo (struct, campo, DB column):
  - Debe tener bounded semantics explícito
  - Debe documentar lifecycle completo
  - No debe expandir sin bound definido

3 **Verificar scope de phase activa**

- Phase 16 (OPEN): Autonomous Agent Safety Layer
- Phase 17 (OPEN): Functional Contract Audit v1.0
- El patch debe estar dentro del scope de phase activa
- Si el cambio sale del scope, marcar como "out of scope for current phase"

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

## Tests verification

- [ ] Tests existentes ejecutados y pasan
- [ ] Nuevos tests cubren el comportamiento modificado
- [ ] No se reduce coverage existente

## State boundedness

- [ ] Si se introduce estado nuevo, tiene bound definido
- [ ] Si se modifica estado existente, no se rompe invariant

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