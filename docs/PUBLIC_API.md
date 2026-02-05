# Public API Surface

This crate exposes a deliberately small and stable surface area.

Public modules:
1. `env` - High-level safe wrappers (`Jvmti`, `JniEnv`, `LocalRef`, `GlobalRef`).
2. `sys` - Raw FFI bindings for JNI and JVMTI.
3. `classfile` - Class file parser with Java 8-27 attributes.
4. `prelude` - Recommended imports for agent authors.
5. `advanced` - Feature-gated helpers (disabled by default).

Public items:
1. `Agent` trait
2. `export_agent!` macro
3. `get_default_callbacks` helper
4. `jni` re-export (`crate::sys::jni`)

Stability notes:
1. `sys` follows the JVMTI/JNI C headers and may grow with new JDK versions.
2. `env` is the recommended API for most users and aims for stability.
3. `advanced` APIs can change faster and are feature-gated.
