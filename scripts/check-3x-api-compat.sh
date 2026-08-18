#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

baseline=${1:-v3.0.0}
current=${2:-HEAD}
report=${OUT:-"target/public-api/${baseline//\//_}-to-${current//\//_}.diff"}

git rev-parse --verify "${baseline}^{commit}" >/dev/null 2>&1 || {
  echo "missing API baseline revision: $baseline" >&2
  echo "run this gate only after the signed 3.0.0 baseline tag exists" >&2
  exit 2
}
git rev-parse --verify "${current}^{commit}" >/dev/null 2>&1 || {
  echo "missing current API revision: $current" >&2
  exit 2
}

if [[ -n "$(git status --porcelain)" ]]; then
  echo "API compatibility check requires a clean tree because cargo-public-api checks out revisions" >&2
  exit 2
fi

if ! cargo semver-checks --version >/dev/null 2>&1; then
  cat >&2 <<'EOF'
cargo-semver-checks is required.
Install the current compatible release with:
  cargo +stable install cargo-semver-checks --locked
EOF
  exit 2
fi
if ! cargo public-api --version >/dev/null 2>&1; then
  cat >&2 <<'EOF'
cargo-public-api is required.
Install a compatible release with:
  cargo +stable install cargo-public-api --version 0.52.0 --locked
  rustup toolchain install nightly --profile minimal
EOF
  exit 2
fi

# The tools are complementary: semver-checks explains known violations, while
# public-api catches changed signatures that semver-checks does not yet model.
cargo semver-checks --baseline-rev "$baseline" --all-features

mkdir -p "$(dirname "$report")"
toolchain=${PUBLIC_API_TOOLCHAIN:-nightly-2026-08-17}
cargo "+$toolchain" public-api --all-features --color never -sss diff \
  --deny=changed --deny=removed "$baseline..$current" | tee "$report"

echo "3.x API compatibility gates passed: $baseline -> $current"
