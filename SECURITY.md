# Security Policy

`jvmti-bindings` crosses JNI and JVM TI foreign-function boundaries inside the
JVM process. Memory-safety, ABI, ownership, callback, and lifecycle defects are
therefore treated as security-sensitive even when no exploit is known.

## Supported Versions

| Version | Security support |
|---------|------------------|
| 3.x | Supported |
| 2.x | Unsupported |
| Earlier | Unsupported |

## Reporting A Vulnerability

Use GitHub's private vulnerability reporting for this repository:

<https://github.com/JavaPerformance/jvmti/security/advisories/new>

Do not open a public issue for a suspected memory-safety, ABI, use-after-free,
double-release, callback-boundary, or native-loading vulnerability. Include:

1. The crate version, operating system, architecture, Rust version, and JDK.
2. A minimal reproducer or the smallest failing callback/wrapper sequence.
3. Whether startup loading, dynamic attach, or embedded-JVM mode is involved.
4. Sanitizer, JVM crash, or native backtrace output when available.

An acknowledgement should be expected within seven days. Disclosure timing is
coordinated after impact and remediation are understood.

## Release Integrity

Release artifacts include SHA-256 checksums, an SPDX SBOM, and GitHub artifact
attestations. Verify an attestation with:

```bash
gh attestation verify jvmti-bindings-<version>.crate \
  --repo JavaPerformance/jvmti
```

The crate intentionally has zero third-party Cargo dependencies. Build and
conformance tools used by maintainers are not linked into consumer artifacts.
