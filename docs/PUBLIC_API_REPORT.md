# Public API Report

(Curated public API contract. `scripts/public_api_report.sh` writes a separate
generated symbol inventory under `target/` for comparison.)

This report summarizes the intended public surface of `jvmti-bindings`.

## Top-Level Exports

1. `Agent` trait
2. `export_agent!` macro
3. `get_default_callbacks`
4. `jni` re-export (`crate::sys::jni`)
5. `describe_jni_result`
6. `GlobalAgentAlreadySet` and `set_global_agent`
7. Modules: `agent`, `callbacks`, `env`, `version`, `sys`, `classfile`, `prelude`, `embed` (feature-gated), `advanced` (feature-gated)

## `agent` Module

Callback-scoped lifecycle types:
1. `AgentLoadContext`
2. `AgentUnloadContext`
3. `JavaVmRef`

## `callbacks` Module

1. `CallbackContext`, `JvmtiRef`, and `JniEnvRef`
2. Complete, crate-constructed payloads for all 34 standard JVM TI events
3. Callback-scoped JIT map and compiler-record views
4. Mutable class-file and native-method-bind outputs with checked ownership transfer

## `env` Module

Public types:
1. `Jvmti`
2. `JniEnv`
3. `LocalRef`
4. `GlobalRef`
5. `JvmtiAllocation`
6. `JniFunctionTable`
7. `JniVersionError`
8. `ThreadInfo`
9. `ThreadGroupInfo`
10. `MonitorUsage`
11. `StackInfo`
12. `ExtensionParamInfo`
13. `ExtensionFunctionInfo`
14. `ExtensionEventInfo`
15. `LocalVariableEntry`

Common `Jvmti` helper methods:
1. `set_default_agent_callbacks`
2. `add_class_file_load_hook_capabilities`
3. `add_method_trace_capabilities`
4. `add_exception_capabilities`
5. `add_heap_sampling_capabilities`
6. `enable_class_file_load_hook_events`
7. `enable_method_entry_exit_events`
8. `enable_exception_events`
9. `enable_heap_sampling_events`
10. `enable_vm_lifecycle_events`
11. `configure_class_file_load_hook_agent`
12. `configure_method_trace_agent`
13. `configure_exception_agent`
14. `configure_heap_sampling_agent`
15. `get_error_name_string`

## `sys` Module

1. `sys::jni` - Raw JNI types, constants, and function tables.
2. `sys::jvmti` - Raw JVMTI types, constants, and function tables.
3. Diagnostics: `jni::result_name`, `jni::describe_result`, `jvmti::error_name`.
4. Capability presets: `jvmtiCapabilities::for_class_file_load_hook`, `for_method_trace`,
   `for_exceptions`, `for_heap_sampling`.

Note: `sys` mirrors JNI/JVMTI headers and may grow with new JDK versions.

## `version` Module

1. `RELEASE_PROFILES`, `ReleaseProfile`, and `release_profile`
2. `ReleaseDelta` and `release_delta`
3. `JniFeature`, `JvmtiFeature`, and `FeatureMaturity`
4. `JvmtiSemanticChange`, `JvmtiErrorAddition`, `NativeSourceChange`, and `NativePolicyChange`
5. `RuntimeChange`, `RuntimeSupport`, and interface-version helpers

## `classfile` Module

1. `ClassFile` and supporting structs/enums for all standard Java 8-27 attributes.
2. `ClassFile::parse(bytes)` entry point.

## `prelude` Module

Recommended imports for agent authors:
1. `Agent`, `export_agent!`, `get_default_callbacks`
2. `agent` lifecycle contexts and complete `callbacks` payloads
3. `env::{Jvmti, JniEnv, LocalRef, GlobalRef, JvmtiAllocation, JniFunctionTable}`
4. `version` release profiles, deltas, feature gates, and compatibility metadata
5. `sys::{jni, jvmti}`
6. `embed::{JavaVmBuilder, JavaVm, AttachedThread}` when the `embed` feature is enabled

## `embed` Module

Feature-gated JVM embedding helpers (`embed` feature):
1. `JavaVmBuilder`
2. `JavaVm`
3. `AttachedThread`
4. `find_libjvm`
5. `find_libjvm_verbose`
6. `EmbedError`

## `advanced` Module

Feature-gated helpers (disabled by default):
1. `advanced::heap_graph` (`heap-graph` feature)

## Stability Notes

1. `env` and top-level exports are intended to be stable.
2. `sys` is a low-level mirror of JNI/JVMTI C headers.
3. `advanced` is explicitly allowed to change more rapidly.
4. The complete 2.x-to-3.0 source migration is documented in
   [Migrating From 2.x to 3.0](MIGRATING_2_TO_3.md).
