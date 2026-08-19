# JVMTI 3.0.0 ABI and Callback Fidelity Roadmap

Status: implementation complete on local `feature/jvmti-3.0-rust-1.85`; final release-candidate validation remains
Audit date: 2026-08-17; implementation status updated 2026-08-18
Baseline: `main` at `24ed8db4c23354484548cd37edb9e4dc9ff9e0b6`, crate version 2.3.0

## Executive decision

Version 3.0.0 is a deliberate breaking cleanup of the raw ABI, callback, and
ownership surfaces before the crate adds more convenience APIs. The audit that
followed the 2.3.0 JIT callback fix found that
the dropped `jvmtiEnv*` was one instance of a broader problem: the manually
maintained raw bindings have no systematic conformance check against `jvmti.h`.

Several discrepancies are not merely ergonomic. They can make the JVM write
past a Rust allocation, make Rust interpret a C value with the wrong layout, or
call a function through an incompatible signature. Those items are release
blockers for 3.0.0.

The callback migration is intentionally not an additive 2.x compatibility
layer. The canonical `Agent` callback for each event will expose the
callback-scoped JVMTI environment, any available JNI environment, and the full
event payload under the existing unsuffixed callback name. The transitional
`*_with_jvmti` methods and the old reduced-parameter callbacks will be removed.
Because this breaks existing `Agent` implementations, the planned 2.4.0 work is
renumbered as 3.0.0 rather than weakening the project's semantic-versioning
policy.

The forward-looking design constraints are documented in
`docs/FORWARD_COMPATIBILITY_JAVA_26_TO_29.md`. In particular, current JDK 28
work appends JNI `HasIdentity`, adds `JNI_VERSION_28`, and introduces JVM TI
value-object semantics. Version 3.0.0 must absorb those additions and leave an
extension model that can accept comparable future changes in minor releases.
JDK 29 has no published source line yet, so it is covered by an explicit
unknown-next-release boundary and a mandatory source/conformance refresh gate.

The audit also established a useful positive fact: the JVM TI function-table
order remains append/reclaimed-reserved-slot compatible throughout the pinned
JDK 8-28 matrix. JDK 8-10 expose a 155-slot prefix and JDK 11-28 expose a
156-slot prefix. The problem was field type, dependent-structure fidelity, and
safe access to runtime-sized prefixes, not wholesale vtable reordering.

An automated arity comparison of all callable vtable fields found exactly
three arity discrepancies: `SetEventNotificationMode`,
`SuspendAllVirtualThreads`, and `ResumeAllVirtualThreads`. This narrows the
signature repair surface, but does not eliminate type, pointer-indirection, or
dependent-callback mismatches described below.

## Audit method and scope

The review compared:

1. `src/sys/jvmti.rs` against the installed JDK 25 `jvmti.h` and a fresh
   `bindgen 0.72.1` rendering.
2. Every public `Agent` event method against the corresponding raw callback and
   trampoline in `src/lib.rs`.
3. Every wrapper that consumes an affected structure or function pointer in
   `src/jvmti_wrapper.rs`.
4. Ownership rules and callback restrictions against the official Oracle JVMTI
   specification.
5. FFI validity and unwinding assumptions against the Rust Reference and Rust
   Nomicon.
6. JDK 26, JDK 27, current OpenJDK main-line, Java SE 28, Valhalla, native
   access, dynamic-agent, AOT, FFM, and JNA evolution relevant to the next
   several JDK releases.

The original findings came from a static audit. The local implementation now
adds C-header ABI probes, exhaustive callback sentinels, ownership tests,
runtime feature-gate tests, lifecycle FFI tests, and an external-consumer gate.
Publication remains a separate decision after final live-JVM validation.

## Forward-compatibility boundary

Version 3.0.0 is intended to be the last foreseeable major API reset. That goal
is supported by these mandatory design properties:

- callback contexts have private fields, accessors, callback-bound lifetimes,
  and `#[non_exhaustive]` evolution space;
