#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [[ -n "$(git status --porcelain)" ]]; then
    echo "release evidence requires a clean Git tree" >&2
    exit 2
fi

version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
expected_tag="v$version"
if [[ ${GITHUB_REF_TYPE:-} == tag && ${GITHUB_REF_NAME:-} != "$expected_tag" ]]; then
    echo "tag ${GITHUB_REF_NAME} does not match Cargo version $expected_tag" >&2
    exit 2
fi

out=${1:-"target/release-evidence/$expected_tag"}
rm -rf "$out"
mkdir -p "$out"

cargo +1.85.0 package --locked
archive="target/package/jvmti-bindings-$version.crate"
cp "$archive" "$out/"
scripts/generate-release-sbom.py \
    --archive "$out/$(basename "$archive")" \
    --output "$out/jvmti-bindings-$version.spdx.json"

(
    cd "$out"
    sha256sum "jvmti-bindings-$version.crate" \
        "jvmti-bindings-$version.spdx.json" >SHA256SUMS
)

echo "release evidence written to: $out"
