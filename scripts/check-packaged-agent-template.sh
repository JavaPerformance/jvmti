#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
[[ -n "$version" ]] || {
    echo "could not read package version from Cargo.toml" >&2
    exit 2
}

archive="$root/target/package/jvmti-bindings-$version.crate"
extract_root="$root/target/packaged-agent-template-check"
package_source="$extract_root/jvmti-bindings-$version"

rm -rf "$extract_root"
mkdir -p "$extract_root"
cargo +1.85.0 package --locked --allow-dirty --no-verify
[[ -f "$archive" ]] || {
    echo "cargo package did not create $archive" >&2
    exit 1
}

forbidden=$(tar -tzf "$archive" | grep -E \
    '(^|/)(__pycache__|target|\.git)(/|$)|\.py[co]$|(^|/)\.env$' || true)
if [[ -n "$forbidden" ]]; then
    echo "packaged crate contains generated or sensitive paths:" >&2
    printf '%s\n' "$forbidden" >&2
    exit 1
fi
tar -xzf "$archive" -C "$extract_root"

JVMTI_BINDINGS_SOURCE="$package_source" scripts/check-agent-template.sh
echo "packaged startup-and-attach agent compile check passed: $archive"
