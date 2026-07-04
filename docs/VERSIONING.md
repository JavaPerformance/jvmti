# Versioning and API Stability

This project follows SemVer with the following policy:

1. `2.x` releases preserve the documented public API unless a safety fix makes a narrow break unavoidable.
2. Breaking public API changes require a new major version and migration notes.
3. New JVMTI/JNI helpers are added in minor releases with clear changelog notes.
4. Unsafe APIs are never silently changed; safety assumptions are documented explicitly.
5. Feature-gated helper modules may grow faster, but feature behavior is still documented.

API review goals before each minor release:

1. Public types are minimal, stable, and well-documented.
2. No unsound `Send` or `Sync` behavior.
3. All FFI allocations have clear ownership and cleanup.
4. Examples and docs match the released crate name and feature flags.
