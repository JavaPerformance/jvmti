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
    let system = env.find_class_cstr(c"java/lang/System").unwrap();
    let get_prop = unsafe {
        env.get_static_method_id_cstr(
            system,
            c"getProperty",
            c"(Ljava/lang/String;)Ljava/lang/String;",
        )
        .unwrap()
    };
    let key = env.new_string_utf_cstr(c"java.version").unwrap();
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
- Use `attach_current_thread()` / `detach_current_thread()` directly only when
  explicit lifecycle control is required.

```rust,ignore
let answer = vm.with_attached_current_thread(|env| {
    let cls = env.find_class_cstr(c"java/lang/Integer").unwrap();
    let method = unsafe {
        env.get_static_method_id_cstr(cls, c"toString", c"(I)Ljava/lang/String;")
            .unwrap()
    };
    let value = unsafe {
        env.call_static_object_method(cls, method, &[jni::jvalue { i: 42 }])
    };
    unsafe { env.get_string_utf(value) }.unwrap()
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
