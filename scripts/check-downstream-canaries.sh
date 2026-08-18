#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
archive="$root/target/package/jvmti-bindings-$version.crate"
extract_root="$root/target/downstream-canary-check"
package_source="$extract_root/jvmti-bindings-$version"

rm -rf "$extract_root"
mkdir -p "$extract_root"
cargo +1.85.0 package --locked --allow-dirty --no-verify
tar -xzf "$archive" -C "$extract_root"

JVMTI_BINDINGS_SOURCE="$package_source" scripts/check-agent-template.sh

embed_work="$extract_root/embed-starter"
embed_build="$root/target/embed-starter-check-build"
rm -rf "$embed_work" "$embed_build"
mkdir -p "$embed_work"
cp -R templates/embed-starter/. "$embed_work/"
cat >>"$embed_work/Cargo.toml" <<EOF

[patch.crates-io]
jvmti-bindings = { path = "$package_source" }
EOF
CARGO_TARGET_DIR="$embed_build" cargo +1.85.0 check \
    --locked --manifest-path "$embed_work/Cargo.toml"

echo "packaged startup/attach and embedded-JVM downstream canaries passed"
