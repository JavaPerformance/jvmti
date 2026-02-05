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

    let vm = builder.create_from_library("/path/to/libjvm.so")?;

    // Only valid on the creating thread:
    let env = unsafe { vm.creator_env() };
    let system = env.find_class("java/lang/System").unwrap();
    let _ = system;

    vm.destroy()?;
    Ok(())
}
```

## Thread Rules

- `creator_env()` is **only valid on the thread that created the JVM**.
- For any other thread, use `attach_current_thread()` and call
  `detach_current_thread()` when you are done.

## Notes

- On Linux, `libjvm.so` is typically under `${JAVA_HOME}/lib/server/`.
- If you already link to `libjvm` and have a `JNI_CreateJavaVM` symbol,
  you can call `JavaVmBuilder::create_with` directly (unsafe).
