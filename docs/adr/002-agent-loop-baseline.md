# ADR 002: Agent Loop Baseline

## Status
Accepted

## Context
The core of OpenGravity is the `AgentLoop`, which manages the interaction between the user, the LLM, and tools. This loop must be robust, deterministic, and prevent runaway execution.

## Decision
We define the following baseline rules for the Agent Loop:

### Max Iterations
The `AgentLoop` must have a hard-coded maximum iteration limit (currently 3). This prevents infinite loops in case of LLM reasoning failure or tool call repetition.

### Tool Persistence
All tool execution results must be persisted as `Tool` messages. This ensures that the state is recorded and available for future turns, even if the current turn fails.

### Transport-Level Command Interception
The `/start` command is handled at the transport level (e.g., in `telegram.rs`). This is an initialization signal and not a part of the core agent logic, ensuring the loop remains focused on conversation.

### Deterministic Execution
The baseline loop follows a strict sequential process:
1. Load context from DB.
2. Build system prompt.
3. LLM Generate.
4. Parse for `TOOL:` call.
5. If TOOL found:
    - Execute Tool.
    - Loop back to step 3 (adding Tool result to context).
6. If no TOOL:
    - Persist Assistant response.
    - Return result.

Reasoning preceding a `TOOL` call is explicitly NOT persisted and NOT added to the active context of subsequent steps in the same loop iteration or future turns.

## Consequences
- Protects against cost and rate-limit exhaustion.
- Simplifies debugging by making the loop's state transitions predictable.
- Provides a stable foundation for Phase 2 (Skills and Workflows).
