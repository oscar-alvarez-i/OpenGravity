# OpenGravity Production Baseline

This document defines the frozen production-grade state of OpenGravity as of March 2026. This baseline must be preserved during all future development.

## 1. Repository Status & Thresholds
- **Current State**: Production-Grade (Post-Hardening Phase)
- **Code Coverage**: **98.83%** (measured via Tarpaulin)
- **Minimum Release Threshold**: **87.0%**
- **Test Count**: **56 passing tests** (54 unit/internal + 2 security integration)
- **Stability**: Zero compilation/Clippy warnings.

## 2. Accepted Justified Exclusions
The following components are explicitly excluded from coverage requirements using `#[cfg(not(tarpaulin_include))]`:
- `src/main.rs`: High-level orchestration and runtime entry point.
- `src/bot/telegram.rs`: External framework delegation (Teloxide REPL) and thin trait wrappers.

## 3. Accepted Technical Debt & Invariants
- **Teloxide Transitive Stack**: Accepted temporary dependency on `reqwest 0.11` and `hyper 0.14` via `teloxide`.
- **Uncovered Tracing**: Tracing macro extensions (specifically `warn!` paths in `agent/loop.rs`) are accepted as uncovered to avoid test suite over-instrumentation.
- **SQLite Fallback**: The database uses a "User" role fallback for unknown role strings to maintain legacy compatibility.

## 4. Frozen Dependency Baseline
Critical crates and versions used in this baseline:
- `teloxide`: **0.13** (Sensitive: upgrades will impact transport layer)
- `tokio`: **1.37** (Core runtime)
- `reqwest`: **0.12** (Primary network client)
- `rusqlite`: **0.31** (Persistence layer)
- `tracing`: **0.1** (Telemetry)

## 5. Validation Baseline
Always use the following commands to verify compliance with this baseline:

### Iterative (Daily Work)
```bash
./scripts/check_fast.sh
```
*Purpose: Quick feedback loop for unit tests and formatting.*

### Release/Commit (Integrity)
```bash
./scripts/check_full.sh
```
*Purpose: Full audit, license check, and dependency verification before crossing major milestones.*

## 6. Architecture Snapshot

### Module Boundaries
- **`agent`**: Turn-based reasoning, planning, and execution loop logic.
- **`bot`**: Telegram interface and message transport handling.
- **`config`**: Environment configuration and secret management.
- **`db`**: SQLite persistence layer using `rusqlite`.
- **`domain`**: Shared domain models and role definitions.
- **`llm`**: Multi-provider orchestration (Groq primary, OpenRouter fallback).
- **`security`**: Authorization and user ID whitelist enforcement.
- **`tools`**: Function registry and internal capability management.

### Execution Path
1. **Ingress**: Messages received via long-polling (`teloxide`).
2. **Security**: Whitelist validation at the dispatcher level.
3. **Context**: `MemoryBridge` retrieves the last 10 turns from SQLite.
4. **Reasoning**: `AgentLoop` coordinates the planner and orchestrates LLM calls.
5. **Tools**: `Registry` detects tool calls, executes Rust functions, and returns results to context.
6. **Egress**: Final response dispatched back to user via Telegram.

### Persistence Model
- **Store**: SQLite (`memories` table).
- **Strategy**: Captures initial user input, internal reasoning turns (including tool outputs), and final agent responses.
- **Context Window**: Hardcoded to 10 most recent messages for LLM context construction.

### Security Boundaries
- **Network**: Restricted to Telegram API ingress.
- **Access**: Strict User ID Whitelist enforced before reasoning begins.
- **Capabilities**: LLM isolated to registered Rust functions; no raw shell or file system access.
- **Secret Handling**: Sensitive keys use memory-safe wrappers and environment-only sourcing.