- event payload structures are `#[non_exhaustive]`, crate-constructed, and
  complete for the runtime contract;
- open C integer domains use transparent newtypes and preserve unknown values;
- capabilities use an opaque/open bitset with named accessors;
- high-level enums that may grow are `#[non_exhaustive]`;
- new table-tail functions are checked against the negotiated runtime version
  before access; and
- preview behavior is explicitly runtime-gated and may evolve without changing
  the stable core callback shape.

This is not an unconditional promise never to publish another major version.
An incompatible upstream ABI change or a newly discovered soundness defect may
still require one. It is a commitment not to spend another major version on
predictable additive JDK evolution.

## Release-blocking findings

Unless a subsection explicitly states otherwise, the findings below describe
the audited 2.3 baseline that motivated the 3.0 implementation; the current
validation state is recorded near the end of this document.

### P0-1: `jvmtiError` cannot safely represent JVM results

`src/sys/jvmti.rs:86` defines `jvmtiError` as a Rust enum with only 9 values.
The JDK 25 header defines 53 concrete error values from
`JVMTI_ERROR_NONE` through `JVMTI_ERROR_INVALID_ENVIRONMENT`.

Every JVMTI function returns this type, and thread-list functions can write it
into caller-provided arrays. If the JVM returns one of the 44 omitted values,
Rust observes an invalid enum discriminant. Producing an enum with an invalid
discriminant is undefined behavior, including when the value originated in
foreign code.

Required 3.0.0 change:

- Replace the closed Rust enum with a `#[repr(transparent)]` numeric newtype.
- Preserve familiar names as associated constants, for example
  `jvmtiError::NONE` and `jvmtiError::WRONG_PHASE`.
- Make `error_name` exhaustive over known constants and return a stable
  `JVMTI_ERROR_UNKNOWN` representation for unknown future values.
- Add `raw()` and `from_raw()` accessors so extension-defined or future values
  round-trip without transmutation.

This breaks exhaustive enum matches. It is included in the explicit 3.0.0
migration rather than treated as an exceptional 2.x compatibility break.

### P0-2: modern heap callback bindings have the wrong ABI

`src/sys/jvmti.rs:189-266` mixes deprecated heap-iteration callbacks with the
modern `FollowReferences` / `IterateThroughHeap` callback table.

Confirmed discrepancies:

- Rust `jvmtiHeapCallbacks` has 4 fields; the current JVMTI structure has 16.
- `jvmtiHeapIterationCallback` omits the `length` parameter.
- The modern `jvmtiHeapReferenceCallback` and its reference-information union
  are absent.
- Primitive-field, primitive-array, and string-value callbacks are absent.
- The legacy heap-root, stack-reference, and object-reference callback
  signatures have wrong arity, ordering, or semantics.
- `jvmtiObjectReferenceInfo` is not the modern JVMTI
  `jvmtiHeapReferenceInfo` union.

`src/advanced/heap_graph.rs:65-108` constructs the four-field structure and
passes it to `FollowReferences`. This feature must be considered unsafe to use
until the raw binding and helper are replaced together.

Required 3.0.0 change:

- Reproduce all modern heap callback types, information structures, union
  members, and reserved slots exactly.
- Keep deprecated iteration APIs in a separately named legacy section with
  their exact legacy callback signatures.
- Rebuild `advanced::heap_graph` on the modern table and document callback-safe
  operations and callback-scoped pointer lifetimes.
- Add live `FollowReferences` and `IterateThroughHeap` smoke tests under the
  `heap-graph` feature.

### P0-3: `jvmtiTimerInfo` is undersized

`src/sys/jvmti.rs:268-275` omits `reserved1` and `reserved2`, both `jlong`.
The timer wrappers at `src/jvmti_wrapper.rs:2060-2106` allocate the smaller Rust
structure and ask the JVM to populate the full C structure. On a normal 64-bit
ABI this permits a 16-byte overwrite beyond the Rust value.

Required 3.0.0 change:

