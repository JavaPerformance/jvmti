# API Stability Checklist

This checklist defines the stability expectations for 2.x releases and the criteria for API changes.

## 2.x Stability Rules

1. Preserve documented public APIs within the 2.x line.
2. Deprecate before removal where possible; removals require a major release.
3. Keep `env` APIs stable and ergonomic.
4. Keep `sys` in sync with upstream JNI/JVMTI headers.
5. Feature-gated modules may grow faster, but must document behavior changes.

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
