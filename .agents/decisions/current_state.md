# Decision Log: Current Repository State
- **Validation Split**: `check_fast` vs `check_full` split was implemented to reduce token consumption and speed up iterations.
- **Branch Coverage**: Branch-driven coverage is required to ensure complex logic paths (like fallback LLMs) are actually robust.
- **Telexide Debt**: Transitive dependencies (`reqwest` 0.11, etc.) are temporarily accepted until `teloxide` upgrades.
- **Engineering Contract**: `AGENT.md` remains the "Source of Truth" for human/AI collaboration policies.
- **Minimal Output**: Validation verbosity is strictly minimized (`-q`) to prevent context overflow.

## Current Architectural Risks
- **Error Resilience**: Network or API errors during a reasoning turn can lead to turn failure; lacks robust retry or graceful degradation for transient network issues.
- **Planner Constraints**: System prompts are static and hardcoded; cannot yet adapt to specialized task routing.

## Fragile Areas
- **Tool Parsing**: `Registry::parse_tool_call` relies on simple string splitting/regex and is sensitive to LLM verbosity or formatting deviations.
- **Tool Arguments**: Infrastructure for parsing complex tool arguments (JSON) is currently missing; only simple trigger-based tools are stable.
- **Persistence Growth**: SQLite database grows monotonically; lacks TTL, manual pruning, or automated history management.

## Dependency Constraints
- **Reqwest Overlap**: Mismatch between `teloxide` (`reqwest 0.11` / `native-tls`) and core `OpenRouter` calls (`reqwest 0.12` / `rustls`) adds binary bloat and redundant TLS stacks.