- Add both reserved fields in header order.
- Zero-initialize the complete structure.
- Assert size, alignment, and every field offset against a compiled C probe.

### P0-4: `jvmtiStackInfo` layout and deallocation are wrong

`src/sys/jvmti.rs:317-324` orders fields as
`frame_buffer, thread, state, frame_count`; JVMTI orders them as
`thread, state, frame_buffer, frame_count`. The wrappers therefore read the
wrong fields after `GetAllStackTraces` and `GetThreadListStackTraces`.

There is a second independent ownership defect at
`src/jvmti_wrapper.rs:1059-1061` and `1094-1096`: the wrapper may separately
deallocate embedded frame buffers. JVMTI specifies that the returned top-level
allocation includes those frame buffers and that they must not be separately
deallocated.

Required 3.0.0 change:

- Correct the raw field order.
- Copy owned Rust results while the JVMTI allocation is live.
- Deallocate exactly the top-level `jvmtiStackInfo*` once.
- Remove the pointer-range ownership heuristic.
- Reject negative frame counts before allocating or calling the JVM.
- Add live multi-thread stack tests under ASan where supported.

## Other confirmed ABI findings

### P1-1: global virtual-thread suspend/resume omit arguments

`JvmtiSuspendAllVirtualThreadsFn` and `JvmtiResumeAllVirtualThreadsFn` at
`src/sys/jvmti.rs:716-717` accept only `jvmtiEnv*`. Since Java 21 the official
functions also require `except_count` and `except_list`. The wrappers at
`src/jvmti_wrapper.rs:1781-1796` therefore call through incompatible function
pointer types.

3.0.0 should expose slice-based wrappers and pass an empty slice for the
convenience form that suspends or resumes every virtual thread.

### P1-2: JNI function-table pointers have one indirection too many

The raw types at `src/sys/jvmti.rs:718-719` are expressed in terms of `JNIEnv`.
In this crate, `JNIEnv` is already a pointer to `JNINativeInterface_`, so these
signatures introduce an extra pointer level. JVMTI expects
`const jniNativeInterface*` and `jniNativeInterface**`.

3.0.0 should use `JNINativeInterface_` directly in the raw and wrapper APIs and
add a compile-time C signature probe.

### P1-3: `jvmtiStartFunction` omits `JNIEnv*`

`src/sys/jvmti.rs:174` models an agent-thread entry point as
`(jvmtiEnv*, void*)`; JVMTI defines `(jvmtiEnv*, JNIEnv*, void*)`.
`run_agent_thread` consequently accepts callbacks with the wrong signature.

The 3.0.0 raw alias must be corrected. A helper may wrap all three callback
arguments in a callback-scoped context, but it must not hide the JNI argument.

### P1-4: extension event callbacks are modeled as zero-argument functions

`jvmtiExtensionEventCallback` at `src/sys/jvmti.rs:308` is `fn()`. JVMTI
extension events are variadic and always begin with `jvmtiEnv*`. Invoking a
zero-argument Rust callback through the foreign variadic signature is not a
sound generic interface.

Rust cannot provide one safe, statically typed callback for arbitrary
vendor-defined variadic payloads. The 3.0.0 API should therefore:

- deprecate the current callback registration surface;
- expose a clearly unsafe raw registration primitive that preserves the opaque
  function-pointer value; and
- add typed adapters only for extension IDs whose complete signatures are
  known and tested.

The related `jvmtiExtensionFunctionInfo.func` should likewise preserve the
actual opaque callable identity rather than imply that `*mut c_void` is safely
callable.

### P1-5: `SetEventNotificationMode` loses the variadic tail

`JvmtiSetEventNotificationModeFn` at `src/sys/jvmti.rs:603` has four fixed
arguments. The JVMTI function-table declaration is variadic after
`event_thread`. Calling the standard four-argument form happens to use the same
register-level convention on common current targets, but the function pointer
type is not an exact declaration and cannot express extension arguments.

