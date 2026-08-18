# Release Procedure

This procedure applies to 3.0.0 and every later 3.x release. Version 3.0 is the
last foreseeable broad API reset, not a promise that maintenance releases will
stop.

## Candidate

1. Work from a clean release-candidate commit.
2. Confirm `Cargo.toml`, `CHANGELOG.md`, and the intended tag agree.
3. Run `scripts/run-release-gates.sh`.
4. Review `api/jvmti-bindings-3.0.0.txt` and run
   `scripts/check-public-api-baseline.sh`.
5. Obtain an independent review of `docs/UNSAFE_FFI_REVIEW.md`; the author of
   the release candidate must not self-approve this gate.
6. Push the candidate and require every GitHub Actions job to pass on the exact
   commit.

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

## External Boundaries

Local success does not prove hosted runner behavior, every supported JVM, or
an independent review. Record these separately in the release notes:

- local conformance result;
- hosted Linux x86-64/AArch64, macOS, and Windows result;
- installed-JDK live matrix;
- latest-source-only JDK evidence;
- independent unsafe/FFI reviewer and reviewed commit.
