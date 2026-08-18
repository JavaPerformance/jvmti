# Embedding A JVM From Rust

This crate can embed a JVM inside a Rust process through the `embed` feature.
The feature has zero third-party crate dependencies. Its small internal loader
uses `dlopen`/`dlsym`/`dlclose` on Unix and
`LoadLibraryW`/`GetProcAddress`/`FreeLibrary` on Windows.

## Enable The Feature

```toml
[dependencies]
jvmti-bindings = { version = "3", features = ["embed"] }
```

Rust 1.85 or newer is required.

## Minimal Example

```rust,ignore
use std::io;
use jvmti_bindings::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = JavaVmBuilder::default()
        .option("-Xms64m")?
        .option("-Xmx256m")?
        .option("-Djava.class.path=./myapp.jar")?;

    // Uses JVM_LIB_PATH or JAVA_HOME for auto-discovery.
    let vm = builder.create()?;

    // Only valid on the creating thread.
    let env = unsafe { vm.creator_env() };
    let system = env
        .find_class_cstr(c"java/lang/System")
        .ok_or_else(|| io::Error::other("java.lang.System was not found"))?;
    let get_prop = unsafe {
        env.get_static_method_id_cstr(
            system,
            c"getProperty",
            c"(Ljava/lang/String;)Ljava/lang/String;",
        )
        .ok_or_else(|| io::Error::other("System.getProperty was not found"))?
    };
    let key = env
        .new_string_utf_cstr(c"java.version")
        .ok_or_else(|| io::Error::other("could not allocate java.version key"))?;
    let value = unsafe {
        env.call_static_object_method(system, get_prop, &[jni::jvalue { l: key }])
    };
    let version = unsafe { env.get_string_utf(value) }
        .unwrap_or_else(|| "<unknown>".to_string());
    println!("java.version={version}");

    vm.destroy()?;
    Ok(())
}
```

The library handle is owned by `JavaVm` and remains live until the VM is
destroyed. Invocation option strings and their C pointer array are retained for
the same lifetime because JVM startup may continue to observe them after
`JNI_CreateJavaVM` returns. Do not retain raw symbols or JVM handles beyond
that lifetime.

## Thread Rules

- `creator_env()` is only valid on the thread that created the JVM.
- For native worker threads, prefer `attach_current_thread_guard()` or
  `with_attached_current_thread()`; they detach only if they attached the
  thread themselves.
- Use the unsafe `attach_current_thread()` / `detach_current_thread()` pair
  directly only when explicit lifecycle control is required and no `JniEnv`
  survives detachment. The safe guard and closure APIs bind the environment to
  the VM and attachment lifetime.

```rust,ignore
let answer = vm.with_attached_current_thread(|env| {
    let Some(cls) = env.find_class_cstr(c"java/lang/Integer") else {
        return "<missing java.lang.Integer>".to_owned();
    };
    let method = unsafe {
        let Some(method) =
            env.get_static_method_id_cstr(cls, c"toString", c"(I)Ljava/lang/String;")
        else {
            return "<missing Integer.toString>".to_owned();
        };
        method
    };
    let value = unsafe {
        env.call_static_object_method(cls, method, &[jni::jvalue { i: 42 }])
    };
    unsafe { env.get_string_utf(value) }.unwrap_or_else(|| "<conversion failed>".to_owned())
})?;

assert_eq!(answer, "42");
```

## Discovery

The helper checks:

1. `JVM_LIB_PATH`, when set, as an explicit `libjvm` path.
2. `JAVA_HOME` using common JDK layouts.

If discovery fails, call
`create_from_library("/path/to/libjvm.so")` directly.

To print the discovered path:

```rust,ignore
use jvmti_bindings::embed::find_libjvm_verbose;
let libjvm = find_libjvm_verbose()?;
let vm = builder.create_from_library(libjvm)?;
```

Typical locations are:

- Linux: `${JAVA_HOME}/lib/server/libjvm.so`
- macOS: `${JAVA_HOME}/lib/server/libjvm.dylib`
- Windows: `${JAVA_HOME}\\bin\\server\\jvm.dll`

If the process already links `libjvm` and has a correctly typed
`JNI_CreateJavaVM` symbol, `JavaVmBuilder::create_with` is the explicit unsafe
escape hatch.

## Loader Safety Boundary

The internal loader intentionally supports only Unix and Windows. Symbol lookup
validates loader success and pointer representation size, but cannot prove a
symbol's C signature. The unsafe caller must provide the exact function-pointer
type and must not let it outlive the library. Cross-platform CI compiles and
tests this feature on Linux, macOS, and Windows; the live embedding smoke test
requires an installed JDK.
