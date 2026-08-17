#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$root/target/agent-starter-check"
build="$root/target/agent-starter-check-build"

rm -rf "$work" "$build"
mkdir -p "$work"
cp -R "$root/templates/agent-starter/." "$work/"

cat >>"$work/Cargo.toml" <<EOF

[patch.crates-io]
jvmti-bindings = { path = "$root" }
EOF

CARGO_TARGET_DIR="$build" cargo check --manifest-path "$work/Cargo.toml"
echo "agent starter compile check passed"
