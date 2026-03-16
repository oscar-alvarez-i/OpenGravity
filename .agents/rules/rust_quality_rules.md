# Rust Quality Rules
- **Zero Warnings**: No compilation or Clippy warnings allowed.
- **Clippy Rules**: All Clippy warnings are considered failures.
- **Hygiene**: No `TODO` or `FIXME` comments in production code.
- **Dependencies**: No unnecessary crates; justify every new one.
- **Safety**: No `unwrap()` or `expect()` outside of documented invariant cases.
