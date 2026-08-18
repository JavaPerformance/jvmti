#!/usr/bin/env python3
"""Generate a deterministic, dependency-free SPDX 2.3 SBOM for the crate."""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import subprocess
import tomllib


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git(root: pathlib.Path, *args: str) -> str:
    return subprocess.check_output(
        ["git", "-C", str(root), *args], text=True
    ).strip()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--archive", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()

    root = pathlib.Path(__file__).resolve().parent.parent
    archive = args.archive.resolve()
    if not archive.is_file():
        raise SystemExit(f"crate archive does not exist: {archive}")

    with (root / "Cargo.toml").open("rb") as stream:
        package = tomllib.load(stream)["package"]

    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
            cwd=root,
            text=True,
        )
    )["packages"][0]
    if metadata["dependencies"]:
        raise SystemExit("release SBOM expected zero Cargo dependencies")

    revision = git(root, "rev-parse", "HEAD")
    epoch = int(
        os.environ.get(
            "SOURCE_DATE_EPOCH", git(root, "show", "-s", "--format=%ct", "HEAD")
        )
    )
    created = datetime.datetime.fromtimestamp(
        epoch, datetime.timezone.utc
    ).isoformat(timespec="seconds").replace("+00:00", "Z")
    archive_digest = sha256(archive)
    name = package["name"]
    version = package["version"]
    repository = package["repository"].rstrip("/")
    package_id = "SPDXRef-Package-jvmti-bindings"

    document = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"{name}-{version}",
        "documentNamespace": (
            f"{repository}/spdx/{version}/{revision}/{archive_digest[:16]}"
        ),
        "creationInfo": {
            "created": created,
            "creators": ["Tool: scripts/generate-release-sbom.py"],
        },
        "packages": [
            {
                "name": name,
                "SPDXID": package_id,
                "versionInfo": version,
                "downloadLocation": "NOASSERTION",
                "filesAnalyzed": False,
                "licenseConcluded": package["license"],
                "licenseDeclared": package["license"],
                "copyrightText": "NOASSERTION",
                "checksums": [
                    {"algorithm": "SHA256", "checksumValue": archive_digest}
                ],
                "externalRefs": [
                    {
                        "referenceCategory": "PACKAGE-MANAGER",
                        "referenceType": "purl",
                        "referenceLocator": f"pkg:cargo/{name}@{version}",
                    }
                ],
                "sourceInfo": f"Git revision {revision}; zero Cargo dependencies",
            }
        ],
        "relationships": [
            {
                "spdxElementId": "SPDXRef-DOCUMENT",
                "relationshipType": "DESCRIBES",
                "relatedSpdxElement": package_id,
            }
        ],
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(f"wrote SPDX 2.3 SBOM: {args.output}")


if __name__ == "__main__":
    main()
