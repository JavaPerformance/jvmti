#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

for toolchain in 1.85.0 stable; do
    cargo "+$toolchain" check --locked --all-targets --all-features
    cargo "+$toolchain" test --locked
    cargo "+$toolchain" test --locked --all-features
    cargo "+$toolchain" test --locked --doc --all-features
    cargo "+$toolchain" clippy --locked --all-targets --all-features -- -D warnings
    RUSTDOCFLAGS='-D warnings' cargo "+$toolchain" doc \
        --locked --no-deps --all-features
done

cargo +1.85.0 fmt --all -- --check
scripts/check-zero-dependencies.sh
scripts/check-wrapper-coverage.sh
scripts/check-wrapper-forwarding.py
scripts/check-public-api-extensibility.py
scripts/check-unsafe-surface.py
scripts/check-host-va-list-abi.sh
scripts/check-downstream-canaries.sh
scripts/check-public-api-baseline.sh

if [[ -n ${JAVA_HOME:-} ]]; then
    scripts/check-classfile-corpus.sh "$JAVA_HOME"
fi

cargo +1.85.0 publish --dry-run --locked
git diff --check
echo "common 3.0 release gates passed"
