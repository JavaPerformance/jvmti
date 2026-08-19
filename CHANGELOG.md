# Changelog

## 3.0.2

Safety patch following independent post-publication FFI review.

1. `NativeMethodBind` now preserves the VM-selected implementation address for
   absent and default no-op agents. If a handler changes the address and then
   panics, the trampoline restores the original address before returning to the
   VM. Regression tests cover absent, no-op, redirecting, and panicking paths.
2. Heap tagging now uses checked non-zero tag progression. Exhausted ranges
   abort traversal and return `JVMTI_ERROR_ILLEGAL_ARGUMENT` rather than
   panicking or wrapping inside an FFI callback. Heap-edge storage also uses
   fallible reservation and returns `JVMTI_ERROR_OUT_OF_MEMORY` on capacity
   failure.
3. Adds the cross-platform `minecraft_bullet_time` example. It observes
   configurable Java-side keyboard and mouse-scroll callbacks, requires F8 to
   be held, consumes the armed scroll delta before normal hotbar handling, and
   adjusts a bounded delay at a configurable tick-method breakpoint without
   platform-specific input APIs.

There is no public library API or ABI change, no new dependency, and no change
to the Rust 1.85 minimum.

## 3.0.1

Documentation and examples update. The example suite grows from 13 to 35
programs, covering lifecycle, diagnostics, watchpoints, breakpoints, JNI,
class-file parsing, JIT events, and standalone Minecraft-oriented templates.
Current documentation also uses attribution-neutral wording. There are no
library API, ABI, runtime-behavior, safety-contract, dependency, or
minimum-toolchain changes from 3.0.0.

The packaged-consumer release gate now refreshes its staged path-override lock
before enforcing a locked build, preventing a previous patch version in the
template lock from producing a false release failure. CI also uses a SemVer
checker release that understands the current stable rustdoc format.

## 3.0.0

### Breaking changes
1. Replaced reduced callbacks and parallel `*_with_jvmti` callbacks with one canonical callback per event: `CallbackContext` plus a complete typed event payload.
2. Replaced raw lifecycle arguments with `AgentLoadContext` and `AgentUnloadContext`; option bytes are preserved exactly and Java Modified UTF-8 decoding is explicit.
3. Changed `Jvmti::new` to accept a trusted callback-scoped `JavaVmRef`; arbitrary raw VM construction is now explicitly unsafe.
4. Changed `Jvmti::allocate` to return `JvmtiAllocation`; raw allocation/deallocation is explicitly unsafe.
5. Changed `Jvmti::dispose_environment` to consume the wrapper and `get_jni_function_table` to return an owned allocation.
6. Replaced safe `LocalRef::new` adoption of arbitrary raw references with
   unsafe `LocalRef::from_raw`, and made `GlobalRef::new` unsafe and fallible
   because it accepts a caller-supplied raw local handle and must acquire the
   owning VM before creating the separate global reference.
7. Marked JNI and JVM TI wrapper operations that depend on caller-supplied raw-handle invariants as `unsafe`. The exhaustive method inventory is in `docs/MIGRATING_2_TO_3.md`.
8. Corrected public raw `sys::jvmti` declarations whose 2.x representation or ABI was wrong, including the open `jvmtiError` domain, modern and legacy heap callbacks, timer and stack layouts, extension callbacks, `jvmtiStartFunction`, variadic event notification, virtual-thread exception lists, and JNI table indirection.
9. Changed `suspend_all_virtual_threads` and `resume_all_virtual_threads` to accept an exception-thread slice and corrected `run_agent_thread` to use the three-argument native callback.
10. Changed `set_global_agent` to return the typed `GlobalAgentAlreadySet` error instead of `()`.
11. Renamed raw `jvmtiExtensionParamInfo` to the header-defined `jvmtiParamInfo`, replaced `jvmtiObjectCallback` with the corrected legacy `jvmtiHeapObjectCallback`, and replaced the incorrect object-reference metadata family with `jvmtiHeapReferenceInfo`.
12. Changed `ExtensionFunctionInfo::func` from an untyped pointer to `Option<jvmtiExtensionFunction>` so vendor-defined variadic calls cannot be mistaken for an ordinary safe function.
13. Raised the minimum supported Rust version to 1.85 and adopted Edition 2024.
14. Changed class-file `CONSTANT_Utf8` values from plain `String` to
    `JavaString`, preserving valid unpaired Java UTF-16 surrogate code units.
