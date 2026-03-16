# Dependency Change Workflow
1. **Justification**: Document the functional need for the new crate.
2. **Audit**: Use `cargo tree` to audit the impact on the dependency graph.
3. **Security Audit**: Run `cargo audit` only after the `Cargo.toml` change.
4. **License Check**: Run `cargo deny check` after updating dependencies.
