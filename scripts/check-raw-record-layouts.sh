#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

HEADER=${JVMTI_HEADER:-${1:-}}
if [[ -z "$HEADER" || ! -f "$HEADER" ]]; then
  echo "usage: JVMTI_HEADER=/path/to/jvmti.h $0" >&2
  exit 2
fi
JNI_HEADER=${JNI_HEADER:-$(dirname "$HEADER")/jni.h}
PLATFORM_INCLUDE=${JNI_PLATFORM_INCLUDE:-$(dirname "$HEADER")/linux}
if [[ ! -f "$JNI_HEADER" ]]; then
  echo "error: JNI header not found: $JNI_HEADER" >&2
  exit 2
fi
command -v bindgen >/dev/null || {
  echo "error: bindgen-cli 0.72.x is required for complete layout checking" >&2
  exit 2
}

OUT=target/raw-record-layout-check
mkdir -p "$OUT"

bindgen "$HEADER" \
  --no-layout-tests \
  --no-doc-comments \
  --rust-target 1.85 \
  --rust-edition 2024 \
  -- \
  -I"$(dirname "$JNI_HEADER")" \
  -I"$PLATFORM_INCLUDE" \
  > "$OUT/canonical.rs"

python3 scripts/generate-raw-layout-check.py \
  --bindgen "$OUT/canonical.rs" \
  --jni-rust src/sys/jni.rs \
  --jvmti-rust src/sys/jvmti.rs \
  --out "$OUT/checker.rs"

cargo +1.85.0 build --locked --lib --message-format=json \
  > "$OUT/cargo-build.json"
RLIB=$(python3 - "$OUT/cargo-build.json" <<'PY'
import json
import sys

for line in open(sys.argv[1]):
    message = json.loads(line)
    target = message.get("target", {})
    if target.get("name") == "jvmti_bindings" and "rlib" in target.get("kind", []):
        for filename in message.get("filenames", []):
            if filename.endswith(".rlib"):
                print(filename)
PY
)
if [[ -z "$RLIB" ]]; then
  echo "error: cargo did not report the jvmti-bindings rlib" >&2
  exit 1
fi

{
  echo '#![allow(dead_code, non_camel_case_types, non_snake_case, non_upper_case_globals)]'
  echo 'mod canonical {'
  echo '  #![allow(dead_code, non_camel_case_types, non_snake_case, non_upper_case_globals)]'
  cat "$OUT/canonical.rs"
  echo '}'
  cat "$OUT/checker.rs"
} > "$OUT/complete-checker.rs"

rustc +1.85.0 --edition=2024 "$OUT/complete-checker.rs" \
  --extern "jvmti_bindings=$RLIB" \
  -L dependency=target/debug/deps \
  -o "$OUT/complete-checker"
"$OUT/complete-checker"

echo "all public JNI/JVM TI record layouts match $(realpath "$HEADER")"
