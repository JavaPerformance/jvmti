# Changelog

## 3.0.0

### Breaking changes
1. Replaced reduced callbacks and parallel `*_with_jvmti` callbacks with one canonical callback per event: `CallbackContext` plus a complete typed event payload.
2. Replaced raw lifecycle arguments with `AgentLoadContext` and `AgentUnloadContext`; option bytes are preserved exactly and UTF-8 decoding is explicit.
3. Changed `Jvmti::new` to accept a trusted callback-scoped `JavaVmRef`; arbitrary raw VM construction is now explicitly unsafe.
4. Changed `Jvmti::allocate` to return `JvmtiAllocation`; raw allocation/deallocation is explicitly unsafe.
5. Changed `Jvmti::dispose_environment` to consume the wrapper and `get_jni_function_table` to return an owned allocation.
6. Replaced safe `LocalRef::new` adoption of arbitrary raw references with unsafe `LocalRef::from_raw`, and made `GlobalRef::new` unsafe because it consumes a caller-supplied local reference.
7. Marked JNI and JVM TI wrapper operations that depend on caller-supplied raw-handle invariants as `unsafe`. The exhaustive method inventory is in `docs/MIGRATING_2_TO_3.md`.
8. Corrected public raw `sys::jvmti` declarations whose 2.x representation or ABI was wrong, including the open `jvmtiError` domain, modern and legacy heap callbacks, timer and stack layouts, extension callbacks, `jvmtiStartFunction`, variadic event notification, virtual-thread exception lists, and JNI table indirection.
9. Changed `suspend_all_virtual_threads` and `resume_all_virtual_threads` to accept an exception-thread slice and corrected `run_agent_thread` to use the three-argument native callback.
10. Changed `set_global_agent` to return the typed `GlobalAgentAlreadySet` error instead of `()`.
11. Renamed raw `jvmtiExtensionParamInfo` to the header-defined `jvmtiParamInfo`, replaced `jvmtiObjectCallback` with the corrected legacy `jvmtiHeapObjectCallback`, and replaced the incorrect object-reference metadata family with `jvmtiHeapReferenceInfo`.
12. Changed `ExtensionFunctionInfo::func` from an untyped pointer to `Option<jvmtiExtensionFunction>` so vendor-defined variadic calls cannot be mistaken for an ordinary safe function.
13. Raised the minimum supported Rust version to 1.85 and adopted Edition 2024.

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

### Fixed
1. Corrected raw timer, stack, heap-reference, extension, callback, JNI indirection, and function-table declarations to match upstream headers.
2. Corrected versioned table access and capability-bit use so older JVMs are rejected before newer slots or bits are touched.
3. Corrected signed native count handling and JVM TI allocation ownership.
4. Removed optional archive, embedding-loader, and benchmark dependencies entirely; all features and development targets now use zero third-party crates.
5. Rejected trailing classfile bytes and added truncation coverage for every input boundary.
6. Kept embedded-JVM invocation option storage alive through VM destruction,
   preventing use-after-free during deferred JVM startup work.

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
