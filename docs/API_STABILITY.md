# API Stability Checklist

This checklist defines the stability expectations for 3.x releases and the criteria for API changes.

The intentional 2.x-to-3.0 break and its mechanical replacements are recorded
in [Migrating From 2.x to 3.0](MIGRATING_2_TO_3.md). Version 2.3.x is the final
compatibility line for the old callback trait.

## 3.x Stability Rules

1. Preserve documented public APIs within the 3.x line.
2. Deprecate before removal where possible; removals require a major release.
3. Keep `env` APIs stable and ergonomic.
4. Keep `sys` in sync with upstream JNI/JVMTI headers.
5. Feature-gated modules may grow faster, but must document behavior changes.
6. Rust 1.85 is the 3.0 MSRV. A later 3.x MSRV increase requires a documented
   minor release, a concrete safety or maintenance benefit, and CI coverage for
   the new floor.
7. The zero-third-party-crate contract covers every feature and development
   target. Adding a crate requires an explicit public policy change.
8. Compare full public signatures and the compile-tested API fixture against
   the tagged 3.0.0 baseline before every 3.x release. A name-only inventory is
   diagnostic and cannot establish SemVer compatibility.
9. Extend crate-constructed, non-exhaustive callback payloads for additive JDK
   evolution. Do not add reduced or suffixed parallel callback methods.
10. Keep append-only raw function and callback tables non-exhaustive at the
    Rust source boundary; upstream table growth is additive within 3.x.

## Review Checklist for Any Public API Change

1. Does this change break existing code? If yes, can we avoid it?
2. Is there a migration path or deprecation notice?
3. Are docs/examples updated to the new API?
4. Are safety assumptions updated?
5. Are tests updated or added?

## Minor Release Gates

1. Public surface area is documented and intentional.
2. Unsafe boundaries are minimal and clearly documented.
3. No unsound `Send` or `Sync` behavior.
4. All JVMTI allocations have explicit ownership.
5. Examples cover core workflows (profiling, tracing, heap sampling).
6. CI green on Linux/macOS/Windows.
7. `cargo publish --dry-run` succeeds.
8. Raw ABI probes pass against every maintained JDK header generation.
9. Every additive JNI/JVM TI operation is rejected before touching an older table.
10. Migration examples compile and the migration inventory still covers every 3.0 source break.
11. The crate builds with all features on the declared MSRV.
12. The complete raw signature, layout, constant, inventory, capability-bit,
    callback-sentinel, and live lifecycle gates remain green.
13. Linux x86-64 and AArch64 native ABI jobs pass, including the executable
    host `va_list` forwarding proof.
14. Wrapper coverage accounts for every JNI/JVM TI table and callback slot;
    raw-only and reserved exceptions are explicit, minimal, and non-stale.
15. JNI native-storage leases remain RAII-owned, and class-file resource limits
    plus mutation and real-runtime corpus tests remain green.
16. Every pinned OpenJDK fetch is tied to the manifest's immutable commit, not
    merely to a cached feature-number directory.
17. Public API extensibility and complete wrapper-forwarding gates remain
    green; a new callback hook must have a default implementation.
18. The reviewed unsafe-sensitive source baseline is unchanged, or an
    independent FFI review explicitly accepts and regenerates it.
19. Both packaged downstream canaries compile against the release artifact,
    not merely against the repository checkout.

See [Definitive 3.0 Release Gates](DEFINITIVE_3_0_RELEASE_GATES.md) for the
release-candidate command set and long-lived 3.x intake policy.
