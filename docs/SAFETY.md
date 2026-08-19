# Safety and FFI Checklist

This document captures the safety model for `jvmti-bindings` and a checklist to review before shipping an agent.

## Safety and FFI Principles

1. Treat all JNI/JVMTI callbacks as `unsafe` boundaries.
2. Never panic across a JNI/JVMTI callback boundary.
3. Do not store or share `JNIEnv` across threads.
4. For embedded JVMs, only use `creator_env` on the creating thread; attach/detach other threads.
5. Use `GlobalRef` or `WeakGlobalRef` for long-lived references and ensure they are released.
6. Always check JVMTI error codes and handle failures explicitly.
7. Assume callbacks can be concurrent and re-entrant.
8. Avoid long-running work inside callbacks; offload to worker threads.
9. Respect callback-specific constraints (some callbacks forbid JNI).
10. Keep unsafe operations narrow. The crate denies `unsafe_op_in_unsafe_fn`, so
    an unsafe function does not implicitly make its whole body an unsafe block.
11. Keep a dynamically loaded JVM library alive until every function pointer
    obtained from it is no longer callable and the embedded VM is destroyed.

## Agent Safety Checklist

1. Capabilities requested in `on_load` match the events you enable.
2. Event callbacks are registered before enabling notifications.
3. `JNIEnv` is only used on the thread that provided it.
4. No `unwrap()` or panics in callback code paths.
5. JVM TI allocations remain in `JvmtiAllocation` guards, or raw ownership is explicitly transferred and later released with unsafe `Jvmti::deallocate_raw` on the same environment.
6. Owning reference guards are closed or dropped; raw JNI/JVM TI object returns
   are explicitly deleted or bounded by a local frame, especially on
   long-running agent threads.
7. Agent state is thread-safe (`Mutex`, atomics, or lock-free).
8. You avoid JNI calls during `GarbageCollectionStart/Finish` callbacks.
9. Native method redirects or bytecode rewriting are validated and bounded.
10. Any attach-based initialization is idempotent.

## FFI Review Reminders

1. Never treat JNI, JVM TI, or `CONSTANT_Utf8` bytes as ordinary UTF-8. Native
   strings are NUL-terminated Java Modified UTF-8; use the `mutf8` APIs and use
   UTF-16 when unpaired Java surrogates must be preserved.
2. Treat pointer lifetimes as scoped to the callback unless documented otherwise.
3. Validate lengths before copying into Rust buffers.
4. Prefer owned Rust structures over returning raw JVMTI structs.
5. Check runtime feature support before touching an appended JNI tail, a reclaimed JVM TI slot, or a newly consumed capability bit.
6. Treat null allocation objects as valid for JDK 28 value-object events.
7. Keep JNI array-element and critical-region leases in their owning guards.
   `abort()` cannot undo writes to directly pinned Java storage.
8. Do not block or invoke arbitrary JNI functions while a critical-region
   guard is live.
9. Prefer `LocalFrame` and `JavaMonitorGuard`; manually paired local frames and
   monitor entry are unsafe `*_raw` operations.
10. Prefer `&CStr` APIs for fixed JNI names in hot paths; construct or validate
   the C string outside the callback loop.
11. For dynamic symbols, verify platform loader success before converting an
   address to a typed function pointer. The caller remains responsible for the
   symbol's exact ABI and signature.
12. Assume `Agent_OnAttach` can be called repeatedly and concurrently. Do not
   reconstruct process-global state or treat a second attach as impossible.
13. Prefer owning `RawMonitor`/`RawMonitorGuard` values. If explicit `close` or
    `exit` fails, ownership remains live and drop makes one best-effort retry.
14. Treat `audits/unsafe-surface-3.0.txt` as a review tripwire, not a proof of
    soundness. Any change requires the independent checklist in
    `UNSAFE_FFI_REVIEW.md` before its baseline is regenerated.
15. Call `Jvmti::dispose_environment` only after disabling and draining every
    callback for that environment. The JVM may let already-running callbacks
    continue, and no environment-derived pointer or wrapper may be used after
    disposal.

## Panic Strategy

The exported lifecycle and callback trampolines use `catch_unwind` as defense
in depth. That contains only unwinding panics. With `panic = "abort"`, a panic
terminates the process before the library can intercept it. No callback should
use panic as normal error handling under either strategy; return a status from
lifecycle methods and record or defer errors from void event callbacks.

## Embedded JVM Lifetimes

Prefer `attach_current_thread_guard` or the scoped
`with_attached_current_thread` helpers. The manual `get_env`, attach, and detach
methods are unsafe because Rust cannot otherwise prevent a `JniEnv` from
outliving the VM, crossing threads, or being used after detachment.
