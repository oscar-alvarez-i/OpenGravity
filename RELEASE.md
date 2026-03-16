# Release Checklist

This checklist must be successfully completed before any production release of OpenGravity.

## Pre-Release Validation
- [ ] `./scripts/check_full.sh` passes completely (fmt, clippy, tests, nextest, coverage, audit, deny, build).
- [ ] Test coverage is >= 95% (verified by tarpaulin).
- [ ] `cargo audit` returns no security vulnerabilities.
- [ ] `cargo deny check` returns no license or ban violations.

## Code Quality & Security
- [ ] No forbidden `unwrap()`, `expect()`, or `panic!()` in production code (verified by audit).
- [ ] No `TODO` or `FIXME` comments remaining in the `src/` directory.
- [ ] Architecture boundaries are respected (no business logic in `bot`, no infra in `domain`).
- [ ] Logging is audited: no secrets, credentials, or unsafe payloads are exposed.
- [ ] All environment variables are explicitly validated during startup.

## Operational Readiness
- [ ] `cargo build --release` succeeds without warnings.
- [ ] Startup robustness verified: missing environment variables cause clear, deterministic failure messages.
- [ ] Contract consistency between LLM adapters (Groq, OpenRouter) is maintained.
