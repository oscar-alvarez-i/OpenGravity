# Coverage Hardening Workflow
1. **Inspection**: Identify uncovered branches first (not just lines).
2. **Prioritization**: Prioritize branch coverage over simple line coverage.
3. **Paths**: Test error paths and edge cases first.
4. **Logic**: Avoid cosmetic or trivial tests (e.g., simple constructors).
5. **Validation**: Validate with `check_fast.sh` before running full coverage suite.
