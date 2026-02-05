# Changelog

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
