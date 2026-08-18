#!/usr/bin/env python3
"""Compare native JNI/JVMTI table field order with the Rust raw bindings."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


def c_block(text: str, start: str, end: str) -> str:
    begin = text.find(start)
    if begin < 0:
        raise SystemExit(f"missing C table start: {start}")
    finish = text.find(end, begin)
    if finish < 0:
        raise SystemExit(f"missing C table end: {end}")
    return text[begin : finish + len(end)]


def rust_block(text: str, name: str) -> str:
    match = re.search(rf"pub struct {re.escape(name)}\s*\{{", text)
    if not match:
        raise SystemExit(f"missing Rust table: {name}")
    depth = 1
    cursor = match.end()
    while cursor < len(text) and depth:
        if text[cursor] == "{":
            depth += 1
        elif text[cursor] == "}":
            depth -= 1
        cursor += 1
    if depth:
        raise SystemExit(f"unterminated Rust table: {name}")
    return text[match.end() : cursor - 1]


def c_function_table_fields(block: str) -> list[str]:
    fields: list[str] = []
    declaration = ""
    for raw_line in block.splitlines()[1:]:
        line = re.sub(r"/\*.*?\*/", "", raw_line).strip()
        if not line:
            continue
        declaration += " " + line
        if ";" not in line:
            continue
        if declaration.lstrip().startswith("}"):
            declaration = ""
            continue
        function = re.search(r"\(\s*JNICALL\s*\*(\w+)\s*\)", declaration)
        if function:
            fields.append(function.group(1))
        else:
            plain = re.search(r"\b(\w+)\s*;\s*$", declaration)
            if plain:
                fields.append(plain.group(1))
        declaration = ""
    return fields


def c_plain_table_fields(block: str) -> list[str]:
    return re.findall(r"^\s*(?:\w+\s+)+(\w+)\s*;\s*$", block, re.MULTILINE)


def rust_fields(block: str) -> list[str]:
    return re.findall(r"^\s*pub\s+(\w+)\s*:", block, re.MULTILINE)


def compare(label: str, native: list[str], rust: list[str]) -> None:
    if native == rust:
        print(f"{label}: {len(native)} fields match in order")
        return
    limit = max(len(native), len(rust))
    for index in range(limit):
        left = native[index] if index < len(native) else "<missing>"
        right = rust[index] if index < len(rust) else "<missing>"
        if left != right:
            raise SystemExit(
                f"{label}: field {index + 1} differs: header={left}, rust={right} "
                f"(header count={len(native)}, rust count={len(rust)})"
            )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--jni-header", required=True, type=Path)
    parser.add_argument("--jvmti-header", required=True, type=Path)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    jni_header = args.jni_header.read_text()
    jvmti_header = args.jvmti_header.read_text()
    rust_jni = (root / "src/sys/jni.rs").read_text()
    rust_jvmti = (root / "src/sys/jvmti.rs").read_text()

    checks = [
        (
            "JNI native interface",
            c_function_table_fields(
                c_block(jni_header, "struct JNINativeInterface_ {", "};")
            ),
            rust_fields(rust_block(rust_jni, "JNINativeInterface_")),
        ),
        (
            "JNI invocation interface",
            c_function_table_fields(
                c_block(jni_header, "struct JNIInvokeInterface_ {", "};")
            ),
            rust_fields(rust_block(rust_jni, "JNIInvokeInterface_")),
        ),
        (
            "JVM TI function interface",
            c_function_table_fields(
                c_block(jvmti_header, "typedef struct jvmtiInterface_1_ {", "} jvmtiInterface_1;")
            ),
            rust_fields(rust_block(rust_jvmti, "jvmtiInterface_1_")),
        ),
        (
            "JVM TI event callbacks",
            c_plain_table_fields(
                c_block(jvmti_header, "typedef struct {", "} jvmtiEventCallbacks;")
            ),
            rust_fields(rust_block(rust_jvmti, "jvmtiEventCallbacks")),
        ),
    ]
    for label, native, rust in checks:
        compare(label, native, rust)


if __name__ == "__main__":
    main()