15. Changed the open JNI `jobjectRefType` domain from a closed Rust enum to a
    transparent integer newtype that preserves future VM values.
16. Corrected nullable raw JVM TI callback parameters to `Option<fn>` and the
    31 unsuffixed JNI variadic invocation slots from untyped pointers to their
    exact callable C function types.
17. Made manual embedded-VM environment and attach/detach methods unsafe;
    lifetime-bound `AttachedThread` guards and scoped closure helpers remain the
    safe default.
18. Marked crate-produced metadata records and extensible high-level enums
    non-exhaustive so future JDK fields and variants can enter 3.x additively.
19. Replaced the one-size pointer `va_list` placeholder with target-aware ABI
    representations, including by-value Linux AArch64 and ARM forms.

See [Migrating From 2.x to 3.0](docs/MIGRATING_2_TO_3.md) for the callback-by-callback table and complete source migration inventory.

### Added
1. Complete callback payloads for all 34 standard non-reserved JVM TI events, including JIT maps, return values, field values, resource data, and mutable outputs.
2. Runtime-gated JNI additions through the pinned current JDK 28 source snapshot: modules, virtual threads, long modified-UTF length, and preview value-object identity.
3. Runtime-gated JVM TI additions through the pinned current JDK 28 source snapshot: modules, heap sampling, virtual threads, `ClearAllFramePops`, and preview value-object capability semantics.
4. Exact C/Rust ABI probes against every OpenJDK feature release from 8 through current JDK 28 headers.
5. Panic containment at lifecycle and event FFI boundaries.
6. A JDK 29 acceptance gate that requires official source rather than inferring support from JDK 28 main line.
7. Allocation-free `&CStr` variants for class, string, method, field, and exception JNI operations while retaining the existing `&str` convenience adapters.
8. A standard-library-only JVM dynamic loader and dependency-free benchmark harnesses.
9. A CI-enforced zero-third-party-crate contract across normal, optional, build, development, and benchmark targets.
10. A live callback-dispatch benchmark comparing no agent, an idle Rust agent,
    raw-C no-op JVMTI delivery, Rust no-op dispatch, and Rust relaxed-atomic
    counting under the same non-inlined Java workload.
11. An owning `WeakGlobalRef` guard and a live counting-allocator proof for the
    normal callback dispatch path.
12. Scope-guarded cleanup for successful JVM TI output allocations exposed by
    the high-level wrapper, direct processing of nested native arrays, and
    allocation-free `JniEnv::new_string_utf16`.
13. Single-assignment class transformation output, preventing repeated setter
    calls from stranding an earlier JVM TI allocation.
14. A standard-library-only Java Modified UTF-8 codec with exact UTF-16 escape
    hatches for unpaired surrogates.
15. Owning `RawMonitor` and `RawMonitorGuard` types with explicit fallible
    release and best-effort drop cleanup.
16. Compiler-checked conformance for all 440 JNI/JVM TI table-field signatures,
    plus exact layout checks for 31 public native records and 562 fields.
17. Live JVM proofs for Modified UTF-8, repeated dynamic attach, heap traversal,
    callback delivery, and allocation-free callback dispatch.
18. Bounded class-file parsing through `ClassFileParseLimits` and
    `ClassFile::parse_with_limits`, with shared allocation, input-size, and
    recursive-annotation budgets.
19. A host C-to-Rust-to-C `va_list` forwarding proof and Linux AArch64 CI for
    native ABI conformance.
20. Complete high-level coverage for all fixed-signature JNI operations,
    including typed `jvalue` (`A`) invocation families; C variadic and
    `va_list` slots remain deliberate raw-only escape hatches.
21. Allocation-free `PrimitiveArrayElements`, `PrimitiveArrayCritical`, and
    `StringCritical` guards that pair every successful JNI native-storage
    acquisition with exactly one final release.
22. Owning `LocalFrame` and `JavaMonitorGuard` types for automatic
    `PopLocalFrame` and `MonitorExit` on early return or panic.
