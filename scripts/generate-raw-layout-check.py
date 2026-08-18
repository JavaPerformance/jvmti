#!/usr/bin/env python3
"""Generate exhaustive record layout checks from bindgen and in-tree bindings."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


def records(source: str) -> dict[str, tuple[str, list[str]]]:
    found: dict[str, tuple[str, list[str]]] = {}
    pattern = re.compile(r"^pub (struct|union) ([A-Za-z_]\w*)\s*\{", re.MULTILINE)
    for match in pattern.finditer(source):
        kind, name = match.groups()
        cursor = match.end()
        depth = 1
        while cursor < len(source) and depth:
            if source[cursor] == "{":
                depth += 1
            elif source[cursor] == "}":
                depth -= 1
            cursor += 1
        if depth:
            raise SystemExit(f"unterminated Rust {kind}: {name}")
        block = source[match.end() : cursor - 1]
        fields = re.findall(r"^\s*pub\s+([A-Za-z_]\w*)\s*:", block, re.MULTILINE)
        found[name] = (kind, fields)
    return found


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bindgen", required=True, type=Path)
    parser.add_argument("--jni-rust", required=True, type=Path)
    parser.add_argument("--jvmti-rust", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    canonical = records(args.bindgen.read_text())
    jni = records(args.jni_rust.read_text())
    jvmti = records(args.jvmti_rust.read_text())
    ours = {**jni, **jvmti}

    # Capability fields are C bitfields and intentionally represented as an
    # opaque word array. Every individual bit is checked by abi_conformance.
    excluded = {"jvmtiCapabilities"}
    common = sorted((set(canonical) & set(ours)) - excluded)
    if len(common) != 31:
        raise SystemExit(f"expected 31 common native records, found {len(common)}")

    lines = [
        "use std::mem::{align_of, offset_of, size_of};",
        "use jvmti_bindings::sys::{jni, jvmti};",
        "",
        "fn main() {",
    ]
    field_count = 0
    for name in common:
        canonical_kind, canonical_fields = canonical[name]
        rust_kind, rust_fields = ours[name]
        if canonical_kind != rust_kind:
            raise SystemExit(
                f"{name}: native kind {canonical_kind} != Rust kind {rust_kind}"
            )
        if canonical_fields != rust_fields:
            raise SystemExit(
                f"{name}: native fields {canonical_fields} != Rust fields {rust_fields}"
            )
        module = "jni" if name in jni else "jvmti"
        native_type = f"canonical::{name}"
        rust_type = f"{module}::{name}"
        lines.append(
            f'    assert_eq!(size_of::<{native_type}>(), size_of::<{rust_type}>(), "size.{name}");'
        )
        lines.append(
            f'    assert_eq!(align_of::<{native_type}>(), align_of::<{rust_type}>(), "align.{name}");'
        )
        for field in canonical_fields:
            lines.append(
                f'    assert_eq!(offset_of!({native_type}, {field}), '
                f'offset_of!({rust_type}, {field}), "offset.{name}.{field}");'
            )
            field_count += 1
    lines.append("}")
    args.out.write_text("\n".join(lines) + "\n")
    print(
        f"generated size/alignment checks for {len(common)} records and "
        f"offset checks for {field_count} fields"
    )


if __name__ == "__main__":
    main()