3.0.0 should preserve the exact raw variadic signature. The safe wrapper can
continue exposing the normal fixed-argument operation and should not expose
untyped variadic arguments as safe Rust.

### P1-6: capability storage assumes a target bitfield layout

`jvmtiCapabilities` is represented as `[u32; 4]` with numeric bit operations.
The current 45 named bit offsets exactly match the JDK 25 declaration on the
audited little-endian Linux target, and the overall size/alignment are expected
to match there. C bitfield allocation order is target/compiler-specific,
however, so this representation is not yet proved portable to every platform
the crate supports.

The conformance probe must validate capability size, alignment, and the byte
produced by setting each individual C field on every release target. If the
encoding differs, use a target-aware byte representation rather than assuming
little-endian `u32` bit numbering.

### P1-7: safe APIs accept or retain unchecked raw ownership

The wrapper's safety boundary needs a focused correction:

- `Jvmti::new` is safe but dereferences a caller-provided `JavaVM*`. Safe Rust
  can construct an arbitrary non-null raw pointer, so null checking alone does
  not make the dereference sound.
- `Jvmti::deallocate` is safe but accepts any `*mut u8`, allowing safe Rust to
  ask JVMTI to free memory it did not allocate.
- `Jvmti::dispose_environment(&self)` invalidated the JVM TI environment but
  left the same wrapper available for subsequent safe method calls. Consuming
  the wrapper removed that direct reuse, and 3.0.2 additionally made disposal
  unsafe because active callback contexts can outlive the native dispose call.
- `get_jni_function_table` returns an allocated raw pointer without an ownership
  guard, while `set_jni_function_table` mutates a process-critical table from a
  safe method.
- `LocalRef::new` is safe but will pass any supplied `jobject` to
  `DeleteLocalRef` on drop.
- `GlobalRef::new` was safe and accepted an unchecked raw local reference, passing
  it to `NewGlobalRef` before retaining a `JavaVM*` for cleanup. The 3.0 API is
  unsafe and fallible and acquires the VM before creating the reference.

Minimum 3.0.0 correction:

- make construction from a raw `JavaVM*` explicitly unsafe, while keeping
  trusted internal/macro paths ergonomic;
- replace public raw deallocation with an unsafe escape hatch and an owned
  `JvmtiAllocation<T>` guard for normal use;
- make environment disposal consume `self` and prevent post-disposal calls;
- return the JNI function-table allocation through an owning guard and make
  table replacement explicitly unsafe; and
- make local/global raw-reference guard constructors unsafe or only construct
  them from methods that created/validated the reference.

The broader JNI wrapper exposes many safe operations over raw JNI handles. A
3.0.0 safety audit must classify each operation by whether the JVM validates an
invalid handle or assumes its validity. Any method whose safety relies on a
caller-supplied handle invariant must either take a typed/lifetime-bound handle
or be marked unsafe. Version 3.0.0 must complete that migration rather than
retain an unsound 2.x surface behind deprecation.

As an audit signal, strict Clippy on this baseline reports 214
`not_unsafe_ptr_arg_deref` diagnostics across the JNI/JVMTI wrappers. Clippy is
conservative for opaque foreign handles, so this is not a claim that all 214
sites are independently exploitable. It is a concrete inventory that must be
classified; blanket `allow` attributes are not an acceptable resolution.

### P2: raw enum and constant coverage is incomplete

`src/sys/jvmti.rs` describes itself as a complete binding, but the source
currently declares 54 `JVMTI_*` constants while the audited JDK 25 enum region
contains about 216 constants. Apart from the unsafe closed `jvmtiError` enum,
many C enums are represented as untyped `jint`/`u32` values. This is generally
ABI-compatible but leaves callers without standard names and allows unrelated
integer domains to be mixed.

For 3.0.0:

- remove the unsupported “complete” claim until completeness is mechanically
  enforced;
- generate or audit all standard constants and transparent numeric enum
  newtypes;
- include new version constants and JDK-version annotations; and
- make CI compare exported standard names and values to each supported header.

