# OpenGravity

OpenGravity is a production-grade, local-first AI agent written entirely in Rust, interacting primarily through Telegram. 
It guarantees zero-trust arbitrary tool executions, enforces strict whitelist barriers, and has immutable zero-warning, 95%+ coverage rules.

## Features
- **Local-first Memory Persistence:** SQLite stores conversational context safely without third-party exfiltration.
- **Telegram Native:** Long polling strategy ensuring real-time responsiveness.
- **Strict Security:** Whitelist-only access. Zero-trust tool execution strategy.
- **Fail-safe LLM Orchestration:** Queries Groq primarily, safely falling back to OpenRouter on timeouts/5xx.

## Installation

1. Install Rust via [rustup](https://rustup.rs/).
2. Clone this repository.

## Configuration

Copy `.env.example` to `.env` and fill the variables:
```bash
cp .env.example .env
```

Ensure `TELEGRAM_ALLOWED_USER_IDS` is filled out with your distinct Telegram Numeric ID (comma separated).

## Running the Agent

```bash
# Build binary
cargo build --release

# Run locally
cargo run --release
```

## Testing & Quality Assurance

Before making any commits, you must run the strict validation suite:

```bash
./scripts/check.sh
```

This enforces:
- `cargo fmt`
- `cargo clippy` (with zero warnings)
- `cargo test` and `cargo nextest run`
- `cargo tarpaulin --fail-under 95` (95% code coverage minimum)
- `cargo audit` and `cargo deny check` (Zero security debt and OSI-approved licenses)

## Continuous Integration
Pushing to the repo runs GitHub Actions (`ci.yml`) guarding the main branch perfectly:
1. `fmt`
2. `clippy -- -D warnings`
3. `cargo test` / `cargo nextest`
4. `cargo tarpaulin --fail-under 95`
5. `cargo audit`
6. `cargo deny`
