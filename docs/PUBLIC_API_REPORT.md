# Public API Report

(Curated public API contract. `scripts/public_api_report.sh` writes a separate
all-feature symbol-name inventory under `target/` for comparison.)

This report summarizes the intended public surface of `jvmti-bindings`.

## Top-Level Exports

1. `Agent` trait
2. `export_agent!` macro
3. `get_default_callbacks`
4. `jni` re-export (`crate::sys::jni`)
5. `describe_jni_result`
6. `GlobalAgentAlreadySet` and `set_global_agent`
7. Modules: `agent`, `callbacks`, `env`, `mutf8`, `version`, `sys`, `classfile`, `prelude`, `embed` (feature-gated), `advanced` (feature-gated)

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
5. `WeakGlobalRef`
6. `JvmtiAllocation`
7. `JniFunctionTable`
8. `JniVersionError`
9. `ThreadInfo`
10. `ThreadGroupInfo`
11. `MonitorUsage`
12. `StackInfo`
13. `ExtensionParamInfo`
14. `ExtensionFunctionInfo`
15. `ExtensionEventInfo`
16. `LocalVariableEntry`
17. `RawMonitor`
18. `RawMonitorGuard`
19. `PrimitiveArrayElements`
20. `PrimitiveArrayCritical`
21. `StringCritical`
22. `LocalFrame`
23. `JavaMonitorGuard`

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

Allocation-free `JniEnv` input methods:
1. `find_class_cstr`
2. `define_class_cstr`
3. `throw_new_cstr`
4. `new_string_utf_cstr`
5. `get_method_id_cstr`
6. `get_static_method_id_cstr`
7. `get_field_id_cstr`
8. `get_static_field_id_cstr`
9. `new_string_utf16`

Each method accepts borrowed `&CStr` input. The existing `&str` convenience
methods remain public and perform temporary conversion where required.

`JniEnv` covers every fixed-signature JNI native operation and uses typed
`jvalue` (`A`) invocation families. The C variadic and `va_list` slots remain
raw-only in `sys::jni`. Native primitive-array and critical-region leases are
represented by allocation-free RAII guards and cannot be accidentally exposed
as unmatched high-level acquire/release pairs.
Local-reference frames and entered Java monitors use the same owning pattern.

## `mutf8` Module

1. `encode`, `encode_utf16`, and `encode_cstring`
2. `validate`, `decode`, `decode_utf16`, and `decode_cow`
3. `decode_cstr`, `decode_cstr_cow`, and explicit lossy variants
4. `Mutf8Error` and `Mutf8ErrorKind`

This is Java Modified UTF-8, not ordinary UTF-8. Exact UTF-16 conversion is
available for Java strings containing unpaired surrogate code units.

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

1. `ClassFile` and supporting structs/enums for typed JVMS-standard attributes through Java 28, with opaque preservation of unknown and VM-specific attributes.
2. `ClassFile::parse(bytes)` entry point.
3. `JavaString` for exact Java Modified UTF-8 values, including unpaired UTF-16 surrogates.
4. `ClassFileParseLimits` and `ClassFile::parse_with_limits` for explicit
   input-size, cumulative-allocation, recursive-attribute, and
   recursive-annotation bounds.

## `prelude` Module

Recommended imports for agent authors:
1. `Agent`, `export_agent!`, `get_default_callbacks`
2. `agent` lifecycle contexts and complete `callbacks` payloads
3. `env::{Jvmti, JniEnv, LocalRef, GlobalRef, WeakGlobalRef, JvmtiAllocation,
   JniFunctionTable, PrimitiveArrayElements, PrimitiveArrayCritical,
   StringCritical, LocalFrame, JavaMonitorGuard, RawMonitor, RawMonitorGuard}`
4. `version` release profiles, deltas, feature gates, and compatibility metadata
5. `sys::{jni, jvmti}`
6. `embed::{JavaVmBuilder, JavaVm, AttachedThread}` when the `embed` feature is enabled
7. `mutf8::{Mutf8Error, Mutf8ErrorKind}`

## `embed` Module

Feature-gated JVM embedding helpers (`embed` feature):
1. `JavaVmBuilder`
2. `JavaVm`
3. `AttachedThread`
4. `find_libjvm`
5. `find_libjvm_verbose`
6. `EmbedError`

The implementation uses an in-tree Unix/Windows dynamic-library loader and
adds no feature dependency. `JavaVm` owns the dynamic-library handle, JVM
option strings, and native option table until after JVM destruction.
Safe worker-thread access uses `AttachedThread` guards or scoped closure
helpers; manual `get_env`, attach, and detach operations are unsafe.

## `advanced` Module

Feature-gated helpers (disabled by default):
1. `advanced::heap_graph` (`heap-graph` feature)

## Stability Notes

1. `env` and top-level exports are intended to be stable.
2. `sys` is a low-level mirror of JNI/JVMTI C headers.
3. `advanced` is explicitly allowed to change more rapidly.
4. Normal, optional, build, and development dependency counts are all zero.
5. The complete 2.x-to-3.0 source migration is documented in
   [Migrating From 2.x to 3.0](MIGRATING_2_TO_3.md).
6. Crate-produced event and metadata records are non-exhaustive so additive JDK
   data does not force a 4.0 release.
