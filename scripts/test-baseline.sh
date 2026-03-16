#!/usr/bin/env bash
set -e

export RUST_LOG=debug

cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib -- --nocapture
cargo test --test conversation_regression -- --nocapture
