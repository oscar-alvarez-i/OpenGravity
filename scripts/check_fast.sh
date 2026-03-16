#!/usr/bin/env bash
set -euo pipefail

echo "Running fast validation..."

cargo fmt --check
cargo clippy -q --all-targets --all-features -- -D warnings
cargo test -q

echo "Fast validation passed."
