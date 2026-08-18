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

OUT=target/raw-header-inventory
mkdir -p "$OUT"

grep -oE 'JVMTI_[A-Z0-9_]+' "$HEADER" \
  | grep -v '^JVMTI_H_$' \
  | sort -u > "$OUT/header-constants.txt"
sed -nE 's/.*pub const (JVMTI_[A-Z0-9_]+).*/\1/p' src/sys/jvmti.rs \
  | sort -u > "$OUT/rust-constants.txt"
comm -12 "$OUT/header-constants.txt" "$OUT/rust-constants.txt" \
  > "$OUT/shared-constants.txt"

{
  echo '#include <stdio.h>'
  echo '#include "jvmti.h"'
  echo 'int main(void) {'
  while read -r name; do
    printf '  printf("%s=%%lld\\n", (long long)(%s));\n' "$name" "$name"
  done < "$OUT/shared-constants.txt"
  echo '  return 0;'
  echo '}'
} > "$OUT/constants.c"

cc -std=c11 -Wall -Wextra -Werror \
  -I "$(dirname "$JNI_HEADER")" -I "$PLATFORM_INCLUDE" \
  "$OUT/constants.c" -o "$OUT/constants-c"
"$OUT/constants-c" > "$OUT/constants-c.txt"

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
  cat <<'RS'
use jvmti_bindings::sys::jvmti::*;

trait InventoryValue { fn inventory_value(self) -> i128; }
impl InventoryValue for i32 { fn inventory_value(self) -> i128 { self as i128 } }
impl InventoryValue for u32 { fn inventory_value(self) -> i128 { self as i128 } }
impl InventoryValue for jvmtiError {
    fn inventory_value(self) -> i128 { self.raw() as i128 }
}
fn print_value<T: InventoryValue>(name: &str, value: T) {
    println!("{name}={}", value.inventory_value());
}
fn main() {
RS
  while read -r name; do
    printf '    print_value("%s", %s);\n' "$name" "$name"
  done < "$OUT/shared-constants.txt"
  echo '}'
} > "$OUT/constants.rs"

rustc +1.85.0 --edition=2024 "$OUT/constants.rs" \
  --extern "jvmti_bindings=$RLIB" \
  -L dependency=target/debug/deps \
  -o "$OUT/constants-rust"
"$OUT/constants-rust" > "$OUT/constants-rust.txt"

if ! diff -u "$OUT/constants-c.txt" "$OUT/constants-rust.txt"; then
  echo "error: Rust JVMTI constant values differ from the audited header" >&2
  exit 1
fi

python3 scripts/check-raw-table-inventory.py \
  --jni-header "$JNI_HEADER" --jvmti-header "$HEADER"
echo "raw constant values and table inventories match $(realpath "$HEADER")"
