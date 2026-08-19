# Versioning and API Stability

This project follows SemVer with the following policy:

Version 3.0 is intended to be the last foreseeable broad API reset. That means
new JDK releases should normally enter the 3.x line through additive,
runtime-gated minor releases; it is not a promise to retain an unsound API or
to avoid a major version after a genuinely incompatible upstream change.

1. `3.x` releases preserve the documented public API unless a safety fix makes a narrow break unavoidable.
2. Breaking public API changes require a new major version and migration notes.
3. New JVMTI/JNI helpers are added in minor releases with clear changelog notes.
4. Unsafe APIs are never silently changed; safety assumptions are documented explicitly.
5. Feature-gated helper modules may grow faster, but feature behavior is still documented.
6. New JDK table tails, reclaimed slots, and capability bits must be runtime-gated before access.
7. Version 3.0 has an MSRV of Rust 1.85 and uses Edition 2024. Any 3.x MSRV
   increase is a documented minor-release change, not an incidental CI update.
8. Zero third-party crates across all features and development targets is part
   of the 3.x product contract. A future dependency requires an explicit
   changelog entry and supply-chain review.
9. Every 3.x release is compared with the tagged 3.0.0 full public-signature
   and compile-tested API baseline. A name-only symbol list is insufficient.
   Predictable JDK evolution is not grounds for a breaking callback redesign.
10. Every raw table slot must remain mechanically classified as high-level
    wrapped, reserved, or deliberately raw-only. New fixed-signature upstream
    operations normally receive additive high-level wrappers in a 3.x minor.
11. Native leases and parser resource budgets remain encoded in owning or
    bounded APIs; convenience methods must not bypass those contracts.

Version 2.3.x is the final source-compatible line for the old callback trait.
Version 3.0 intentionally removes reduced callbacks, `*_with_jvmti` callbacks,
and unsound raw ownership contracts. Its complete upgrade contract is
[Migrating From 2.x to 3.0](MIGRATING_2_TO_3.md).

API review goals before each minor release:

1. Public types are minimal, stable, and well-documented.
2. No unsound `Send` or `Sync` behavior.
3. All FFI allocations have clear ownership and cleanup.
4. Examples and docs match the released crate name and feature flags.

The definitive 3.x release and maintenance gates are in
[Definitive 3.0 Release Gates](DEFINITIVE_3_0_RELEASE_GATES.md). Adding a newer
JDK is a documented minor release under [Compatibility](COMPATIBILITY.md)
“Next JDK Policy”, not an inferred bump from `master`.
