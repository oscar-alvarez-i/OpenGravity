# OpenGravity — Development Protocol

## Purpose

This document defines the permanent execution protocol for developing **OpenGravity** across future chats, phases, milestones, and versions.

Its goal is to prevent drift, reduce hallucination risk, and preserve architectural discipline while working across multiple LLM sessions.

It is intentionally compact, operational, and machine-readable for both human and AI continuity.

---

# 1. Core Development Principle

OpenGravity evolves through **strictly isolated phases**.

Each phase must:

* solve exactly one bounded objective;
* avoid hidden side effects;
* preserve backward stability;
* leave explicit documentary trace.

A phase is never considered complete only because code compiles.

A phase closes only when:

* code behavior is verified;
* architecture remains coherent;
* documentation is updated;
* next dependency boundary is clear.

---

# 2. Execution Workflow Per Phase

## Step A — Architectural Definition

Before coding:

* define exact target;
* define impacted files;
* define non-goals;
* define risk surface.

No coding begins if scope is ambiguous.

---

## Step B — External Coding Pass (Minimax or equivalent)

Implementation is delegated externally when desired.

Required output format:

* exact files modified;
* minimal diff;
* rationale;
* no speculative extra refactors.

---

## Step C — OpenGravity Audit (ChatGPT)

Every external implementation must be audited for:

* architectural correctness;
* hidden coupling;
* future extensibility;
* unnecessary complexity;
* protocol violations.

Audit happens before commit.

---

## Step C.1 — External Code Review (Opencode / Codex)

Before closing a phase, an additional independent code review must be performed.

This review must:

- analyze full diff of the phase
- detect hidden bugs
- detect unintended side effects
- validate test coverage adequacy
- highlight potential dead code or drift

Accepted reviewers:

- Opencode
- Codex
- equivalent static/code review system

---

### Rule

A phase MUST NOT be closed unless:

- external implementation audit is completed (Step C)
- independent code review is completed (Step C.1)

---

## Step D — Documentation Closure

At phase close:

update only affected documents.

Never update documentation speculatively.

---

## Step E — Commit Closure

Each closed phase ends with one explicit commit.

Commit format:

`phase XX: short technical description`

Example:

`phase 21: isolate provider execution contract`

---

# 3. Mandatory Documentation Hierarchy

## Permanent Sources (max critical set)

These files form the stable reasoning base:

1. `OpenGravity-Active-State.md`
2. `OpenGravity-Phase-Gates.md`
3. `OpenGravity-Roadmap.md`
4. `OpenGravity-Dev-Protocol.md`
5. `OpenGravity-Debug-Discipline.md`

---

## Optional Temporary Files

Temporary files may exist during specific milestones, but should be removed when absorbed.

Example:

* patch templates;
* temporary migration notes;
* experimental branch notes.

---

# 4. Source Discipline Rules

Sources must remain:

* concise;
* non-redundant;
* factual;
* version-aware.

Avoid narrative text.

Avoid historical duplication.

If a file becomes obsolete, merge or remove it.

---

# 5. Versioning Protocol

## Patch (`1.0.x`)

Use when:

* fixing internal behavior;
* improving reliability;
* no capability expansion.

---

## Minor (`1.x.0`)

Use when:

* adding bounded new capability;
* preserving architecture;
* no conceptual platform shift.

---

## Major (`2.0.0`)

Use when:

* execution model changes;
* memory model changes;
* orchestration model changes;
* multi-agent substrate becomes native.

---

# 6. Milestone Logic

A milestone contains many phases.

A milestone closes only when a usable capability exists end-to-end.

Example:

* not only parser;
* but parser + execution + observability + recovery.

---

# 7. Simplicity Constraint

OpenGravity must remain deployable on constrained hardware.

Target baseline:

* Raspberry Pi 4 class device;
* low idle memory;
* bounded concurrency;
* graceful degradation.

Any new subsystem must justify resource cost.

---

# 8. Anti-Drift Rule

Never introduce:

* framework expansion without necessity;
* abstraction before proven need;
* hidden daemon behavior;
* implicit persistent state.

---

# 9. Before Opening a New Chat

Always ensure current chat leaves:

* active phase status explicit;
* documentation updated;
* next phase target clear;
* no unresolved architectural ambiguity.

New chat must start from Sources only.

No dependency on previous conversational memory.

---

# 10. Long-Term Objective

OpenGravity should evolve into a:

**lightweight general-purpose personal autonomous agent with auditable execution and local-first trust model**.

Core traits:

* local-first;
* inspectable;
* deterministic where possible;
* extensible through skills;
* observable;
* safe under limited hardware.

---

# 11. Rule for Every Future Decision

If two solutions work:

choose the one that:

* is simpler to audit;
* easier to explain;
* cheaper to run;
* easier to reverse.

---

# 12. Merge Gate

PR merge is allowed when:

* CI checks pass
* external audit completed
* no blocking architectural findings remain
* affected documentation is synchronized when scope requires it

For single-maintainer operation, explicit approving review may be bypassed only when technical gates above are satisfied.

---

# 13. External AI Roles

## Minimax

* bounded implementation
* minimal diff generation

## Codex

* preferred structural review before merge when runtime or architecture changes

## ChatGPT

* architectural arbitration
* scope control
* closure validation

---

# 14. Branch Model

## Phase Branching Rule

Every phase MUST be developed in a dedicated branch.

### Naming convention

```

phase/<phase-number>-<short-description>

```

Example:

```

phase/2.2-tool-execution-layer

```

### Operational rules

- No direct work on main
- main is updated ONLY via PR
- Each phase branch maps to exactly one phase
- No mixing multiple phases in one branch
- Phase closes → branch merged → branch can be deleted

### PR requirements

Each phase must produce:
- one PR
- passing CI
- audited changes
- documentation aligned before merge

---

## main

* stable only

## phase/*

* active functional phases

## infra/*

* transversal repository changes

## hotfix/*

* bounded corrective patches

---

## Phase Kickoff Checklist

Before starting any new phase:

- Create phase branch (phase/X.Y-name)
- Verify Active-State reflects previous phase closure
- Define phase scope BEFORE coding
- Validate no unresolved structural debt exists
- Confirm documentation baseline is consistent

No implementation begins without completing this checklist.

---

## Phase Closure Checklist

A phase can be closed only if:

- Code implemented and audited
- Tests passing
- No structural issues detected
- Documentation updated (Phase-Gates + others if needed)
- Known limitations explicitly documented

If any item is missing → phase remains OPEN

---

## Legacy Code Rule

Temporary coexistence of old and new implementations is allowed ONLY if:

- explicitly documented in Phase-Gates
- marked as temporary
- has a clear future removal phase

Legacy code must never become implicit or permanent.
