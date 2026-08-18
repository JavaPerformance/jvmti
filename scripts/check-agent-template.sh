#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dependency_path=${JVMTI_BINDINGS_SOURCE:-$root}
work="$root/target/agent-starter-check"
build="$root/target/agent-starter-check-build"

[[ -f "$dependency_path/Cargo.toml" ]] || {
    echo "jvmti-bindings source does not contain Cargo.toml: $dependency_path" >&2
    exit 2
}

rm -rf "$work" "$build"
mkdir -p "$work"
cp -R "$root/templates/agent-starter/." "$work/"

cat >>"$work/Cargo.toml" <<EOF

[patch.crates-io]
jvmti-bindings = { path = "$dependency_path" }
EOF

CARGO_TARGET_DIR="$build" cargo check --manifest-path "$work/Cargo.toml"
echo "agent starter compile check passed"
