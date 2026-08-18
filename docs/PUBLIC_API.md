# Public API Surface

This crate exposes a deliberately small and stable surface area.

Public modules:
1. `env` - High-level wrappers (`Jvmti`, `JniEnv`, `JvmtiAllocation`, `LocalRef`, `GlobalRef`, `WeakGlobalRef`, `PrimitiveArrayElements`, `PrimitiveArrayCritical`, `StringCritical`, `LocalFrame`, `JavaMonitorGuard`, `RawMonitor`, `RawMonitorGuard`).
2. `sys` - Raw FFI bindings for JNI and JVMTI.
3. `classfile` - Resource-bounded class file parser with Java 8-27 attributes and exact `JavaString` values.
4. `prelude` - Recommended imports for agent authors.
5. `embed` - Feature-gated JVM embedding helpers.
6. `advanced` - Feature-gated helpers (disabled by default).
7. `agent` - Callback-scoped load, attach, unload, and `JavaVM` contexts.
8. `callbacks` - Canonical complete callback payloads and borrowed environments.
9. `version` - JDK 8-28 release profiles, adjacent deltas, maturity, and runtime gates.
10. `mutf8` - Java Modified UTF-8 validation and exact/string/UTF-16 conversion.

Public items:
1. `Agent` trait
2. `export_agent!` macro
3. `get_default_callbacks` helper
4. `jni` re-export (`crate::sys::jni`)
5. `describe_jni_result` helper
6. `embed::{JavaVmBuilder, JavaVm, AttachedThread}` when the `embed` feature is enabled

Common high-level helpers:
1. `Jvmti::set_default_agent_callbacks`
2. `Jvmti::add_*_capabilities` presets for class hooks, method tracing, exceptions, and heap sampling
3. `Jvmti::enable_*_events` presets for common event groups
4. `Jvmti::configure_*_agent` presets for common agent workflows
5. `jni::result_name`, `jni::describe_result`, and `jvmti::error_name` diagnostics
6. `RawMonitor` and `RawMonitorGuard` for owned JVM TI synchronization

Allocation-free JNI input helpers:
1. `JniEnv::find_class_cstr`
2. `JniEnv::define_class_cstr`
3. `JniEnv::throw_new_cstr`
4. `JniEnv::new_string_utf_cstr`
5. `JniEnv::get_method_id_cstr`
6. `JniEnv::get_static_method_id_cstr`
7. `JniEnv::get_field_id_cstr`
8. `JniEnv::get_static_field_id_cstr`
9. `JniEnv::new_string_utf16` for caller-provided UTF-16 storage

The corresponding `&str` methods remain available as convenience adapters. Use
the `&CStr` variants with `c"..."` literals or cached names in callback and
lookup hot paths to avoid temporary `CString` allocation and validation.
These native strings use Java Modified UTF-8. The `mutf8` module exposes strict
validation/decoding, lossy decoding, and exact UTF-16 conversion.

JNI fixed-signature coverage:
1. `JniEnv` exposes every fixed-signature JNI native operation.
2. Method and constructor invocation uses the typed `jvalue` (`A`) families.
3. C variadic and `va_list` slots remain raw-only in `sys::jni` because stable
   Rust cannot construct or portably forward arbitrary C variadic arguments.
4. Primitive array element, primitive critical, and string critical leases use
   allocation-free RAII guards that release exactly once.
5. Local frames and entered Java monitors use owning guards; unmatched manual
   operations are explicit unsafe `*_raw` methods.

Dependency contract:
1. The crate has no third-party normal, optional, build, or development dependencies.
2. Enabling `embed` uses the in-tree platform dynamic loader; it does not add a loader crate.
3. Benchmarks and tests use the Rust standard library and installed JDK tools.

Stability notes:
1. `sys` follows the JVMTI/JNI C headers and may grow with new JDK versions.
2. `env` is the recommended API for most users and aims for stability.
3. `embed` is feature-gated but intended for stable JVM embedding workflows; a
   `JavaVm` keeps the loaded library and JVM option storage alive for the VM's
   full lifetime.
4. `advanced` APIs can change faster and are feature-gated.
5. Consumers upgrading from 2.x must follow the callback, ownership, unsafe-operation, and raw-ABI mappings in [Migrating From 2.x to 3.0](MIGRATING_2_TO_3.md).
6. Crate-produced callback and metadata records are `#[non_exhaustive]`; their
   fields remain readable while future JDK data can be added in 3.x.
7. Append-only JNI/JVM TI function and callback tables are non-exhaustive at
   the Rust source boundary. This does not change their C layout; it prevents
   downstream struct literals from turning an upstream table tail into a 4.0.
8. `ClassFile::parse` uses shared input, cumulative-allocation, recursive
   attribute, and recursive-annotation limits; `ClassFile::parse_with_limits`
   and `ClassFileParseLimits` expose deliberate parser-budget overrides.
9. `JavaVm::attach_current_thread_guard` and scoped closure helpers are the safe
   embedding surface. Manual environment acquisition and detach are unsafe.
