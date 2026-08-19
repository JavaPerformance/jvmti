# Release Procedure

This procedure applies to 3.0.0 and every later 3.x release. Version 3.0 is the
last foreseeable broad API reset, not a promise that maintenance releases will
stop.

## Candidate

1. Work from a clean code-candidate commit.
2. Confirm `Cargo.toml`, `CHANGELOG.md`, and the intended tag agree.
3. Review `api/jvmti-bindings-3.0.0.txt` and run
   `scripts/check-public-api-baseline.sh`.
4. If the unsafe surface changed, leave the reviewed baseline unchanged and
   give the independent reviewer both the code candidate and the proposed
   baseline diff. The author of the candidate must not self-approve this gate.
5. Record the independent decision in an immutable external review that names
   the exact code-candidate commit.
6. After acceptance, regenerate the baseline with
   `scripts/check-unsafe-surface.py --write` and commit only the reviewed
   evidence and any explicitly requested corrections.
7. Have the independent reviewer confirm the final evidence-only delta and
   record acceptance of the exact final commit in the external review.
8. Run `scripts/run-release-gates.sh` from that clean final commit.
9. Push the final commit and require every GitHub Actions job to pass on that
   exact revision.

Do not try to place a commit's own hash in tracked content. The immutable
external review, signed tag, and hosted build evidence identify the exact final
revision without creating a self-referential commit loop.

## Tag And Evidence

1. Create a signed, annotated `vX.Y.Z` tag on the accepted commit.
2. Push the tag. `.github/workflows/release-evidence.yml` builds the `.crate`,
   SPDX SBOM, and `SHA256SUMS` from that exact revision.
3. Download the workflow artifact and verify its GitHub attestations before
   creating a GitHub release or publishing to crates.io.
4. Run `cargo +1.85.0 publish --dry-run --locked` from the clean tag.
5. Publish only the exact attested `.crate`; do not rebuild it locally.

GitHub attestations are cryptographic build-provenance and SBOM statements.
They complement rather than replace a signed Git tag and checksum review.

## 3.x Compatibility

After `v3.0.0` exists, every proposed 3.x release must run:

```bash
scripts/check-3x-api-compat.sh v3.0.0 HEAD
```

The gate combines `cargo-semver-checks` with a full-signature
`cargo-public-api` diff. A new JDK may be added only after official source is
pinned and the ABI, runtime-version, callback, and live-test ledgers are
extended.

An unavoidable soundness correction may be listed in
`api/approved-3x-soundness-breaks.json`. The compatibility gate still runs
`cargo-semver-checks`, parses its complete failure set, and fails unless every
reported lint/item pair exactly matches that reviewed allowlist. Never disable
a SemVer lint globally to admit one correction.

## External Boundaries

Local success does not prove hosted runner behavior, every supported JVM, or
an independent review. Record these separately in the release notes:

- local conformance result;
- hosted Linux x86-64/AArch64, macOS, and Windows result;
- installed-JDK live matrix;
- latest-source-only JDK evidence;
- independent unsafe/FFI reviewer and reviewed commit.
