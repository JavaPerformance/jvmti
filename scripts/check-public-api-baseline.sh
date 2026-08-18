#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

baseline=${1:-api/jvmti-bindings-3.0.0.txt}
actual=target/public-api/current.txt

[[ -f "$baseline" ]] || {
    echo "missing reviewed public API baseline: $baseline" >&2
    exit 2
}

scripts/capture-public-api-baseline.sh "$actual"
if ! diff -u "$baseline" "$actual" >target/public-api/baseline.diff; then
    echo "public API differs from the reviewed 3.0 baseline:" >&2
    cat target/public-api/baseline.diff >&2
    exit 1
fi

echo "public API matches reviewed baseline: $baseline"
