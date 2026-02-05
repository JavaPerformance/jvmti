# Versioning and API Stability

This project follows SemVer with the following policy:

1. `0.x` releases may introduce breaking changes, but we aim to keep them rare and documented.
2. `1.0` will lock down public API surfaces and require explicit deprecation cycles.
3. New JVMTI/JNI features will be added behind minor releases with clear changelog notes.
4. Unsafe APIs will not be silently changed; safety assumptions are documented explicitly.

API review goals before 1.0:

1. Public types are minimal, stable, and well-documented.
2. No unsound `Send` or `Sync` behavior.
3. All FFI allocations have clear ownership and cleanup.
