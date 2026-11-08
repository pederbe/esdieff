#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

# --warm compiles what the checks compile without running them, to refill CI caches.
run=true
case "$*" in
    ''|--all) application=true ;;
    --release-tools-only) application=false ;;
    --warm) application=true; run=false ;;
    *) echo 'Usage: bash scripts/check.sh [--all|--release-tools-only|--warm]' >&2; exit 2 ;;
esac

check() {
    local manifest=$1
    if [ "$run" = true ]; then
        cargo fmt --manifest-path "$manifest" --all --check
    fi
    cargo clippy --manifest-path "$manifest" --locked --all-targets -- -D warnings
    if [ "$run" = true ]; then
        cargo test --manifest-path "$manifest" --locked --no-fail-fast
    else
        cargo test --manifest-path "$manifest" --locked --no-run
    fi
}

if [ "$application" = true ]; then
    check Cargo.toml
fi
check tools/release/Cargo.toml
if [ "$application" = true ]; then
    check tools/terminal-html/Cargo.toml
fi
