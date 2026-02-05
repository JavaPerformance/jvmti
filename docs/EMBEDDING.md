# Embedding A JVM From Rust

This crate can embed a JVM inside a Rust process via the `embed` feature.
It keeps the core crate dependency-free by default and only uses dynamic
loading when `embed` is enabled.

## Enable The Feature

```toml
[dependencies]
jvmti-bindings = { version = "2", features = ["embed"] }
```

## Minimal Example

```rust,ignore
use jvmti_bindings::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = JavaVmBuilder::new(jni::JNI_VERSION_1_8)
        .option("-Xms64m")?
        .option("-Xmx256m")?
        .option("-Djava.class.path=./myapp.jar")?;

    // Uses JVM_LIB_PATH or JAVA_HOME for auto-discovery.
    let vm = builder.create()?;

    // Only valid on the creating thread:
let env = unsafe { vm.creator_env() };
let system = env.find_class("java/lang/System").unwrap();
let get_prop = env
    .get_static_method_id(system, "getProperty", "(Ljava/lang/String;)Ljava/lang/String;")
    .unwrap();
let key = env.new_string_utf("java.version").unwrap();
let value = env.call_static_object_method(system, get_prop, &[jni::jvalue { l: key }]);
let version = env.get_string_utf(value).unwrap_or_else(|| "<unknown>".to_string());
println!("java.version={}", version);

    vm.destroy()?;
    Ok(())
}
```

## Thread Rules

- `creator_env()` is **only valid on the thread that created the JVM**.
- For any other thread, use `attach_current_thread()` and call
  `detach_current_thread()` when you are done.

## Discovery

The helper uses:

1. `JVM_LIB_PATH` if set (absolute path to `libjvm`)
2. `JAVA_HOME` with common JDK layouts

If discovery fails, call `create_from_library("/path/to/libjvm.so")` directly.

To print the discovered path, use:

```rust,ignore
use jvmti_bindings::embed::find_libjvm_verbose;
let libjvm = find_libjvm_verbose()?;
let vm = builder.create_from_library(libjvm)?;
```

## Notes

- On Linux, `libjvm.so` is typically under `${JAVA_HOME}/lib/server/`.
- On macOS, `libjvm.dylib` is typically under `${JAVA_HOME}/lib/server/`.
- On Windows, `jvm.dll` is typically under `${JAVA_HOME}\\bin\\server\\`.
- If you already link to `libjvm` and have a `JNI_CreateJavaVM` symbol,
  you can call `JavaVmBuilder::create_with` directly (unsafe).
