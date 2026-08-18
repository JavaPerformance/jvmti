# Allocation and Ownership Audit - 2026-08-17

## Conclusion

Normal Rust callback dispatch is allocation-free. A real JDK callback agent
observed no Rust allocator activity over both a single-thread run of 22,301,473
method-entry callbacks and an eight-thread run of 162,344,903 callbacks.

This is deliberately narrower than saying that every operation exposed by the
crate is allocation-free. Metadata queries return owned `String` and `Vec`
values, Java object creation can allocate in the JVM, class transformation must
publish JVM TI-owned bytes, and user callback implementations may allocate.

## Callback Path

The normal event path performs:

1. a `OnceLock` lookup of the process agent;
2. stack-only `CallbackContext` and typed event construction;
3. a non-panicking `catch_unwind` boundary;
4. one trait method dispatch.

The agent itself is boxed once during the first `Agent_OnLoad` or
`Agent_OnAttach`. Repeated attach requests reuse that same process-global
instance; they do not allocate another agent. The one-time allocation is
intentionally outside callback tracking. Borrowed class
bytes, JIT maps, names, descriptions, and callback payloads are represented as
slices or references over JVM-owned memory and are not copied by dispatch.

`ClassFileLoadHookEvent::set_transformed_class` is the intentional exception:
the JVM TI contract requires a new buffer allocated by the active JVM TI
environment. The helper allocates once, copies the transformed bytes, and
explicitly transfers ownership to the JVM.
The event enforces single assignment, so a second setter call cannot overwrite
and strand the first outbound allocation.

The panic path is not promised to be allocation-free. Constructing a panic
payload or an application's `callback_panicked` implementation may allocate;
normal non-panicking dispatch does not.

## Live Evidence

Command:

```bash
scripts/prove-callback-allocation-free.sh
```

Single-thread result:

```text
callbacks=22301473 allocations=0 allocated_bytes=0 reallocations=0 deallocations=0
```

Eight-thread result:

```text
callbacks=162344903 allocations=0 allocated_bytes=0 reallocations=0 deallocations=0
```

The proof uses a standard-library `GlobalAlloc` wrapper around `System`. It
therefore detects Rust heap traffic from the agent but does not attribute JVM
internal native allocation or Java heap allocation.

## Sanitizer Evidence

The complete all-feature test suite passed AddressSanitizer with leak detection
on 2026-08-18. The first sanitizer run found one 122-byte leak in the fake
embedded-VM test harness: the success-path test deliberately forgot its mock VM
table because the fake `DestroyJavaVM` could not succeed. The harness now gives
the fake VM a valid invocation table, destroys it normally, and reclaims the
mock allocations. The subsequent complete run was clean. This was a test
resource leak, not a production callback or embedded-VM leak.

Production still deliberately retains the support allocation when a real
`DestroyJavaVM` fails. Freeing invocation options or unloading `libjvm` while
that VM remains live would turn an observable bounded retention into a possible
use-after-free. Scoped Miri proofs also pass for Modified UTF-8 and the JNI/JVM
TI ownership guards; Miri and AddressSanitizer complement rather than replace
the live cross-JDK tests.

## JNI Allocation Surface

Fixed class names, method names, field names, signatures, and exception text
have `&CStr` entry points so static `c"..."` literals do not create temporary
`CString`s. The corresponding `&str` helpers intentionally allocate when they
must validate and NUL-terminate dynamic text.

`&str` inputs are encoded as Java Modified UTF-8, not ordinary UTF-8. Static or
pre-encoded `&CStr` inputs avoid that conversion but must already satisfy the
native Modified UTF-8 contract. Strict native-output decoding distinguishes
malformed bytes from null/absent values; exact UTF-16 conversion preserves
unpaired Java surrogates where the API permits them.

`new_string(&str)` must encode to UTF-16 and therefore creates a temporary
buffer. `new_string_utf16(&[jchar])` accepts pre-encoded storage and performs no
Rust allocation. `get_string` and `get_string_utf` return owned Rust strings and
necessarily allocate their result; both release JVM character leases on every
normal return path.

Array region helpers use caller-owned buffers. JNI operations that return
borrowed native storage use allocation-free owning guards:

- `PrimitiveArrayElements<T>` pairs every successful
  `Get<Type>ArrayElements` with the exact matching release function;
- `PrimitiveArrayCritical` pairs `GetPrimitiveArrayCritical` with release;
- `StringCritical` pairs `GetStringCritical` with release.

Each guard releases exactly once on drop or explicit close. `JNI_COMMIT` retains
the primitive-array lease and normal drop still performs the final release.
`JNI_ABORT` requests that copied storage not be written back; it cannot undo
writes when the JVM returned pinned backing storage. Critical guards document
the JNI no-blocking/no-arbitrary-JNI restriction. The raw `sys` table remains
the deliberate escape hatch for manually managed leases.