Unknown future values must remain representable; do not replace open C integer
domains with closed Rust enums.

## Callback fidelity findings

The standard callback table contains 34 non-reserved events. Only 11 public
event paths currently expose the originating `jvmtiEnv*`:

- VM init, VM death, and VM start;
- class load, class prepare, and class-file-load hook;
- method entry and method exit;
- compiled-method load and unload; and
- dynamic-code-generated.

The remaining 23 event paths discard `jvmtiEnv*` in their trampolines:

- thread start and end;
- virtual-thread start and end;
- exception and exception catch;
- single step, frame pop, and breakpoint;
- field access and field modification;
- native-method bind and data-dump request;
- monitor wait, waited, contended-enter, and contended-entered;
- resource exhausted;
- garbage-collection start and finish;
- object free, VM object allocation, and sampled object allocation.

Required 3.0.0 change:

- Change every canonical, unsuffixed `Agent` event method to receive a
  callback-scoped context containing the originating JVMTI environment and the
  JNI environment when that callback supplies one.
- Include the complete JVMTI event payload in the canonical method. Prefer
  event-specific payload structures where that prevents long positional
  signatures or preserves room for typed callback-scoped views.
- Remove every `*_with_jvmti` method and remove the old reduced-parameter
  callback signatures. Do not add another delegation layer.
- Route each trampoline directly through the one canonical full-fidelity
  method.
- Add sentinel tests for every raw parameter of every event, not just table
  offsets or the JIT environment pointer.
- Document that callback pointers and JNI object references are generally valid
  only for the callback duration unless JVMTI explicitly states otherwise.
- Document the especially restricted operation set in GC and object-free
  callbacks.

### Canonical 3.0 callback shape

The target shape is one method per event, not parallel legacy and
full-fidelity traits. In outline:

```rust
#[non_exhaustive]
pub struct CallbackContext<'callback> {
    jvmti: JvmtiRef<'callback>,
    jni: Option<JniEnvRef<'callback>>,
}

pub trait Agent {
    fn method_entry(
        &self,
        context: CallbackContext<'_>,
        event: MethodEntryEvent,
    ) {
    }

    fn method_exit(
        &self,
        context: CallbackContext<'_>,
        event: MethodExitEvent,
    ) {
    }
}
```

The exact wrapper names may be refined during implementation, but the contract
is fixed:

- the context lifetime is bounded by the JVM callback invocation;
- `jvmti` always identifies the environment that originated the callback;
- `jni` is present only where JVMTI supplies a valid `JNIEnv*`;
- event payloads contain every standard parameter, including reserved opaque
  values, with borrowed views tied to the callback lifetime; and
- there is no `_with_jvmti` fallback, forwarding chain, or second callback
  surface.

For example, a 2.3 implementation of `method_entry_with_jvmti` migrates by
renaming it to `method_entry` and reading the JVMTI environment from `context`.
An implementation of the old reduced `method_entry` must additionally accept
the context. The migration guide must show both cases for every affected event.

### Method-exit payload loss

Method exit has an additional loss beyond `jvmtiEnv*`. The raw callback includes
`was_popped_by_exception` and `return_value`, but
`trampoline_method_exit` names and discards both values. The return value is
defined on normal exit and is valuable to profilers, tracers, and diagnostics.

The canonical `method_exit` callback must carry both fields. Remove the old
reduced callback, and document that `return_value` must not be read when
`was_popped_by_exception` is true.

`ResourceExhausted` also drops its reserved opaque pointer. The field is
currently reserved for future use, but the full-fidelity callback should carry
it so a future JVM meaning does not require another trait revision.

### Agent unload and option fidelity

Two lower-severity entry-point issues should be included while the callback
surface is being normalized:

- The export macro receives the `JavaVM*` on unload but `Agent::on_unload`
  discards it. Change canonical `on_unload` to receive a callback-scoped VM
  handle; do not add `on_unload_with_vm`.
