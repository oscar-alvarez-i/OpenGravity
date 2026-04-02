---
description: Perform a full project stabilization pass before commit or merge.
---

# Project Stabilization Workflow

Follow this workflow when completing a feature or before committing major changes.

1. Ensure all formatting is correct.
   // turbo
   cargo cf

2. Run full validation suite (includes security and coverage).
   // turbo
   ./scripts/check_full.sh

3. If coverage is below 95%, add additional tests to cover missing branches.
   Reference `.agents/workflows/coverage_hardening.md` for guidance.
