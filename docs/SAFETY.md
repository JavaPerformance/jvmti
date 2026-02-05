# Safety and FFI Checklist

This document captures the safety model for `jvmti-bindings` and a checklist to review before shipping an agent.

## Safety and FFI Principles

1. Treat all JNI/JVMTI callbacks as `unsafe` boundaries.
2. Never panic across a JNI/JVMTI callback boundary.
3. Do not store or share `JNIEnv` across threads.
4. Use `GlobalRef` for long-lived references and ensure they are released.
5. Always check JVMTI error codes and handle failures explicitly.
6. Assume callbacks can be concurrent and re-entrant.
7. Avoid long-running work inside callbacks; offload to worker threads.
8. Respect callback-specific constraints (some callbacks forbid JNI).

## Agent Safety Checklist

1. Capabilities requested in `on_load` match the events you enable.
2. Event callbacks are registered before enabling notifications.
3. `JNIEnv` is only used on the thread that provided it.
4. No `unwrap()` or panics in callback code paths.
5. All JVMTI-allocated memory is deallocated via `Jvmti::deallocate`.
6. Global and local references are cleaned up.
7. Agent state is thread-safe (`Mutex`, atomics, or lock-free).
8. You avoid JNI calls during `GarbageCollectionStart/Finish` callbacks.
9. Native method redirects or bytecode rewriting are validated and bounded.
10. Any attach-based initialization is idempotent.

## FFI Review Reminders

1. Never assume null termination or UTF-8 validity for JVM-provided strings.
2. Treat pointer lifetimes as scoped to the callback unless documented otherwise.
3. Validate lengths before copying into Rust buffers.
4. Prefer owned Rust structures over returning raw JVMTI structs.