- Load and attach convert options with `CStr::to_str().unwrap_or("")`, making
  invalid UTF-8 indistinguishable from no options. Make the canonical callbacks
  byte-preserving or `CStr`-based and make any lossy string convenience
  explicit.

## JIT callback completion

The 2.3.0 fix forwards `jvmtiEnv*`, but two useful JIT payloads remain opaque:

1. `CompiledMethodLoad.map` is typed as `*const c_void`; JVMTI defines an array
   of `jvmtiAddrLocationMap { start_address, location }`.
2. `compile_info` is VM-specific, but HotSpot publishes the stable
   `jvmticmlr.h` compiled-method-load-record chain, including inline stack
   information.

For 3.0.0:

- Add the exact raw `jvmtiAddrLocationMap` structure and typed raw callback
  parameter.
- Add a callback-scoped slice/view validated from `map_length`.
- Add an optional HotSpot CMLR parser behind a distinct feature. It must validate
  record kind/version/length and preserve unknown records rather than assuming
  one VM layout.
- Document the 2.3 opaque-pointer-to-typed-view source migration. Do not retain
  an old `Agent` callback signature in the 3.0 trait.
- Add a JIT capability/configuration preset analogous to the existing method,
  class-hook, exception, and heap-sampling helpers.

No typed view may outlive the callback unless it copies the underlying data.

## FFI panic containment

Every `extern "system"` trampoline currently calls user `Agent` code directly.
There is no `catch_unwind` boundary. A panic crossing a non-unwinding FFI
boundary cannot be allowed; on common Rust configurations it aborts the process.

3.0.0 should centralize callback invocation:

- Wrap user callbacks in `catch_unwind(AssertUnwindSafe(...))` when the crate is
  built with unwinding enabled.
- Convert load/attach panics to `JNI_ERR` after recording a minimal diagnostic.
- For void event callbacks, record the failure, optionally disable that event,
  and return without unwinding into the JVM.
- Define safe fallback values for out-parameter callbacks such as class-file
  transformation and native-method binding before invoking user code.
- State explicitly that `panic = "abort"` cannot be contained.

This guard is defense in depth. Agent callbacks should still be documented as
non-panicking and should avoid allocations or locks where the JVMTI callback
contract makes them unsafe.

## Safe-wrapper hardening

The wrapper is described as safe, but many methods unwrap optional vtable slots
and some convert signed JVM counts to `usize` before validation. These are not
all ABI defects, but they weaken the promise of a safe layer.

3.0.0 hardening targets:

- Replace vtable-slot `unwrap()` calls with a stable `NOT_AVAILABLE` or an
  explicit wrapper error.
- Validate all negative counts and lengths before allocation or slice creation.
- Check count multiplication for overflow.
- Treat null pointer plus nonzero count as an error, not an empty result.
- Centralize JVM-owned allocation cleanup in RAII guards so early returns do not
  leak and nested buffers are freed only according to the function's ownership
  contract.
- Separate callback-safe methods from methods forbidden in restricted callbacks
  where practical.

`get_stack_trace` at `src/jvmti_wrapper.rs:1022-1030` is the first concrete
case: a negative `max_frame_count` is cast to `usize` before JVMTI can return
`ILLEGAL_ARGUMENT`, potentially causing a huge allocation or panic.

## Implementation sequence

### Slice 1: conformance harness first

Build the regression detector before repairing the bindings:

1. Compile a small C probe against the selected JDK's `jvmti.h`.
2. Compare `sizeof`, alignment, and every field offset for shared structures.
3. Compile function-pointer assignment probes for every callback and vtable
   slot; incompatible signatures must fail CI.
4. Compare all 156 vtable field positions.
5. Run the harness against every supported feature release from JDK 8 through
   28, using pinned source headers for non-installed releases and version-gated
   assertions for every table, callback, capability, event, and error addition.
6. Add the official JDK 29 source snapshot and probes immediately when OpenJDK
   publishes it; until then, verify that unknown numeric domains and table-tail
   guards behave correctly with synthetic future values.

