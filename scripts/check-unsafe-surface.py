#!/usr/bin/env python3
"""Fail closed when an unsafe-sensitive source line changes without review."""

from __future__ import annotations

import argparse
import difflib
import pathlib
import re


UNSAFE = re.compile(r"\bunsafe\b")


def inventory(root: pathlib.Path) -> list[str]:
    entries: list[str] = []
    for path in sorted((root / "src").rglob("*.rs")):
        relative = path.relative_to(root).as_posix()
        for line in path.read_text(encoding="utf-8").splitlines():
            if UNSAFE.search(line):
                normalized = " ".join(line.split())
                entries.append(f"{relative}\t{normalized}")
    return entries


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    parser.add_argument(
        "--baseline", default="audits/unsafe-surface-3.0.txt", type=pathlib.Path
    )
    args = parser.parse_args()

    root = pathlib.Path(__file__).resolve().parent.parent
    baseline = root / args.baseline
    current = inventory(root)
    text = "\n".join(current) + "\n"

    if args.write:
        baseline.parent.mkdir(parents=True, exist_ok=True)
        baseline.write_text(text, encoding="utf-8")
        print(f"wrote unsafe-sensitive baseline: {baseline} ({len(current)} lines)")
        return

    if not baseline.is_file():
        raise SystemExit(f"missing unsafe-sensitive baseline: {baseline}")
    expected = baseline.read_text(encoding="utf-8").splitlines()
    if expected != current:
        diff = difflib.unified_diff(
            expected,
            current,
            fromfile=str(args.baseline),
            tofile="current unsafe-sensitive source",
            lineterm="",
        )
        print("\n".join(diff))
        raise SystemExit(
            "unsafe surface changed; perform the FFI review and deliberately "
            "regenerate the baseline with --write"
        )
    print(f"unsafe-sensitive surface matches baseline: {len(current)} lines")


if __name__ == "__main__":
    main()