23. A wrapper-coverage gate that inventories every JNI/JVM TI table and
    callback slot, requiring each to be wrapped or explicitly reviewed as
    reserved or raw-only.
24. Deterministic class-file mutation tests and full installed-runtime corpus
    parsing, in addition to recursive-attribute, recursive-annotation,
    input-size, and shared cumulative-allocation budgets.
25. Fail-closed wrapper-forwarding and public-API-extensibility gates that
    reject dropped/reordered native arguments, exhaustive future-facing data
    types, required future agent hooks, and constructible growing native
    tables.
26. Packaged downstream canaries for both startup/dynamic-attach agents and
    embedded-JVM consumers.
27. A reviewed full-signature 3.0 API baseline, an unsafe-sensitive source
    baseline, and mandatory SemVer comparison tooling for later 3.x releases.
28. Release evidence containing SHA-256 checksums, an SPDX 2.3 SBOM, and
    GitHub build-provenance and SBOM attestations.
29. A private vulnerability-reporting policy and an independent unsafe/FFI
    review packet tied to the exact release-candidate commit.

### Fixed
1. Corrected raw timer, stack, heap-reference, extension, callback, JNI indirection, and function-table declarations to match upstream headers.
2. Corrected versioned table access and capability-bit use so older JVMs are rejected before newer slots or bits are touched.
3. Corrected signed native count handling and JVM TI allocation ownership.
4. Removed optional archive, embedding-loader, and benchmark dependencies entirely; all features and development targets now use zero third-party crates.
5. Rejected trailing classfile bytes and added truncation coverage for every input boundary.
6. Kept embedded-JVM invocation option storage alive through VM destruction,
   preventing use-after-free during deferred JVM startup work.
7. Reused one process-global agent across repeated `Agent_OnAttach` invocations,
   matching the Attach API lifecycle instead of rejecting a second attach.
8. Corrected heap traversal control flow to return the JVM TI visit constants,
   rather than treating ordinary Rust booleans as protocol values.
9. Distinguished null native metadata strings from malformed Modified UTF-8;
   malformed successful output now fails explicitly instead of becoming absent.
10. Retained raw-monitor ownership after failed explicit destroy/exit operations
    so drop can make one best-effort cleanup attempt.
11. Corrected JVMTI capability bit numbering on big-endian targets while
    preserving unknown future bits.
12. Bound safe embedded-thread environments to their VM and attachment guard,
    preventing safe use after detach or VM destruction.
13. Rejected excessive recursive annotation nesting with a typed class-file
    parse error instead of risking native stack exhaustion.
14. Removed ordinary UTF-8 assumptions and panic-prone initialization from the
    shipped startup/dynamic-attach agent template.
15. Preserved the embedded JVM support allocation when `DestroyJavaVM` fails,
    preventing a live VM from retaining callbacks or option pointers into an
    unloaded native library.
16. Charged failed scalar-string conversion, dynamic attribute names, and
    dynamic parser errors to the same cumulative class-file allocation budget.
17. Made OpenJDK ABI source caching commit-aware so changing a pinned revision
    cannot silently reuse stale downloaded headers.
18. Made append-only JNI/JVM TI function and callback tables non-exhaustive at
    the Rust source boundary, allowing future OpenJDK tails to enter 3.x minor
    releases without changing their native C layout or forcing a 4.0.

## 2.3.0

### Added
1. `Agent::compiled_method_load_with_jvmti`, `Agent::compiled_method_unload_with_jvmti`, and `Agent::dynamic_code_generated_with_jvmti`, allowing JIT callbacks to query method metadata through the callback's `jvmtiEnv*` without breaking existing callback implementations.

## 2.2.1

### Added
1. `Agent::data_dump_request`, `Agent::virtual_thread_start`, and `Agent::virtual_thread_end` callbacks.
2. ABI regression tests for every JVMTI event number, reserved callback slot, and JDK-generation callback-table prefix.
3. `scripts/prove-event-callback-matrix.sh`, which loads a real Rust agent and exercises method-entry and post-gap GC callbacks under each installed JDK.