Do not make checked-in bindgen output the public API. Use it as an independent
oracle or audit artifact; retain deliberate crate naming and ergonomics.

### Slice 2: repair release-blocking raw bindings

Repair `jvmtiError`, modern and legacy heap callbacks, `jvmtiTimerInfo`, and
`jvmtiStackInfo`. Correct virtual-thread, JNI-table, agent-thread, and extension
callback signatures in the same raw-binding pass.

Compile and run the ABI harness before editing wrappers so each raw correction
has isolated evidence.

### Slice 3: repair ownership and wrappers

Fix stack allocation ownership, heap helpers, timer wrappers, virtual-thread
exceptions, JNI-table wrappers, and signed count validation. Add RAII for
JVMTI-owned allocations.

### Slice 4: complete event fidelity

Introduce the callback-scoped context and canonical full-fidelity payload for
all 34 standard events. Rename the existing `*_with_jvmti` behavior into the
unsuffixed methods, fill the 23 events that currently discard `jvmtiEnv*`, and
remove both the suffixed methods and reduced legacy signatures. Include full
method-exit payloads, unload VM access, and byte-preserving load/attach options.
Keep context fields private and mark context and event payload structures
`#[non_exhaustive]`.

### Slice 5: typed JIT views

Add `jvmtiAddrLocationMap`, callback-scoped map views, and the optional HotSpot
CMLR parser. Extend the existing JIT sentinel tests to every parameter and
multiple map entries.

### Slice 6: contain panics and harden the safe layer

Introduce one FFI invocation guard and migrate every exported entry point and
event trampoline to it. Then remove vtable unwraps and audit every signed
length/count conversion.

### Slice 7: documentation and migration

Update the public API report, safety guide, examples, changelog, and migration
notes. Provide a callback-by-callback 2.3-to-3.0 signature table and state that
2.3.x is the final compatibility line for the old callback trait.

## 3.0.0 release gates

The normalized, executable release-candidate checklist is maintained in
`docs/DEFINITIVE_3_0_RELEASE_GATES.md`. The historical findings and gates below
remain as the audit record.

3.0.0 is not ready until all of the following are true:

- The C/Rust ABI conformance suite passes for every supported JDK line.
- No known raw struct, union, callback, or vtable signature differs from the
  corresponding supported header.
- Exported standard constants match the supported headers, and open numeric
  domains safely preserve unknown future values.
- Every capability bit is byte-for-byte compatible with a C-produced value on
  every release target.
- All 34 standard events have full-parameter sentinel tests.
- Every event trampoline calls exactly one canonical unsuffixed `Agent` method,
  and no public `*_with_jvmti` callback remains.
- Compile-fail migration tests prove that callback contexts and borrowed event
  views cannot escape their callback lifetime.
- Heap graph tests execute successfully with realistic object graphs.
- Timer APIs pass under ASan or an equivalent overwrite detector.
- Stack-trace ownership tests pass under ASan/Valgrind without invalid frees.
- Unknown JVMTI error values round-trip safely.
- Panics in load, attach, unload, and representative event callbacks do not
  unwind into the JVM when unwinding is enabled.
- Safe constructors, reference guards, allocation guards, and environment
  disposal cannot be driven into an invalid dereference/free from safe Rust.
- The JDK 21+ virtual-thread APIs are exercised with empty and non-empty
  exception lists.
- JNI table-tail access is runtime-gated, including JDK 28 `HasIdentity`.
- JDK 28 EA value-object allocation tests accept a null object reference while
  preserving the non-null class, and run only with the pinned preview build.
- No JDK 29 compatibility claim is made until a pinned official JDK 29 source
  snapshot passes the same ABI and runtime matrix; the API must nevertheless
  preserve synthetic unknown future values before 3.0.0 ships.
- Unknown future capability bits survive read, copy, add, and relinquish paths.
- `cargo test --all-features`, Clippy with warnings denied, formatting, docs,
  and examples all pass.
- Migration notes identify every source break, including every renamed or
  reshaped callback, and show its replacement.

