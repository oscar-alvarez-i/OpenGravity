# OpenGravity - Contrato Interno y Políticas de Desarrollo

Este archivo define las reglas inmutables de ingeniería, seguridad, evolución, testing y CI para el proyecto OpenGravity.
Su cumplimiento es OBLIGATORIO en cada iteración de desarrollo.

## 0. Antigravity Guidance
Future Antigravity sessions MUST consult the `.agents/` directory (rules, workflows, skills, decisions) before starting major work or architectural changes.

## 1. Reglas Permanentes de Ingeniería
* Nunca introducir warnings de compilación ni de linter (Clippy).
* Nunca introducir deuda técnica (TODOs, FIXMEs o implementaciones temporales).
* Nunca bajar la cobertura de tests por debajo del mínimo exigido (92%).
* Nunca romper el pipeline de CI.
* Toda nueva "tool" del agente IA requiere sus correspondientes unit tests y validación de seguridad.
* Toda nueva dependencia (`crate`) debe justificarse funcionalmente.
* Toda nueva `crate` debe evaluarse por seguridad antes de incluirse.
* Toda nueva "feature" debe respetar los "boundaries" (límites) arquitectónicos existentes.

## 2. Policy de Seguridad
* **Zero trust tool execution:** Toda tool debe registrarse explícitamente y sus inputs deben validarse rigurosamente.
* **Deny by default:** Las tools futuras están denegadas por defecto y requieren autorización explícita para implementarse.
* **No shell execution arbitraria:** Prohibido ejecutar comandos de shell de forma arbitraria para evitar shell injection.
* **No acceso a filesystem fuera de boundaries explícitos:** Limitar operaciones I/O a rutas específicas (ej. memoria SQLite).
* **No exposición accidental de secretos:** Nunca imprimir, parsear de vuelta o loguear variables de entorno sensibles (tokens, API keys, credenciales).

## 3. Policy de Evolución
* **Backward compatibility obligatoria:** Las nuevas implementaciones no deben romper el funcionamiento previo.
* **Refactor seguro antes de agregar complejidad:** Mantener el código base simple y refactorizar si es necesario antes de sumar capas.
* **Evitar crates innecesarias:** Mantener el `Cargo.toml` minimalista y auditable.
* **Priorizar simplicidad mantenible:** Implementar soluciones limpias e idomáticas en Rust en lugar de sobreingeniería.

## 4. Policy de Testing
* **Todo módulo nuevo requiere unit tests:** Validando edge cases y happy paths.
* **Toda integración nueva requiere integration tests:** Especialmente las que tocan SQLite, Telegram y LLM.
* **Toda corrección de bug requiere regression test:** Para evitar que el bug reaparezca en el futuro.

## 7. Validation Execution Policy

- During iterative development, only `./scripts/check_fast.sh` may be used.
- Full validation must NOT run after every small edit.
- `./scripts/check_full.sh` is mandatory only when:
  - a feature is complete
  - architectural refactor is complete
  - dependencies changed
  - before commit
- Avoid unnecessary terminal output to minimize AI token consumption.
- Prefer targeted checks when editing isolated modules:
  - `cargo test module_name -q`
  - `cargo clippy --lib -q`
- Prefer cargo aliases when possible to reduce command verbosity and AI token consumption.

## 8. AI Development Workflow Rules

- Never execute full validation repeatedly during small fix loops.
- Prefer narrow-scope validation before global validation.
- If only one module changes, validate only impacted module first.
- Full pipeline is reserved for stabilization stage.
- Minimize output verbosity whenever possible.
- Never use coverage tools (`tarpaulin`) during micro-iterations unless tests changed.

## 9. Policy de Dependencias

- No se aceptan múltiples versiones mayores de crates críticas sin justificación documentada.
- Toda crate de red debe auditar impacto TLS y superficie HTTP.
- Se debe inspeccionar periódicamente:
  `cargo tree | grep reqwest`
  `cargo tree | grep hyper`
  `cargo tree | grep rustls`

## 10. Known Controlled Dependency Debt

`teloxide` currently introduces `reqwest` 0.11 + `hyper` 0.14 + `native-tls` transitively.

This is accepted temporarily because:

- Telegram transport remains isolated
- No direct OpenSSL usage exists in OpenGravity code
- Migration will be evaluated when `teloxide` updates transport stack