### Fixed
1. Corrected JVMTI event constants from `MethodEntry` onward to match the specification (`65` through `88`).
2. Corrected `jvmtiEventCallbacks` to preserve reserved slots `72`, `77`, `78`, `79`, and `85`.
3. Added the JDK 21 virtual-thread callback tail and a typed `DataDumpRequest` callback, restoring ABI-compatible callback delivery on JDK 8 through 27.

## 2.2.0

### Added
1. Safer JVM embedding thread helpers:
   - `JavaVm::attach_current_thread_guard`
   - `JavaVm::attach_current_thread_as_daemon`
   - `JavaVm::attach_current_thread_as_daemon_guard`
   - `JavaVm::with_attached_current_thread`
   - `JavaVm::with_attached_current_thread_as_daemon`
   - `AttachedThread`
2. Common JVMTI workflow helpers for class-file hooks, method tracing, exception tracing, heap sampling, and default callback wiring.
3. Capability preset builders on `jvmtiCapabilities`.
4. `Default` for `JavaVmBuilder`, using the Java 8 JNI baseline.
5. `jni::result_name`, `jni::describe_result`, `jvmti::error_name`, and top-level `describe_jni_result` diagnostics.
6. `Jvmti::get_error_name_string` for JVM-provided JVMTI error names.
7. API tests for null `JavaVM` handling, diagnostics, workflow helpers, capability presets, and embedding helper surface.

### Fixed
1. `Jvmti::new` now rejects null `JavaVM` pointers instead of dereferencing them.
2. Embedding error messages now include JNI status names.
3. Documentation and crate metadata now say "zero dependencies by default" instead of implying optional features have no dependencies.
4. Versioning and API-stability docs now describe the current 2.x SemVer policy.
5. README no longer implies dynamic attach is unsupported.

## 2.1.0

### Added
1. JVM embedding helpers behind the `embed` feature (`JavaVmBuilder`, `JavaVm`) with `JAVA_HOME`/`JVM_LIB_PATH` discovery.
2. Embedding documentation and runnable example (`docs/EMBEDDING.md`, `examples/embed.rs`).
3. Dynamic attach documentation and example (`docs/ATTACH.md`, `examples/attach_logger.rs`).
4. Benchmark guide plus streaming JAR parser tool (`docs/BENCHMARKS.md`, `jar_parse_bench`).
5. Comparison matrix doc for alternative crates (`docs/COMPARISON.md`).

### Fixed
1. CI example builds (feature-gated embed example and attach logger `on_load` stub).

## 2.0.2

### Fixed
1. Corrected crates.io documentation link to point at `docs.rs/jvmti-bindings`.

## 2.0.1

### Fixed
1. README and documentation alignment with 2.0 behavior (prelude-first, classfile parser, dynamic attach, and safety model).

## 2.0.0

### Breaking changes
1. `jvmti_wrapper` and `jni_wrapper` are now crate-private. Use `env::*` for public wrappers.
2. Several JVMTI wrapper methods now return owned safe structs instead of raw JVMTI structs:
   - `get_thread_info`, `get_thread_group_info`, `get_object_monitor_usage`, `get_all_stack_traces`,
     `get_thread_list_stack_traces`, `get_extension_functions`, `get_extension_events`,
     `get_local_variable_table`.
3. `Agent::field_access` and `Agent::field_modification` now take `jfieldID` (not `jobject`).
4. `JniEnv` and `GlobalRef` are now explicitly `!Send`/`!Sync` to enforce thread-local safety.

### Added
1. Full classfile parser with all Java 8-27 attributes (`classfile` module).
2. `Agent::on_attach` and `Agent_OnAttach` export.
3. `prelude` module for standard agent imports.
4. Safety, pitfalls, compatibility, and API surface documentation.
5. Feature-gated advanced helpers (`advanced`, `heap-graph`).
6. New examples (profiler, tracer, heap sampler) and agent starter template.
7. Cross-platform CI (Linux/macOS/Windows) and benchmark harness.

### Fixed
1. Eliminated several JVMTI use-after-free hazards by deep-copying JVMTI-allocated buffers.
2. Safer string handling in JNI wrappers (UTF-16 helpers).
3. Error handling for invalid CString inputs in JVMTI wrappers.
