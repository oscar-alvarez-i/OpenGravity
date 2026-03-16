# Validation Rules
- **Iterative default command**: `./scripts/check_fast.sh`
- **Full validation**: Only during stabilization phase.
- **Coverage**: Never run `tarpaulin` repeatedly during micro-fixes.
- **Security Check**: Never run `audit`/`deny` unless `Cargo.toml` changed.
- **Process**: Prefer targeted module validation before global validation.
