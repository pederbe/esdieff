#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

case "$*" in
    ''|--all) application=true ;;
    --release-tools-only) application=false ;;
    *) echo 'Usage: bash scripts/check.sh [--all|--release-tools-only]' >&2; exit 2 ;;
esac

if [ "$application" = true ]; then
    cargo fmt --all --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked --no-fail-fast
fi

cargo fmt --manifest-path tools/release/Cargo.toml --all --check
cargo clippy --manifest-path tools/release/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path tools/release/Cargo.toml --locked --no-fail-fast

if [ "$application" = true ]; then
    cargo fmt --manifest-path tools/terminal-html/Cargo.toml --all --check
    cargo clippy --manifest-path tools/terminal-html/Cargo.toml --locked --all-targets -- -D warnings
    cargo test --manifest-path tools/terminal-html/Cargo.toml --locked --no-fail-fast

    # CI checks this standalone helper once in its dedicated Linux job.
    if [ "${GITHUB_ACTIONS:-false}" != true ]; then
        bash scripts/check-release-changelog.sh
    fi
fi
