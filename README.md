# OpenGravity

OpenGravity is a local-first conversational AI agent written in Rust, designed around explicit execution control, deterministic orchestration, and bounded tool usage.

It currently operates primarily through Telegram and emphasizes runtime clarity over hidden agent autonomy.

---

# Core Principles

- explicit execution over implicit agent behavior
- local-first state persistence
- bounded tool invocation
- deterministic runtime transitions
- strict validation discipline

---

# Current Runtime Architecture

OpenGravity separates planning, execution, memory extraction, and response generation into explicit runtime stages.

```mermaid
flowchart TD
    A[User Input] --> B[Planner]
    B --> C[Pending Plan]
    C --> D[Executor]
    D --> E[Tool or LLM Response]
    E --> F[Memory Extraction]
    F --> G[State Commit]
````

The Rust runtime remains the execution authority.

The language model supports semantic generation but does not control orchestration.

Detailed architecture:

* `docs/ARCHITECTURE.md`

---

# Current Capabilities

* local conversation persistence using SQLite
* Telegram-native interaction
* explicit pending plan execution
* bounded tool invocation
* provider fallback handling

---

# Documentation

Technical references:

* `docs/ARCHITECTURE.md`
* `docs/KNOWN_FRAGILITIES.md`
* `docs/PHASES.md`
* `docs/adr/`

Project evolution:

* `ROADMAP.md`
* `RELEASE.md`

Contribution rules:

* `CONTRIBUTING.md`

---

# Installation

1. Install Rust via [https://rustup.rs/](https://rustup.rs/)
2. Clone the repository

---

# Configuration

Copy `.env.example` to `.env`

```bash
cp .env.example .env
```

Required:

* Telegram bot token
* allowed Telegram user IDs
* provider credentials

Ensure:

`TELEGRAM_ALLOWED_USER_IDS`

contains your Telegram numeric user ID list.

---

# Running

```bash
cargo build --release
cargo run --release
```

---

# Validation

Before committing, run:

```bash
./scripts/check.sh
```

This enforces:

* `cargo fmt`
* `cargo clippy -- -D warnings`
* `cargo test`
* `cargo nextest run`
* `cargo tarpaulin --fail-under 95`
* `cargo audit`
* `cargo deny check`

---

# CI Discipline

GitHub Actions validates:

* formatting
* warnings
* tests
* coverage
* dependency safety

---

# Current Focus

The current architectural focus is:

* executor hardening
* skill formalization
* stronger execution contracts

See:

* `docs/PHASES.md`
