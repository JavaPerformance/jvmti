#!/usr/bin/env python3
"""Generate compiler checks for every JNI and JVM TI table-field signature."""

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


def function_table_fields(block: str) -> list[tuple[str, bool]]:
    fields: list[tuple[str, bool]] = []
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
            fields.append((function.group(1), True))
        else:
            plain = re.search(r"\b(\w+)\s*;\s*$", declaration)
            if plain:
                fields.append((plain.group(1), False))
        declaration = ""
    return fields


def plain_table_fields(block: str) -> list[tuple[str, bool]]:
    return [
        (name, True)
        for name in re.findall(
            r"^\s*(?:\w+\s+)+(\w+)\s*;\s*$", block, re.MULTILINE
        )
    ]


def emit_aliases(
    lines: list[str], prefix: str, c_type: str, fields: list[tuple[str, bool]]
) -> None:
    for field, _ in fields:
        lines.append(
            f"typedef __typeof__((({c_type} *)0)->{field}) "
            f"canonical_{prefix}_{field};"
        )


def emit_checker(
    lines: list[str],
    prefix: str,
    rust_type: str,
    fields: list[tuple[str, bool]],
    wrap_functions: bool,
) -> None:
    lines.append(f"fn check_{prefix}(value: {rust_type}) {{")
    for field, is_function in fields:
        value = f"value.{field}"
        if wrap_functions and is_function:
            value = f"Some({value})"
        lines.append(
            f"    let _: canonical_{prefix}_{field} = {value};"
        )
    lines.append("}")
    lines.append("")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--jni-header", required=True, type=Path)
    parser.add_argument("--jvmti-header", required=True, type=Path)
    parser.add_argument("--header-out", required=True, type=Path)
    parser.add_argument("--checker-out", required=True, type=Path)
    args = parser.parse_args()

    jni = args.jni_header.read_text()
    jvmti = args.jvmti_header.read_text()

    tables = [
        (
            "jni_native",
            "struct JNINativeInterface_",
            "jni::JNINativeInterface_",
            True,
            function_table_fields(
                c_block(jni, "struct JNINativeInterface_ {", "};")
            ),
        ),
        (
            "jni_invoke",
            "struct JNIInvokeInterface_",
            "jni::JNIInvokeInterface_",
            True,
            function_table_fields(
                c_block(jni, "struct JNIInvokeInterface_ {", "};")
            ),
        ),
        (
            "jvmti_functions",
            "struct jvmtiInterface_1_",
            "jvmti::jvmtiInterface_1_",
            False,
            function_table_fields(
                c_block(
                    jvmti,
                    "typedef struct jvmtiInterface_1_ {",
                    "} jvmtiInterface_1;",
                )
            ),
        ),
        (
            "jvmti_events",
            "jvmtiEventCallbacks",
            "jvmti::jvmtiEventCallbacks",
            False,
            plain_table_fields(
                c_block(jvmti, "typedef struct {", "} jvmtiEventCallbacks;")
            ),
        ),
    ]

    expected = {
        "jni_native": 237,
        "jni_invoke": 8,
        "jvmti_functions": 156,
        "jvmti_events": 39,
    }
    for prefix, _, _, _, fields in tables:
        if len(fields) != expected[prefix]:
            raise SystemExit(
                f"{prefix}: expected {expected[prefix]} fields, found {len(fields)}"
            )

    header = ['#include "jni.h"', '#include "jvmti.h"', ""]
    for prefix, c_type, _, _, fields in tables:
        emit_aliases(header, prefix, c_type, fields)
    args.header_out.write_text("\n".join(header) + "\n")

    checker = [
        "use jvmti_bindings::sys::{jni, jvmti};",
        "",
    ]
    for prefix, _, rust_type, wrap_functions, fields in tables:
        emit_checker(checker, prefix, rust_type, fields, wrap_functions)
    checker.append("fn main() {}")
    args.checker_out.write_text("\n".join(checker) + "\n")

    total = sum(len(fields) for _, _, _, _, fields in tables)
    print(
        "generated exact native signature checks for "
        f"{total} fields (JNI 237+8, JVM TI 156+39)"
    )


if __name__ == "__main__":
    main()
