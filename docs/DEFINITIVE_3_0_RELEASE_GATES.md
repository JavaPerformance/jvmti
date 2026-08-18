# Definitive 3.0 Release Gates

Status: release-candidate policy
Updated: 2026-08-18

## Intent

Version 3.0 is intended to be the last foreseeable major API reset. The 3.x
line must absorb additive JNI, JVM TI, and Java class-file evolution through
minor releases without repeating the callback and raw-ABI break made here.

This is not a promise that no major release can ever be necessary. A future
incompatible upstream ABI or a newly discovered soundness defect takes
precedence over source compatibility. It is a commitment that predictable JDK
growth, new callbacks, table tails, capability bits, and open numeric values do
not require another major release.

## What the Callback Gap Exposed

The 2.3 JIT callbacks discarded their originating `jvmtiEnv*`. Adding parallel
`*_with_jvmti` methods fixed those callbacks locally but exposed the systemic
problem: a hand-maintained binding and reduced high-level callback API could
silently lose native arguments without a complete conformance gate.

The 3.0 audit found analogous defects in eleven families:

1. Callback fidelity: discarded environment pointers, return values, flags,
   reserved values, and mutable outputs.
2. Raw ABI fidelity: approximate structures, pointer indirection, callback
   nullability, variadic functions, constants, and open numeric domains.
3. String fidelity: treating Java Modified UTF-8 as ordinary UTF-8 and losing
   unpaired Java UTF-16 surrogate code units.
4. Ownership: allocations, JNI references, raw monitors, and transformation
   output whose cleanup or transfer was incomplete on failure paths.
5. Lifecycle: assuming one attach invocation and reconstructing or rejecting
   state even though the JVM can invoke `Agent_OnAttach` repeatedly.
6. Platform ABI: assuming C `va_list` is pointer-shaped on every target and
   assuming little-endian C bitfield allocation for JVM TI capabilities.
7. Lifetime encoding: returning an unscoped embedded `JniEnv` from safe APIs,
   allowing safe code to retain it after detach or VM destruction.
8. Input robustness: permitting adversarial recursive annotation values to
   consume the native stack in the class-file parser.
9. Wrapper completeness: leaving fixed-signature raw JNI operations without a
   high-level counterpart and relying on manual review to notice the gap.
10. Native lease ownership: exposing a native pointer without encoding its
    matching JNI release operation, mode, and critical-region restrictions.
11. Evidence provenance and resource accounting: allowing a changed OpenJDK
    pin to reuse stale cached headers and undercharging parser allocations made
    along failure or exact-string paths.

Version 3.0 removes the parallel callback API. Every standard event has one
canonical callback with a callback-scoped context and complete typed payload.

## Mechanical Regression Barriers

The release must keep all of these gates:

1. All 34 standard non-reserved JVM TI callbacks are invoked by a sentinel that
   verifies the exact `jvmtiEnv*`, JNI availability, every payload field,
   mutable outputs, and panic containment.
2. Compiler assignments compare all 440 JNI/JVM TI table fields against the
   pinned OpenJDK headers: 237 JNI native slots, 8 JNI invocation slots, 156
   JVM TI functions, and 39 JVM TI callback slots.
3. Native record checks compare kind, size, alignment, and field order for 31
   public records and offsets for 562 fields.
4. Header inventory, constant-value, event-number, callback-prefix, capability
   bit, and version-gating tests reject incomplete or shifted declarations.
5. Open native numeric domains preserve unknown future values rather than
   creating invalid Rust enum discriminants.
6. Runtime gates reject access before reading an appended table tail, reclaimed
   slot, or newly consumed capability bit on an older JVM.
7. Ownership tests count create/delete, allocate/deallocate, close/retry, and
   failure paths for JNI references, JVM TI buffers, and raw monitors.
8. Live proofs cover callback delivery, allocation-free normal callback
   dispatch, Java Modified UTF-8, repeated attach, and heap traversal.
9. The dependency gate verifies zero third-party Cargo dependencies across all
   features and development targets. Repository conformance CI may install an
   external `bindgen` executable as an independent header oracle; consumers do
   not build or run bindgen.
