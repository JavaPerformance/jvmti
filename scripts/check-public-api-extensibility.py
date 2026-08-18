#!/usr/bin/env python3
"""Reject public API shapes that would force avoidable 3.x source breaks."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src"
EVOLVING_RAW_TYPES = {
    "JNINativeInterface_": SRC / "sys" / "jni.rs",
    "JNIInvokeInterface_": SRC / "sys" / "jni.rs",
    "jvmtiCapabilities": SRC / "sys" / "jvmti.rs",
    "jvmtiInterface_1_": SRC / "sys" / "jvmti.rs",
    "jvmtiEventCallbacks": SRC / "sys" / "jvmti.rs",
}
BASELINE_REQUIRED_AGENT_HOOKS = {"on_load"}


def preceding_attributes(lines: list[str], index: int) -> str:
    start = index
    while start > 0:
        candidate = lines[start - 1].strip()
        if candidate.startswith("#[") or candidate.startswith("///") or not candidate:
            start -= 1
            continue
        break
    return "\n".join(lines[start:index])


def item_body(lines: list[str], start: int) -> str:
    depth = 0
    opened = False
    collected: list[str] = []
    for line in lines[start:]:
        collected.append(line)
        depth += line.count("{") - line.count("}")
        opened = opened or "{" in line
        if opened and depth == 0:
            break
    return "\n".join(collected)


def audit_source_items() -> tuple[int, int, list[str]]:
    enum_count = 0
    record_count = 0
    errors: list[str] = []
    for path in sorted(SRC.rglob("*.rs")):
        lines = path.read_text(encoding="utf-8").splitlines()
        relative = path.relative_to(ROOT)
        for index, line in enumerate(lines):
            enum_match = re.match(r"\s*pub enum\s+([A-Za-z_][A-Za-z0-9_]*)", line)
            if enum_match:
                enum_count += 1
                if "#[non_exhaustive]" not in preceding_attributes(lines, index):
                    errors.append(
                        f"{relative}:{index + 1}: public enum "
                        f"{enum_match.group(1)} must be #[non_exhaustive]"
                    )

            struct_match = re.match(
                r"\s*pub struct\s+([A-Za-z_][A-Za-z0-9_]*)", line
            )
            if not struct_match or path.parent.name == "sys":
                continue
            body = item_body(lines, index)
            if not re.search(r"(?m)^\s+pub\s+[A-Za-z_][A-Za-z0-9_]*\s*:", body):
                continue
            record_count += 1
            if "#[non_exhaustive]" not in preceding_attributes(lines, index):
                errors.append(
                    f"{relative}:{index + 1}: public data record "
                    f"{struct_match.group(1)} must be #[non_exhaustive]"
                )
    return enum_count, record_count, errors


def audit_raw_types() -> list[str]:
    errors: list[str] = []
    for name, path in EVOLVING_RAW_TYPES.items():
        lines = path.read_text(encoding="utf-8").splitlines()
        matches = [
            index
            for index, line in enumerate(lines)
            if re.match(rf"\s*pub struct\s+{re.escape(name)}\b", line)
        ]
        if len(matches) != 1:
            errors.append(f"{path.relative_to(ROOT)}: expected one definition of {name}")
            continue
        index = matches[0]
        if "#[non_exhaustive]" not in preceding_attributes(lines, index):
            errors.append(
                f"{path.relative_to(ROOT)}:{index + 1}: evolving raw type "
                f"{name} must be #[non_exhaustive]"
            )
    return errors


def audit_agent_defaults() -> tuple[int, list[str]]:
    path = SRC / "lib.rs"
    text = path.read_text(encoding="utf-8")
    start = text.index("pub trait Agent:")
    end = text.index("\npub static GLOBAL_AGENT", start)
    trait = text[start:end]
    starts = list(re.finditer(r"(?m)^\s{4}fn\s+([A-Za-z_][A-Za-z0-9_]*)\b", trait))
    required: set[str] = set()
    for ordinal, match in enumerate(starts):
        stop = starts[ordinal + 1].start() if ordinal + 1 < len(starts) else len(trait)
        method = trait[match.start():stop]
        opening_brace = method.find("{")
        semicolon = method.find(";")
        if opening_brace < 0 or (semicolon >= 0 and semicolon < opening_brace):
            required.add(match.group(1))
    if required != BASELINE_REQUIRED_AGENT_HOOKS:
        return len(starts), [
            "src/lib.rs: required Agent hooks changed: "
            f"expected {sorted(BASELINE_REQUIRED_AGENT_HOOKS)}, got {sorted(required)}; "
            "future hooks must have default implementations"
        ]
    return len(starts), []


def main() -> int:
    enum_count, record_count, errors = audit_source_items()
    errors.extend(audit_raw_types())
    agent_count, agent_errors = audit_agent_defaults()
    errors.extend(agent_errors)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        "public API extensibility: "
        f"{enum_count} enums, {record_count} data records, "
        f"{len(EVOLVING_RAW_TYPES)} evolving raw types, and "
        f"{agent_count} Agent hooks "
        f"({len(BASELINE_REQUIRED_AGENT_HOOKS)} baseline required, "
        f"{agent_count - len(BASELINE_REQUIRED_AGENT_HOOKS)} default) verified"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