## Compatibility and migration policy

Version 3.0.0 is an intentional source-breaking release. It may retain familiar
constant and type names where they remain sound, but it must not preserve an
incorrect raw ABI, an unsound ownership contract, a reduced callback payload,
or duplicate callback names merely to compile unchanged 2.3 code.

For callbacks, the 3.0 trait has one canonical unsuffixed method per lifecycle
or event operation. The method receives a callback-scoped context plus the full
event payload. All `*_with_jvmti`, `on_unload_with_vm`, and reduced 2.x callback
forms are removed. Migration is compiler-guided and documented with before and
after examples; it is not implemented through permanent default-method chains.

The 2.3.x line remains available for users who need source compatibility while
migrating. If a short deprecation release is useful, it may document the coming
3.0 change, but 3.0 implementation and tests must not depend on the deprecated
bridge. Never retain a known-wrong ABI for compatibility.

## Pre-implementation baseline validation result

Before the 3.0 implementation, the audited 2.3 baseline had these results:

- `cargo test --all-features` passes all 16 non-document integration tests and
  the two active compile-fail doctests.
- `cargo clippy --all-targets --all-features -- -D warnings` fails. The dominant
  category is 214 raw-pointer safety diagnostics; it also reports nine ordinary
  style/API diagnostics.
- `cargo fmt --check` also fails on the pre-existing Rust source and tests; the
  audit document itself passes `git diff --check`.

The passing tests are not contradictory evidence. Existing ABI tests validate
event IDs and callback-table offsets, but do not compare C/Rust structure
layouts, function signatures, returned enum validity, or live allocation
ownership. That gap is why Slice 1 precedes the fixes.

## Local 3.0 validation result

The current local implementation passes:

- `cargo test --all-targets --all-features`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- doctests and rustdoc with warnings denied;
- `cargo publish --dry-run --allow-dirty` and a separate external-consumer
  release build;
- `cargo +1.85.0 check --all-targets --all-features`, matching the declared
  MSRV and covering every library, tool, test, example, and benchmark target;
- `scripts/check-zero-dependencies.sh`, proving that every feature and
  development target has zero third-party crate dependencies;
- the pinned C/Rust ABI matrix for every feature release from JDK 8 through
  the current JDK 28 source snapshot, including 440 exact table signatures, 31
  native records, and 562 field offsets;
- an executable host `va_list` forwarding proof, target-aware Linux AArch64
  JNI variadic declarations, and endian-aware JVM TI capability bits;
- real callback delivery on seven installed JVMs spanning JDK 8, 11, 17, 21,
  25, and 27;
- live Modified UTF-8, repeated-attach, and heap-graph proofs;
- allocation-free callback dispatch with one and eight Java threads; and
- an external startup-and-attach agent compiled from the packaged crate.
- the 2.x-to-3.0 migration contract, including all 34 callback mappings, the
  complete JNI/JVM TI safe-to-unsafe method inventory, raw-binding breaks, and
  a compile-checked migration fixture with documentation coverage assertions.

Publication remains gated on the specialized checks above that require a
suitable preview runtime or native instrumentation, including the live JDK 28
value-object case and sanitizer-backed ownership/overwrite exercises. JDK 29
cannot be tested until OpenJDK publishes an identifiable JDK 29 source line.

## Primary references

- [Oracle JVMTI 24 specification](https://docs.oracle.com/en/java/javase/24/docs/specs/jvmti.html)
- [Rust Reference: behavior considered undefined](https://doc.rust-lang.org/reference/behavior-considered-undefined.html)
- [Rust Nomicon: FFI and unwinding](https://doc.rust-lang.org/nomicon/ffi.html#ffi-and-unwinding)
- Installed audit header: JDK 25 `include/jvmti.h`
- Installed HotSpot extension header: JDK 25 `include/jvmticmlr.h`
- [Forward-compatibility research](FORWARD_COMPATIBILITY_JAVA_26_TO_29.md)
