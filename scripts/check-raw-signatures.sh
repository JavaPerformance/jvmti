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
  echo "error: bindgen-cli 0.72.x is required for complete signature checking" >&2
  exit 2
}

OUT=target/raw-signature-check
mkdir -p "$OUT"

python3 scripts/generate-raw-signature-check.py \
  --jni-header "$JNI_HEADER" \
  --jvmti-header "$HEADER" \
  --header-out "$OUT/native-signatures.h" \
  --checker-out "$OUT/checker.rs"

bindgen "$OUT/native-signatures.h" \
  --allowlist-type '^canonical_.*' \
  --no-recursive-allowlist \
  --raw-line 'use jvmti_bindings::sys::jni::*;' \
  --raw-line 'use jvmti_bindings::sys::jvmti::*;' \
  --no-layout-tests \
  --no-doc-comments \
  --formatter=none \
  --rust-target 1.85 \
  --rust-edition 2024 \
  -- \
  -I"$(dirname "$JNI_HEADER")" \
  -I"$PLATFORM_INCLUDE" \
  > "$OUT/canonical.rs"

# bindgen resolves JNI's opaque C typedefs to their private struct names and
# emits the host's concrete `C` ABI. Normalize those nominal representations
# to this crate's intentionally opaque pointer aliases. Keep C variadics as C;
# all fixed JNI/JVM TI entry points use Rust's cross-platform `system` ABI.
# Match tokens rather than formatter whitespace so the proof is independent of
# the bindgen runner's rustfmt version (and also works with --formatter=none).
python3 - "$OUT/canonical.rs" <<'PY'
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()
token_replacements = (
    (r"\*\s*mut\s+\*\s*const\s+JNINativeInterface_\b", "*mut JNIEnv"),
    (r"\*\s*mut\s+\*\s*const\s+JNIInvokeInterface_\b", "*mut JavaVM"),
    (r"\*\s*mut\s+\*\s*const\s+jvmtiInterface_1_\b", "*mut jvmtiEnv"),
    (r"\*\s*mut\s+_jobject\b", "jobject"),
    (r"\*\s*mut\s+_jmethodID\b", "jmethodID"),
    (r"\*\s*mut\s+_jfieldID\b", "jfieldID"),
    (r"\b_jobjectType\b", "jobjectRefType"),
    (r"\*\s*mut\s+_jrawMonitorID\b", "jrawMonitorID"),
    (r"\*\s*mut\s+__va_list_tag\b", "va_list"),
    (r"__BindgenOpaqueArray\s*<\s*u64\s*,\s*4usize\s*>", "va_list"),
    (r"\b__va_list\b", "va_list"),
)
for native, public in token_replacements:
    text = re.sub(native, public, text)

function_pattern = re.compile(r'extern\s+"C"\s+fn\s*\(')
fixed_abi_spans = []
for function in function_pattern.finditer(text):
    cursor = function.end()
    depth = 1
    top_level_variadic = False
    while cursor < len(text) and depth:
        if depth == 1 and text.startswith("...", cursor):
            top_level_variadic = True
            cursor += 3
            continue
        if text[cursor] == "(":
            depth += 1
        elif text[cursor] == ")":
            depth -= 1
        cursor += 1
    if depth:
        raise SystemExit("unterminated bindgen function signature")
    if not top_level_variadic:
        fixed_abi_spans.append(function.span())

for start, end in reversed(fixed_abi_spans):
    text = text[:start] + 'extern "system" fn (' + text[end:]

path.write_text(text)
PY

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
  echo '#![allow(dead_code, non_camel_case_types, non_snake_case)]'
  cat "$OUT/canonical.rs" "$OUT/checker.rs"
} > "$OUT/complete-checker.rs"
rustc +1.85.0 --edition=2024 "$OUT/complete-checker.rs" \
  --extern "jvmti_bindings=$RLIB" \
  -L dependency=target/debug/deps \
  -o "$OUT/complete-checker"

echo "all 440 JNI/JVM TI table-field signatures match $(realpath "$HEADER")"
