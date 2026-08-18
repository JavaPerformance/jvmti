#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

cargo metadata --locked --all-features --format-version 1 >/dev/null

lock_packages=$(awk '$0 == "[[package]]" { count++ } END { print count + 0 }' Cargo.lock)
if [[ "$lock_packages" -ne 1 ]]; then
    echo "zero-dependency check failed: Cargo.lock contains $lock_packages packages" >&2
    cargo tree --locked --all-features --edges normal,build,dev >&2
    exit 1
fi

mapfile -t graph < <(
    cargo tree --locked --all-features --edges normal,build,dev --prefix none \
        | sed '/^[[:space:]]*$/d'
)
if [[ "${#graph[@]}" -ne 1 || "${graph[0]}" != jvmti-bindings\ v* ]]; then
    echo "zero-dependency check failed: expected only the root package" >&2
    printf '  %s\n' "${graph[@]}" >&2
    exit 1
fi

echo "zero-dependency check passed: normal, optional, build, and dev graph contains only ${graph[0]}"
