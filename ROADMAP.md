# OpenGravity Roadmap — Phases 2 to 6

## Objective

OpenGravity should evolve into a robust personal agent platform inspired by the category of systems like OpenClaw, but with its own architecture, priorities, and operational philosophy:

* deterministic core behavior
* strong regression safety
* modular tool orchestration
* durable conversational memory
* explicit workflow execution
* production-grade observability
* clean extensibility for future multi-agent evolution

The immediate strategic rule is:

> **No new capability should be added unless current behavior is protected by tests and observability.**

That means every phase must leave behind:

* stable contracts
* regression tests
* measurable behavior
* explicit boundaries

---

# Phase 2 — Structured Skills Layer

## Goal

Introduce reusable cognitive skills without breaking baseline loop determinism.

This phase creates the first abstraction above raw tool execution.

The agent must stop being only:

`LLM -> tool -> answer`

and become:

`LLM -> skill selection -> controlled execution -> answer`

---

## Phase 2 Deliverables

### 2.1 Skill abstraction

Create explicit skill contract:

* skill name
* trigger conditions
* input schema
* execution contract
* output normalization

Suggested structure:

```text
src/skills/
  mod.rs
  trait.rs
  planner.rs
  memory.rs
  summarizer.rs
```

---

### 2.2 First production skills

## memory_extraction_skill

Responsible for deciding:

* what should be remembered
* what should be ignored
* persistence priority

Must distinguish:

* stable user facts
* transient facts
* irrelevant conversation noise

---

## planner_skill

Transforms vague intent into executable subtasks.

Example:

User:
"Organizame mañana para terminar el proyecto"

Planner returns:

* infer objective
* split subtasks
* produce ordered execution plan

---

## summarization_skill

Compress long context when token pressure grows.

Must preserve:

* factual continuity
* unresolved tasks
* active preferences

---

### 2.3 Skill registry

Avoid hardcoded branching.

Implement:

* registry lookup
* explicit skill dispatch
* deterministic fallback

---

### 2.4 Prompt contract split

Separate prompts into:

* system identity
* tool rules
* skill rules
* memory rules

Suggested structure:

```text
src/prompts/
  system.md
  tools.md
  memory.md
  skills.md
```

---

### 2.5 Logging upgrade

Every skill execution must log:

* selected skill
* input
* output
* duration
* fallback path

---

### 2.6 Professionalization tasks

## Add CI pipeline

Required checks:

* cargo fmt
* cargo clippy
* cargo test
* regression suite

---

## Add architecture decision records

Create:

```text
/docs/adr/
```

Each major design decision documented.

---

## Introduce semantic module boundaries

No feature should grow directly inside agent loop.

---

---

# Exact Test Matrix — Phase 2

## Skill Core Tests

### test_skill_registry_selects_expected_skill

Validate exact dispatch.

---

### test_unknown_skill_falls_back_cleanly

No panic.
No hidden execution.

---

### test_skill_output_normalization

All skills must emit normalized message contract.

---

## Memory Extraction Tests

### test_memory_extracts_stable_fact_only

Input:

"Mi color favorito es azul"

Persist expected.

---

### test_memory_ignores_transient_fact

Input:

"Hoy tomé café"

No persistence expected.

---

### test_memory_no_duplicate_fact

Repeated same fact.

Single stored record.

---

### test_memory_update_existing_fact

Blue -> Green

Must replace previous stable fact.

---

## Planner Tests

### test_planner_generates_ordered_subtasks

---

### test_planner_handles_ambiguous_goal_without_tool

---

### test_planner_does_not_invent_external_state

---

## Workflow Tests

### test_skill_then_tool_chain

Planner -> tool -> answer

---

### test_memory_then_tool_then_recall

Store fact -> tool -> recall

---

### test_two_skill_chain_same_turn

Skill A -> Skill B -> final answer

---

## Long Context Safety Tests

### test_summary_preserves_user_fact

---

### test_summary_preserves_pending_task

---

### test_summary_not_polluted_by_tool_reasoning

---

## Regression Safety

### test_baseline_tool_loop_still_stable_after_skill_layer

Mandatory.

---

### test_start_command_still_excluded

Mandatory.

---

---

# Phase 3 — Workflow Engine

## Goal

Move from isolated skills to explicit execution workflows.

The agent must execute structured chains intentionally.

---

## Phase 3 Deliverables

### 3.1 Workflow abstraction

Workflow contract:

* workflow id
* steps
* branching rules
* failure rules

---

### 3.2 Workflow executor

Separate from main loop.

Suggested:

```text
src/workflows/
```

---

### 3.3 First workflows

## factual lookup workflow

question -> tool -> validate -> answer

---

## memory enrichment workflow

input -> extract -> persist -> answer

---

## planning workflow

goal -> decomposition -> output plan

---

### 3.4 Failure policy

Workflow must stop safely when:

* tool fails
* invalid output
* repeated loop

---

### 3.5 Retry contract

Controlled retries only.

Never hidden recursion.

---

### 3.6 Workflow observability

Log each workflow step.

---

### 3.7 Workflow tests

## test_workflow_exact_step_order

## test_workflow_partial_failure_safe_exit

## test_workflow_retry_limit

## test_workflow_no_context_duplication

## test_workflow_preserves_memory_integrity

---

---

# Bird's-eye View — Later Phases

# Phase 4 — Multi-tool Orchestration

* multiple tool contracts
* tool priority rules
* tool conflict resolution
* tool output arbitration

---

# Phase 5 — Durable Long-Term Memory

* semantic memory indexing
* retrieval ranking
* contradiction handling
* memory aging strategy

---

# Phase 6 — Production Hardening

* metrics
* health endpoints
* structured tracing
* latency dashboards
* error taxonomies

---

# Phase 7 — External Interfaces

* API layer
* CLI mode
* web UI
* remote execution support

---

# Phase 8 — Agent Autonomy Expansion

* scheduled workflows
* deferred tasks
* proactive reminders
* multi-session objective continuity

---

# Non-Negotiable Rule for All Future Phases

Before closing any phase:

## required

* tests green
* manual real Telegram validation
* logs reviewed
* no hidden context drift

---

# Recommended Permanent Validation Command

```bash
cargo fmt
cargo clippy
./scripts/test-baseline.sh
cargo test -- --nocapture
```

---

# Recommended Repository Additions Now

```text
/docs/
/docs/adr/
/scripts/
/prompts/
/tests/fixtures/
```

---

# Strategic Principle

OpenGravity should grow as a controlled platform, not as accumulated features.

Every new capability must either:

* strengthen architecture
  nor
* remain isolated enough to remove safely.
