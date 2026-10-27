#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-fail-fast

cargo fmt --manifest-path tools/release/Cargo.toml --all --check
cargo clippy --manifest-path tools/release/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path tools/release/Cargo.toml --locked --no-fail-fast
