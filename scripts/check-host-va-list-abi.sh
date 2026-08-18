#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$root/target/host-va-list-abi"
mkdir -p "$work"

command -v cc >/dev/null || {
    echo "error: a host C compiler is required for the va_list ABI proof" >&2
    exit 2
}

cargo +1.85.0 build --locked --lib --message-format=json >"$work/cargo-build.json"
rlib=$(python3 - "$work/cargo-build.json" <<'PY'
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
[[ -n "$rlib" ]] || {
    echo "error: cargo did not report the jvmti-bindings rlib" >&2
    exit 1
}

cat >"$work/forward.rs" <<'RS'
use jvmti_bindings::sys::jni::va_list;
use std::os::raw::c_int;

unsafe extern "C" {
    fn c_consume_va_list(args: va_list) -> c_int;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_forward_va_list(args: va_list) -> c_int {
    unsafe { c_consume_va_list(args) }
}
RS

cat >"$work/driver.c" <<'C'
#include <stdarg.h>
#include <stdio.h>

extern int rust_forward_va_list(va_list args);

int c_consume_va_list(va_list args) {
    return va_arg(args, int);
}

static int drive_va_list(int ignored, ...) {
    va_list args;
    va_start(args, ignored);
    int value = rust_forward_va_list(args);
    va_end(args);
    return value;
}

int main(void) {
    const int expected = 0x13579bdf;
    const int actual = drive_va_list(0, expected);
    if (actual != expected) {
        fprintf(stderr, "va_list ABI mismatch: expected %x, got %x\n", expected, actual);
        return 1;
    }
    return 0;
}
C

rustc +1.85.0 --edition=2024 --crate-type=staticlib "$work/forward.rs" \
    --extern "jvmti_bindings=$rlib" \
    -L dependency="$root/target/debug/deps" \
    -o "$work/libva_list_forward.a"

case "$(uname -s)" in
    Darwin)
        cc "$work/driver.c" "$work/libva_list_forward.a" \
            -framework Security -framework CoreFoundation \
            -o "$work/va-list-proof"
        ;;
    *)
        cc "$work/driver.c" "$work/libva_list_forward.a" \
            -ldl -lpthread -lm \
            -o "$work/va-list-proof"
        ;;
esac

"$work/va-list-proof"
echo "host C -> Rust -> C va_list forwarding proof passed on $(uname -m)"
