# OpenGravity Architecture

## Overview

OpenGravity is a local-first Rust conversational agent built around explicit execution boundaries, deterministic orchestration, and strict runtime control.

The system is designed to avoid hidden agent behavior by separating planning, execution, memory extraction, and tool invocation into explicit stages.

The current architecture prioritizes:

- deterministic execution over autonomous drift
- explicit state transitions
- bounded tool execution
- recoverable runtime behavior
- progressive hardening through phase-based evolution

---

# High-Level Runtime Model

The runtime is centered around a controlled execution loop:

```text
User Input
   ↓
Intent Interpretation
   ↓
Planner
   ↓
Pending Plan
   ↓
Executor
   ↓
Tool / LLM / Direct Response
   ↓
Memory Extraction
   ↓
Final State Commit
````

Each stage has strict responsibilities.

---

# Core Runtime Components

## Planner

The planner converts user input into an executable intent representation.

Responsibilities:

* determine whether execution requires direct response or structured steps
* generate bounded pending plans
* avoid speculative branching
* preserve execution simplicity

Planner output must remain lightweight.

It does not execute tools.

It only defines intended next actions.

---

## Pending Plan

Pending plan is the transient execution contract between planner and executor.

It stores:

* ordered execution steps
* tool requirements if needed
* step descriptions
* execution continuity information

Design constraints:

* pending plan is mutable only through executor progress
* plan is consumed step-by-step
* completed steps are removed explicitly

This prevents hidden internal branching.

---

## Executor

Executor is the strict runtime boundary where plans become actions.

Responsibilities:

* execute current step only
* invoke tool when required
* invoke LLM only when required
* preserve execution order
* prevent duplicate step execution

Executor never invents new plans independently.

If a step fails:

* failure remains bounded
* runtime returns controlled fallback behavior

---

## Skill Layer

Skills provide structured reusable execution logic.

A skill may:

* encapsulate deterministic behavior
* expose known execution contracts
* reduce planner complexity

Skills are not free autonomous agents.

They operate under executor control.

Current direction:

* progressively move repeated behaviors into explicit skills
* keep skill boundaries observable

---

## Tool Invocation Boundary

Tool execution is isolated.

Rules:

* tool calls must be explicit
* tool selection must be derived from current step
* tool result must return into executor state before next decision

Tool execution must not silently alter planning state.

This boundary exists to avoid uncontrolled tool loops.

---

## LLM Boundary

The language model is used only where semantic generation is required.

Primary uses:

* planner generation
* direct conversational response
* tool interpretation support
* memory extraction

The LLM is not treated as runtime authority.

Execution authority remains in Rust orchestration.

---

# Memory Pipeline

Memory is processed after response generation.

Pipeline:

```text
Conversation Turn
   ↓
Memory Extraction
   ↓
Candidate Filtering
   ↓
Persistent Memory Commit
```

Memory design goals:

* preserve only relevant durable facts
* avoid transient noise
* avoid speculative user traits

Memory extraction must remain post-response.

It must never interfere with execution flow.

---

# State Model

The runtime maintains explicit state objects.

Current state includes:

* conversation messages
* pending plan
* execution metadata
* memory context

State mutations must remain traceable.

No implicit hidden state transitions are allowed.

---

# Execution Invariants

The architecture relies on several invariants:

## Single active execution path

Only one active execution path may progress at a time.

---

## Tool before continuation

Tool results must be integrated before advancing execution.

---

## No silent replanning inside executor

Executor cannot redefine global intent.

---

## Memory after execution

Memory never changes current turn execution.

---

## Explicit completion

A step must complete before next step begins.

---

# Failure Strategy

Failures are contained rather than masked.

Current strategy:

* bounded fallback
* controlled error surfaces
* no recursive retries without explicit reason

This preserves runtime predictability.

---

# Documentation Strategy

Architecture evolves through:

* phased implementation
* ADR records for major decisions
* explicit technical documentation updates

Primary references:

* `docs/PHASES.md`
* `docs/KNOWN_FRAGILITIES.md`
* `docs/adr/`

---

# Current Architectural Focus

The current focus is strengthening:

* executor reliability
* skill formalization
* tool semantics
* state continuity

Future phases may expand:

* provider abstraction
* stronger skill registry
* richer execution diagnostics

Architecture changes should preserve determinism first.