10. Linux x86-64 and AArch64 CI run an executable C-to-Rust-to-C `va_list`
    forwarding proof; capability tests cover little- and big-endian bitfield
    numbering and unknown-bit preservation.
11. The packaged external-consumer proof compiles both startup and dynamic
    attach entry points and uses strict Java Modified UTF-8 handling.
12. The class-file parser enforces shared input-size, cumulative-allocation,
    recursive-attribute, and recursive-annotation budgets, and survives a
    deterministic mutation corpus without panic.
13. `scripts/check-wrapper-coverage.sh` inventories every native and invocation
    JNI slot, JVM TI function slot, and callback slot. Missing wrappers must be
    explicitly and minimally classified as reserved or raw-only; stale
    exceptions fail.
14. JNI array-element, critical-region, local-frame, and Java-monitor guards
    count acquisition, commit, abort, close/pop/exit, failure-retry, and drop
    behavior. They prove no second release after success and one best-effort
    retry after an observable explicit-release failure.
15. Installed JDK class-file corpora parse with zero failures. The current
    evidence is 186,968 classes across complete JDK 8, 11, 17, 21, 25, 27, and 28
    runtime images. The OpenJDK source fetch cache is keyed by the manifest's
    feature, tag, and immutable commit rather than only by feature number.
16. `scripts/check-wrapper-forwarding.py` fails closed if any of the 237 direct
    hand-written wrappers drops, aliases, or reorders an input, and accounts
    for all 99 hand-written helper methods. It separately pins all 13 JNI
    wrapper macro families and the reviewed transformed or delegated helpers
    whose forwarding cannot be inferred directly from the native call
    expression.
17. Every public source rustdoc example compiles as `no_run` or is an
    intentional `compile_fail` safety proof. Public examples are not hidden
    behind `ignore`; the external starter and packaged-crate gates cover the
    complete agent-project shape.
18. The attach-policy proof distinguishes startup loading, explicitly denied
    dynamic attach, and explicitly enabled dynamic attach on runtimes that
    implement `EnableDynamicAgentLoading`.
19. `scripts/check-public-api-extensibility.py` rejects exhaustive public
    enums and data records, new required `Agent` hooks, or constructible native
    tables whose upstream layouts can grow. This keeps foreseeable JDK growth
    additive throughout 3.x without changing any C layout.

## Publication Gates

Do not publish 3.0.0 until all applicable gates pass from a clean release
candidate commit:

1. `cargo +1.85.0 fmt --all -- --check`
2. `cargo +1.85.0 check --locked --all-targets --all-features`
3. `cargo +1.85.0 test --locked`
4. `cargo +1.85.0 test --locked --all-features`
5. `cargo +1.85.0 test --locked --doc --all-features`
6. `cargo +1.85.0 clippy --locked --all-targets --all-features -- -D warnings`
7. `RUSTDOCFLAGS='-D warnings' cargo +1.85.0 doc --locked --no-deps --all-features`
8. The same all-target/all-feature checks on current stable Rust.
9. Linux, macOS, and Windows CI on Rust 1.85 and stable.
10. `scripts/check-zero-dependencies.sh`, `scripts/check-agent-template.sh`,
    `scripts/check-host-va-list-abi.sh`, and
    `scripts/check-wrapper-coverage.sh`, and
    `scripts/check-wrapper-forwarding.py`, and
    `scripts/check-public-api-extensibility.py`.
11. `scripts/check-jdk-abi.sh --all-releases` for every pinned JDK 8-28 source.
12. `scripts/check-pinned-jdk-abi.sh 28`, including all signatures, layouts,
    constants, inventories, and capability bits.
13. `scripts/prove-event-callback-matrix.sh` on installed supported JVMs.
14. `scripts/prove-callback-allocation-free.sh` in single-thread and concurrent
    modes.
15. `scripts/prove-mutf8-live.sh`, `scripts/prove-repeated-attach-live.sh`, and
    `scripts/prove-attach-policy-live.sh`, and
    `scripts/prove-heap-graph-live.sh`.
