# ADR 001: Message Contract

## Status
Accepted

## Context
OpenGravity requires a clear contract for message roles and their persistence to ensure conversation integrity and prevent reasoning leakage into the LLM's active context.

## Decision
We define the following message roles and rules:

### Message Roles
- **User**: Direct input from the end-user. Always persisted. Always enters active context.
- **Assistant**: Output from the LLM. 
    - **Final Answer**: Persisted and enters active context.
    - **Reasoning/Tool Call**: Assistant content that precedes a `TOOL:` call.
- **Tool**: Output from a tool execution. Persisted and enters active context.

### Persistence Rules
- All `User` messages are persisted to the database.
- All `Tool` messages are persisted to the database.
- `Assistant` messages that contain a final answer are persisted.
- **CRITICAL**: Assistant reasoning that occurs immediately before a `TOOL` call is **never** persisted to the database and **never** re-enters the active context in subsequent turns.

### Active Context Rules
- The active context is built from the latest N persisted messages.
- When an assistant generates a tool call, the reasoning part is discarded after the tool executes. The next LLM step receives the `User` message, any previous history, and the `Tool` result, but NOT the reasoning that led to the tool call.

## Consequences
- Prevents "reasoning loops" where the LLM repeats its previous thought process.
- Ensures the conversation history remains clean and focused on facts (User/Tool/Final Answer).
- Reduces token usage by excluding transient reasoning.
