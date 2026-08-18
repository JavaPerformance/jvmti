#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
out=${1:-"target/public-api/jvmti-bindings-$version.txt"}

if ! cargo public-api --version >/dev/null 2>&1; then
  cat >&2 <<'EOF'
cargo-public-api is required for the full-signature baseline.
Install a compatible release with:
  cargo +stable install cargo-public-api --version 0.52.0 --locked
  rustup toolchain install nightly --profile minimal
EOF
  exit 2
fi

mkdir -p "$(dirname "$out")"
toolchain=${PUBLIC_API_TOOLCHAIN:-nightly-2026-08-17}
cargo "+$toolchain" public-api --all-features --color never -sss >"$out"
echo "wrote full public API baseline: $out"
