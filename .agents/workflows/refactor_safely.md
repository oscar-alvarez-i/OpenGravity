# Safe Refactoring Workflow
1. **Targeted Validation**: Validate only the affected module/scope first.
2. **Boundaries**: Ensure logic remains within its original architectural boundaries.
3. **Simplicity**: Avoid speculative or early abstractions.
4. **Checkpoint**: Run full validation (`check_full.sh`) only after reaching a stable checkpoint.
