#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

python3 - "$ROOT" <<'PY'
from __future__ import annotations

import re
import sys
from pathlib import Path


root = Path(sys.argv[1])


def struct_fields(path: str, name: str) -> list[str]:
    text = (root / path).read_text()
    match = re.search(rf"pub struct {re.escape(name)}\s*\{{", text)
    if match is None:
        raise SystemExit(f"missing raw table struct: {name}")
    depth = 1
    cursor = match.end()
    while cursor < len(text) and depth:
        depth += (text[cursor] == "{") - (text[cursor] == "}")
        cursor += 1
    if depth:
        raise SystemExit(f"unterminated raw table struct: {name}")
    return re.findall(
        r"^\s*pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:",
        text[match.end() : cursor - 1],
        re.MULTILINE,
    )


def production_source(*paths: str) -> str:
    parts: list[str] = []
    for raw_path in paths:
        path = root / raw_path
        sources = sorted(path.rglob("*.rs")) if path.is_dir() else [path]
        for source in sources:
            text = source.read_text()
            # Test fakes must not satisfy production wrapper coverage.
            text = re.split(r"^#\[cfg\(test\)\]\s*$", text, maxsplit=1, flags=re.MULTILINE)[0]
            # Exclude prose and literals so mentioning an operation is not
            # mistaken for implementing it.
            text = re.sub(r"/\*.*?\*/", " ", text, flags=re.DOTALL)
            text = re.sub(r"//[^\n]*", " ", text)
            text = re.sub(r'r#+".*?"#+', '""', text, flags=re.DOTALL)
            text = re.sub(r'"(?:\\.|[^"\\])*"', '""', text, flags=re.DOTALL)
            parts.append(text)
    return "\n".join(parts)


def referenced(field: str, source: str) -> bool:
    return re.search(rf"\b{re.escape(field)}\b", source) is not None


def check(
    label: str,
    fields: list[str],
    sources: tuple[str, ...],
    allowed_missing: set[str],
) -> None:
    source = production_source(*sources)
    missing = {field for field in fields if not referenced(field, source)}
    unexpected = sorted(missing - allowed_missing)
    stale_allowance = sorted(allowed_missing - missing)
    if unexpected:
        raise SystemExit(
            f"{label}: missing production wrappers:\n  " + "\n  ".join(unexpected)
        )
    if stale_allowance:
        raise SystemExit(
            f"{label}: raw-only allowance is stale; review and remove:\n  "
            + "\n  ".join(stale_allowance)
        )
    covered = len(fields) - len(missing)
    print(f"{label}: {covered}/{len(fields)} table slots covered; {len(missing)} reviewed raw/reserved")


jni_fields = struct_fields("src/sys/jni.rs", "JNINativeInterface_")
jni_reserved = {field for field in jni_fields if field.startswith("reserved")}
jni_raw_varargs = {"NewObject", "NewObjectV"}
for field in jni_fields:
    if re.fullmatch(
        r"Call(?:Nonvirtual|Static)?(?:Object|Boolean|Byte|Char|Short|Int|Long|Float|Double|Void)MethodV?",
        field,
    ):
        jni_raw_varargs.add(field)
check(
    "JNI native interface",
    jni_fields,
    ("src/jni_wrapper.rs",),
    jni_reserved | jni_raw_varargs,
)

invocation_fields = struct_fields("src/sys/jni.rs", "JNIInvokeInterface_")
check(
    "JNI invocation interface",
    invocation_fields,
    ("src/embed.rs",),
    {field for field in invocation_fields if field.startswith("reserved")},
)

jvmti_fields = struct_fields("src/sys/jvmti.rs", "jvmtiInterface_1_")
check(
    "JVM TI function interface",
    jvmti_fields,
    ("src/jvmti_wrapper.rs", "src/advanced"),
    {field for field in jvmti_fields if field.startswith("reserved")},
)

callback_fields = struct_fields("src/sys/jvmti.rs", "jvmtiEventCallbacks")
check(
    "JVM TI callback table",
    callback_fields,
    ("src/lib.rs",),
    {field for field in callback_fields if field.startswith("reserved")},
)
PY
