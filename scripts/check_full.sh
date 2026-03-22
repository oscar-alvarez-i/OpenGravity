#!/usr/bin/env bash
set -euo pipefail

echo "Running full validation..."

cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo nextest run
cargo tarpaulin --fail-under 87
cargo audit
cargo deny check
cargo build --release

echo "Full validation passed."
