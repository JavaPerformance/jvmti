# Changelog

## 2.2.0

### Added
1. Safer JVM embedding thread helpers:
   - `JavaVm::attach_current_thread_guard`
   - `JavaVm::attach_current_thread_as_daemon`
   - `JavaVm::attach_current_thread_as_daemon_guard`
   - `JavaVm::with_attached_current_thread`
   - `JavaVm::with_attached_current_thread_as_daemon`
   - `AttachedThread`
2. `Default` for `JavaVmBuilder`, using the Java 8 JNI baseline.
3. `jni::result_name` for human-readable JNI status/result codes.
4. API tests for null `JavaVM` handling and embedding helper surface.

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
