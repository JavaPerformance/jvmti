#!/usr/bin/env python3
"""Fail closed unless semver-checks reports only reviewed soundness breaks."""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=pathlib.Path)
    parser.add_argument("approvals", type=pathlib.Path)
    args = parser.parse_args()

    report = args.report.read_text(encoding="utf-8")
    approvals = json.loads(args.approvals.read_text(encoding="utf-8"))
    expected = {
        (entry["lint"], entry["item"])
        for entry in approvals.get("approved_breaks", [])
    }
    if not expected:
        raise SystemExit("soundness-break approval list is empty")

    headers = list(
        re.finditer(r"^--- failure ([a-z0-9_]+):", report, re.MULTILINE)
    )
    actual: set[tuple[str, str]] = set()
    for index, header in enumerate(headers):
        end = headers[index + 1].start() if index + 1 < len(headers) else len(report)
        block = report[header.end() : end]
        for item in re.findall(r"^  ([^\n]+?) in /[^\n]+:\d+$", block, re.MULTILINE):
            actual.add((header.group(1), item))

    if actual != expected:
        print("unexpected SemVer break set", file=sys.stderr)
        print(f"expected: {sorted(expected)}", file=sys.stderr)
        print(f"actual:   {sorted(actual)}", file=sys.stderr)
        return 1

    manifest = pathlib.Path("Cargo.toml").read_text(encoding="utf-8")
    version = re.search(r'^version = "([^"]+)"$', manifest, re.MULTILINE)
    approved_versions = {entry["version"] for entry in approvals["approved_breaks"]}
    if version is None or approved_versions != {version.group(1)}:
        print(
            "soundness-break approval version does not match Cargo.toml",
            file=sys.stderr,
        )
        return 1

    expected_count = len(expected)
    summary = re.search(
        r"Summary semver requires new major version: (\d+) major and (\d+) minor checks failed",
        report,
    )
    if summary is None or (int(summary.group(1)), int(summary.group(2))) != (
        expected_count,
        0,
    ):
        print("SemVer failure summary does not match the approval list", file=sys.stderr)
        return 1

    for entry in approvals["approved_breaks"]:
        print(
            "approved soundness break: "
            f"{entry['item']} ({entry['lint']}, {entry['version']})"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
