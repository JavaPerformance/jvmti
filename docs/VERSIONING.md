# Versioning and API Stability

This project follows SemVer with the following policy:

1. `3.x` releases preserve the documented public API unless a safety fix makes a narrow break unavoidable.
2. Breaking public API changes require a new major version and migration notes.
3. New JVMTI/JNI helpers are added in minor releases with clear changelog notes.
4. Unsafe APIs are never silently changed; safety assumptions are documented explicitly.
5. Feature-gated helper modules may grow faster, but feature behavior is still documented.
6. New JDK table tails, reclaimed slots, and capability bits must be runtime-gated before access.

Version 2.3.x is the final source-compatible line for the old callback trait.
Version 3.0 intentionally removes reduced callbacks, `*_with_jvmti` callbacks,
and unsound raw ownership contracts. Its complete upgrade contract is
[Migrating From 2.x to 3.0](MIGRATING_2_TO_3.md).

API review goals before each minor release:

1. Public types are minimal, stable, and well-documented.
2. No unsound `Send` or `Sync` behavior.
3. All FFI allocations have clear ownership and cleanup.
4. Examples and docs match the released crate name and feature flags.
