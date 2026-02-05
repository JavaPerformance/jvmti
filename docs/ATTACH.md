# Dynamic Attach (Agent_OnAttach)

This crate supports dynamic attach via the `Agent_OnAttach` entry point.
Implement `Agent::on_attach` and use `export_agent!` — the macro generates
the correct native entry points automatically.

## Minimal Example

```rust,ignore
use jvmti_bindings::prelude::*;

#[derive(Default)]
struct AttachLogger;

impl Agent for AttachLogger {
    fn on_attach(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
        println!("[AttachLogger] attached with options: {}", options);
        let _jvmti = Jvmti::new(vm).expect("get JVMTI");
        jni::JNI_OK
    }
}

export_agent!(AttachLogger);
```

## Notes

- `on_attach` is called when the agent is loaded via the JVM Attach API.
- You can request capabilities and enable JVMTI events inside `on_attach`.
- Thread and JNI safety rules still apply (see `docs/SAFETY.md`).
