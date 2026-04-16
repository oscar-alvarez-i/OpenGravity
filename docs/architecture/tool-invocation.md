# Tool Invocation Guarantees

## Overview

OpenGravity uses an LLM + Executor + Tool system where tool invocation is primarily LLM-driven but enforced by the system. This document defines the guarantees and rules that govern tool execution behavior.

## Core Concepts

### 2.1 Tool Types

#### AlwaysFresh
- **Examples**: `get_current_time`
- **Behavior**: Must always execute when required by user intent
- **History reuse**: Never reused from history; each request triggers fresh execution
- **Idempotency**: Excluded from idempotency checks

#### Cacheable
- **Examples**: `write_local_note`, `read_local_notes`
- **Behavior**: Results can be cached and potentially reused
- **History reuse**: Subject to idempotency rules based on tool name + input
- **Idempotency**: Subject to idempotency enforcement

## Tool Invocation Flow

The tool invocation follows a multi-step loop:

```
1. LLM receives context (filtered messages)
2. LLM MAY emit TOOL call (e.g., "TOOL:tool_name:input")
3. Executor:
   a. Parses tool call from LLM response
   b. Applies AlwaysFresh guardrails (if applicable)
   c. Applies idempotency checks (for cacheable tools)
4. Tool executes OR is blocked
5. Result returned as Role::Tool message
6. Loop continues: next iteration consumes result
7. Loop terminates when assistant produces final answer
```

**Important**: Multi-step iteration is expected and correct behavior. Tool execution does not immediately end the loop.

## Idempotency Rules

Idempotency prevents duplicate execution of the same tool with the same input.

### Matching Criteria
- Tool name (exact match)
- Tool input (exact match, parsed from Tool message format)

### Tool Message Format
```
Tool result available: <tool_name>:<input>; <result>
```

### Behavior Matrix

| Case | Result | Reason |
|------|--------|--------|
| Same tool + same input | **Block execution** | Idempotency match |
| Same tool + different input | **Execute** | Input differs |
| Different tool + same input | **Execute** | Tool differs |
| AlwaysFresh tool | **Always execute** | Excluded from idempotency |

### Scope
- Full unfiltered message history (cross-turn within same runtime session)
- NOT persisted across process restarts
- NOT shared across distributed instances

### Idempotency Response Visibility

When idempotency is triggered, the system returns an Assistant message:
```
IDEMPOTENCY: previous execution detected for <tool> with input <input>
```

**This message is visible to the LLM.**

- Currently used for transparency and debugging
- Exposes internal control flow decisions to the LLM
- LLM may incorporate this into its reasoning for subsequent steps

**This is a temporary implementation detail, not a long-term contract.**

Future versions may replace this with:
- Internal signals (not visible to LLM)
- Non-visible control flow mechanisms

## AlwaysFresh Guarantee

> **Rule**: If the user requests time-related information, the system MUST execute `get_current_time`.

### Intent Detection
Current implementation uses heuristic string matching on user message:
- Contains "hora"
- Contains "time"
- Contains "qué hora"
- Contains "what time"

### Guarantee Scope
- LLM is EXPECTED to call `TOOL:get_current_time` when appropriate
- If LLM fails to emit tool call, Executor enforces via guardrail
- Each time query triggers fresh execution (no caching)

## Guardrail Behavior

Guardrails are fallback mechanisms that enforce correct behavior when LLM reasoning is insufficient.

### AlwaysFresh Guardrail (Current Implementation)

**Trigger Conditions**:
1. No Tool result exists in current context
2. User message matches time query pattern

**Action**:
1. Force `get_current_time` execution
2. Inject Tool message with result
3. Continue loop (should_continue = true)

**Code Location**: `src/agent/executor.rs` (fallback branch)

### Characteristics
- This is a **fallback mechanism**, not the primary execution path
- Primary path: LLM correctly emits tool call
- Guardrail activates only when LLM reasoning is insufficient

## Tool Blocking Rules

Two distinct blocking mechanisms exist:

### 7.1 Idempotency Blocking
- **Based on**: Full message history
- **Matching**: Exact tool name + input
- **Applicability**: Cacheable tools only
- **Scope**: Cross-turn within session
- **Determinism**: Deterministic (exact match)

### 7.2 tool_blocked Flag
- **Based on**: Presence of Tool result in current context
- **Purpose**: Prevents redundant tool calls within same reasoning phase
- **Applicability**: Cacheable tools only
- **Behavior**: Blocks if Tool result already exists (not AlwaysFresh)

### Mutual Exclusivity
- AlwaysFresh tools: Never blocked by either mechanism
- Cacheable tools: Subject to both mechanisms

## Loop Semantics

### Iteration Model
- Agent loop is iterative (max iterations: 4)
- Each iteration: LLM call → potential tool execution → result handling

### Loop Termination Conditions
1. Assistant produces final answer (no tool call)
2. No further tool calls required
3. Max iterations reached (error case)

### Tool Execution and Loop Continuation
- Tool execution does NOT automatically end the loop
- Loop continues until assistant produces final answer
- Tool results are consumed in subsequent iterations

### Message Flow Within Loop
```
User message
    → LLM reasoning
        → TOOL call (if needed)
            → Executor processes
                → Tool executes OR blocks
                    → Tool result
                        → LLM consumes result
                            → Final answer OR next tool
```

## Known Limitations

1. **Intent Detection**: AlwaysFresh intent detection uses heuristic string matching
   - Case-sensitive patterns may miss variations
   - Not semantic understanding

2. **Guardrail Location**: Guardrails implemented in Executor, not registry-driven
   - Hardcoded for `get_current_time`
   - Not extensible to other tools

3. **Idempotency Scope**: In-memory only
   - Lost on process restart
   - Not shared across instances

4. **No Semantic Intent**: System cannot understand arbitrary user intent
   - Only pattern-matched intents supported
   - Guardrails cover specific known cases

## Future Improvements (Non-Blocking)

These are documented for awareness but NOT implemented:

1. **Registry-Driven Guardrails**: Move intent detection to tool metadata
   - `always_execute: bool` flag on ToolDefinition
   - Pattern-based intent matching per tool

2. **Semantic Intent Classification**: Replace string matching
   - NLP-based intent detection
   - Extensible intent handlers

3. **Unified Blocking Policy**: Consolidate idempotency and guardrails
   - Single policy engine per tool
   - Configuration-driven rather than code-driven

4. **Persistence**: Extend idempotency across restarts
   - Store executed tool + input pairs in DB
   - Cross-session deduplication

---

## Summary

| Behavior | Guarantee |
|----------|----------|
| AlwaysFresh tool execution | Always executes when required |
| Idempotency | Same tool + same input blocked |
| Different input | Always executes |
| Loop termination | When assistant produces final answer |
| Guardrail activation | Fallback only; LLM primary path |

---

*Document Version: 1.0*
*Phase: 2.10 Closure*