`LocalFrame` pairs a successful `PushLocalFrame` with `PopLocalFrame` on drop
and permits one explicit promoted result. `JavaMonitorGuard` pairs a successful
JNI `MonitorEnter` with `MonitorExit`; a failed explicit exit remains active so
drop makes one best-effort retry. The unmatched variants are named `*_raw` and
are unsafe.

JNI local references consume the JVM's local-reference table rather than the
Rust heap. JNI methods and JVM TI methods returning `jobject`, `jclass`,
`jthread`, or `jthreadGroup` expose local references; this is documented on the
`Jvmti` wrapper and on result structures carrying nested handles. `LocalRef`
and local frames should be used in loops so the table cannot grow until native
method return. This matters especially on agent threads that may never return
from native code.

## JNI Ownership Corrections

`GlobalRef::new` now obtains the owning `JavaVM*` before calling
`NewGlobalRef`. Previously, a failed `GetJavaVM` after successful reference
creation left `Drop` without a VM through which to release the reference.
Construction is now fallible and cannot create that stranded reference.

`WeakGlobalRef` adds the missing owning guard for `NewWeakGlobalRef`.
`GlobalRef::close` and `WeakGlobalRef::close` report cleanup failures when an
explicit lifecycle boundary requires stronger evidence than best-effort
destruction. Raw create/delete methods remain available for deliberate manual
ownership.

`Jvmti::create_raw_monitor` returns an owning `RawMonitor`; entering it returns
an owning `RawMonitorGuard`. Explicit `close`/`exit` reports the JVM TI result.
If explicit release fails, the handle remains owned so drop can make one
best-effort retry rather than silently forgetting a live monitor or lock.

## JVM TI Ownership Corrections

Every top-level JVM TI output allocation, and every independently allocated
sibling output exposed by a successful call, is now adopted by a scope guard
before conversion to Rust-owned values. Early conversion errors and unwinding
therefore invoke the matching JVM TI `Deallocate` operation. Nested structures
follow the allocation boundaries specified for their individual JVM TI call;
for example, stack-frame buffers are embedded in one top-level stack allocation,
whereas local-variable strings and extension metadata are independent owners.

Nested stack, local-variable, extension, parameter, property, and monitor
arrays are processed directly while their native owner is alive. This removes
redundant temporary vectors for structure arrays while preserving owned output
strings and vectors returned by the public API.

Explicit ownership escapes remain limited and documented:

- `JvmtiAllocation::into_raw` transfers responsibility to the caller;
- class transformation transfers its allocation to the JVM;
- `LocalRef::into_inner` transfers local-reference deletion responsibility;
- `JniFunctionTable::into_raw` transfers table cleanup responsibility.

## Residual Rules

1. A user callback can allocate, lock, block, perform I/O, or call allocating
   metadata APIs; the dispatch proof does not make that handler free.
2. Owned `String` and `Vec` query results intentionally allocate once for the
   returned Rust value.
3. Exact JVM TI events can allocate or synchronize inside HotSpot; that cost is
   outside the Rust allocator and must be measured with the live benchmark.
4. Raw ownership escape hatches can leak if their documented cleanup contract
   is ignored.
5. `Drop` cannot return a JNI error; call `close()` when cleanup success must be
   observed.
6. The process-global `Agent` is boxed once and intentionally lives until JVM
   shutdown. Repeated attach reuses it. This is bounded lifecycle state, not
   per-event or per-attach growth.

## Validation

- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`
- `cargo test --doc --all-features`
- `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features`
- `scripts/check-zero-dependencies.sh`
- `scripts/check-agent-template.sh`
- `scripts/check-wrapper-coverage.sh`
- `scripts/check-wrapper-forwarding.py` across 237 direct methods, 99 helper
  methods, 18 transformed/delegated contracts, and 13 JNI macro families
- `scripts/check-public-api-extensibility.py`
- `scripts/check-classfile-corpus.sh` across installed JDK 8, 11, 17, 21, 25,
  and 27 runtimes (159,591 classes, zero failures on 2026-08-18)
- single-thread and eight-thread runs of
  `scripts/prove-callback-allocation-free.sh`
- `scripts/prove-repeated-attach-live.sh`
- `scripts/prove-attach-policy-live.sh` on JDK 21, 25, and 27
- `scripts/prove-mutf8-live.sh`
- `cargo test --test raw_monitor_ownership`
- `cargo test --test jni_ownership`

The host's Valgrind 3.27.1 cannot execute even `/bin/true`; it terminates in the
dynamic loader with `SIGILL`. No Valgrind result is claimed. Exact deallocation
tests instead use mock JNI/JVM TI allocators and reference tables with counted
create/delete operations.

## Specification Anchors

- [JVM Tool Interface 25](https://docs.oracle.com/en/java/javase/25/docs/specs/jvmti.html)
  defines which successful outputs require `Deallocate`, which nested buffers
  are independent, and which stack buffers share a top-level allocation.
- [JNI Functions 25](https://docs.oracle.com/en/java/javase/25/docs/specs/jni/functions.html)
  defines local, global, and weak-global reference lifecycles.
