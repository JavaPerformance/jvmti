# Public API Surface

This crate exposes a deliberately small and stable surface area.

Public modules:
1. `env` - High-level wrappers (`Jvmti`, `JniEnv`, `JvmtiAllocation`, `LocalRef`, `GlobalRef`).
2. `sys` - Raw FFI bindings for JNI and JVMTI.
3. `classfile` - Class file parser with Java 8-27 attributes.
4. `prelude` - Recommended imports for agent authors.
5. `embed` - Feature-gated JVM embedding helpers.
6. `advanced` - Feature-gated helpers (disabled by default).
7. `agent` - Callback-scoped load, attach, unload, and `JavaVM` contexts.
8. `callbacks` - Canonical complete callback payloads and borrowed environments.
9. `version` - JDK 8-28 release profiles, adjacent deltas, maturity, and runtime gates.

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

Stability notes:
1. `sys` follows the JVMTI/JNI C headers and may grow with new JDK versions.
2. `env` is the recommended API for most users and aims for stability.
3. `embed` is feature-gated but intended for stable JVM embedding workflows.
4. `advanced` APIs can change faster and are feature-gated.
5. Consumers upgrading from 2.x must follow the callback, ownership, unsafe-operation, and raw-ABI mappings in [Migrating From 2.x to 3.0](MIGRATING_2_TO_3.md).