16. `scripts/check-packaged-agent-template.sh` builds an external agent against
    the exact `.crate` archive rather than the repository path, and rejects
    generated caches, build trees, Git internals, and `.env` files in that
    archive.
17. `cargo +1.85.0 publish --dry-run` succeeds and package review finds no
    generated headers, local artifacts, secrets, or accidental files.
18. `scripts/capture-public-api-baseline.sh` generates the full public-signature
    report, which is reviewed with the compile-tested API fixture and saved as
    the 3.0 baseline. The lightweight name-only inventory is diagnostic, not a
    SemVer proof.
19. `scripts/check-classfile-corpus.sh` parses complete installed runtime
    images, not only hand-selected classes, and reports zero failed classes.
20. Rustdoc, doctests, corpus parsing, dependency checks, and packaged-consumer
    compilation run in CI on both Rust 1.85 and current stable, rather than
    relying on one toolchain to stand in for the other.
21. The complete all-feature test suite passes AddressSanitizer with leak
    detection. Pinned hosted Miri proofs cover Modified UTF-8 validation, JVM TI
    allocation ownership, JNI reference and guard ownership, and raw-monitor
    ownership; neither tool substitutes for the cross-JDK live matrix.
22. `scripts/check-downstream-canaries.sh` compiles startup/attach and embedded
    JVM consumers against the exact packaged crate, not the repository crate.
23. `scripts/check-public-api-baseline.sh` matches the reviewed 3.0 signature
    baseline before the tag; later 3.x releases additionally compare against
    the signed `v3.0.0` revision.
24. The release workflow emits a `.crate`, SPDX 2.3 SBOM, and SHA-256 manifest,
    and creates GitHub build-provenance and SBOM attestations for those exact
    artifacts.
25. An independent reviewer accepts the unsafe/FFI checklist in
    `docs/UNSAFE_FFI_REVIEW.md` for the exact candidate commit. Automated
    conformance is evidence for that review, not a substitute for it.
26. `scripts/check-unsafe-surface.py` fails closed when an unsafe-sensitive
    production source line changes. Its baseline is regenerated only after the
    unsafe/FFI review, so new native boundaries cannot enter as incidental
    refactoring.

Valgrind is not a claimed gate on a host where Valgrind cannot execute a trivial
binary. Sanitizer evidence is additive and must be reported with its actual
platform/runtime scope rather than treated as universal proof.

JDK 28 build `28+7` has passed the live preview-runtime matrix recorded in
`docs/JDK_28_LIVE_PROOF_2026-08-18.md`. Preview value-object behavior is still
not described as final until its release line and semantics are final; the
specialized proof establishes the documented preview-runtime scope only.

## 3.x Compatibility Discipline

After the 3.0.0 tag:

1. Compare every 3.x release's public symbol inventory and compile-tested API
   fixture against the 3.0.0 baseline with
   `scripts/check-3x-api-compat.sh`. It combines `cargo-semver-checks` with a
   full `cargo-public-api` signature diff because neither a name list nor one
   lint engine is a complete compatibility proof.
2. Prefer private fields, constructors, accessors, `#[non_exhaustive]` payloads,
   open integer newtypes, and runtime feature gates.
3. Add JDK features in minor releases only after pinning official source and
   extending the release ledger, raw conformance checks, runtime gates, and live
   proofs.
4. Never add a reduced callback or a second suffixed callback surface. Extend a
   crate-constructed non-exhaustive payload when the native event grows.
5. Never infer support for an unpublished JDK from the previous main-line
   snapshot. JDK 29 enters the compatibility matrix only after an identifiable
   official source revision passes the complete gate.
6. Deprecate before removal wherever safety permits. A soundness fix may make a
   narrow break, but must include an explicit migration and release note.
7. Treat the Rust 1.85 MSRV and zero-dependency contract as public 3.x policy.

The expected result is not "no more releases." It is a durable 3.x family in
which maintenance, new JDK support, and additive ergonomics do not force users
through another broad migration.
