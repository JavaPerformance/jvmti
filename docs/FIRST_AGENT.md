# Your First Production JVM Agent in Rust

This guide walks through building a small but production-grade JVM agent using `jvmti-bindings`.

The goal is not to show off every feature — it's to show:
- Where things can go wrong
- How to structure an agent safely
- How to avoid the most common JVMTI mistakes

We'll build an agent that:
- Counts loaded classes
- Logs JVM startup/shutdown
- Is safe to run in production without destabilizing the VM

## What You Should Know Already

This guide assumes:
- You know Rust basics (ownership, `Result`, traits)
- You know what a JVM agent is (`-agentpath`)
- You are comfortable debugging native code if needed

You do **not** need prior JVMTI experience.

---

## 1. Project Setup

Create a new library crate:

```bash
cargo new --lib class_counter_agent
cd class_counter_agent
```

In `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
jvmti-bindings = "2"
```

**Why `cdylib`?**
- The JVM loads agents as native shared libraries (`.so`, `.dylib`, `.dll`)
- `cdylib` produces a clean C-compatible shared library

---

## 2. Define Your Agent State

A JVM agent is long-lived. You must assume:
- Callbacks happen on different threads
- Callbacks may race
- Callbacks may happen very early or very late in VM lifetime

Define state explicitly:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct ClassCounterAgent {
    loaded_classes: AtomicU64,
}
```

**Why atomics?**
- JVMTI callbacks can run concurrently
- Locks inside callbacks increase deadlock risk
- Atomics are cheap and predictable

---

## 3. Implement the Agent Trait

The `Agent` trait is your contract with the JVM:

```rust
use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct ClassCounterAgent {
    loaded_classes: AtomicU64,
}

impl Agent for ClassCounterAgent {
    fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
        eprintln!("[agent] Loading class counter agent");
        if !options.is_empty() {
            eprintln!("[agent] Options: {}", options);
        }

        // Get JVMTI environment
        let jvmti_env = match Jvmti::new(vm) {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[agent] Failed to get JVMTI: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        // 1. Request capabilities (must happen in on_load)
        let mut caps = jvmti::jvmtiCapabilities::default();
        caps.set_can_generate_all_class_hook_events(true);
        if let Err(e) = jvmti_env.add_capabilities(&caps) {
            eprintln!("[agent] Failed to add capabilities: {:?}", e);
            return jni::JNI_ERR;
        }

        // 2. Register callbacks (before enabling events!)
        let callbacks = get_default_callbacks();
        if let Err(e) = jvmti_env.set_event_callbacks(callbacks) {
            eprintln!("[agent] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        // 3. Enable events
        if let Err(e) = jvmti_env.set_event_notification_mode(
            true,
            jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK,
            std::ptr::null_mut(),
        ) {
            eprintln!("[agent] Failed to enable class hook: {:?}", e);
            return jni::JNI_ERR;
        }

        let _ = jvmti_env.set_event_notification_mode(
            true,
            jvmti::JVMTI_EVENT_VM_DEATH,
            std::ptr::null_mut(),
        );

        jni::JNI_OK
    }

    fn vm_init(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread) {
        eprintln!("[agent] VM initialized");
    }

    fn class_file_load_hook(
        &self,
        _jni: *mut jni::JNIEnv,
        _class_being_redefined: jni::jclass,
        _loader: jni::jobject,
        _name: *const std::os::raw::c_char,
        _protection_domain: jni::jobject,
        _class_data_len: jni::jint,
        _class_data: *const u8,
        _new_class_data_len: *mut jni::jint,
        _new_class_data: *mut *mut u8,
    ) {
        // Just count - this is a hot path, keep it fast!
        self.loaded_classes.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _jni: *mut jni::JNIEnv) {
        let count = self.loaded_classes.load(Ordering::Relaxed);
        eprintln!("[agent] VM shutting down");
        eprintln!("[agent] Total classes loaded: {}", count);
    }
}

export_agent!(ClassCounterAgent);
```

---

## 4. The Three-Step Initialization Pattern

JVMTI requires a specific order. Get this wrong and the JVM will crash or silently ignore your events.

```
1. Request capabilities  →  Must happen in on_load, before VM starts
2. Register callbacks    →  Must happen before enabling events
3. Enable events         →  Only after callbacks are registered
```

The `export_agent!` macro handles some of this, but **you** must:
- Choose which capabilities to request
- Choose which events to enable
- Handle errors explicitly

---

## 5. Important Rules (Don't Skip This)

Inside callbacks:

| Rule | Why |
|------|-----|
| **Do not block** | Deadlocks the VM |
| **Do not allocate excessively** | GC can't run during some callbacks |
| **Do not call into Java carelessly** | Wrong phase = crash |
| **Return quickly** | You're blocking VM threads |

**Treat callbacks like signal handlers with privileges.**

JVMTI is not a normal runtime. A callback that takes too long or does too much will destabilize the entire JVM.

---

## 6. Build the Agent

```bash
cargo build --release
```

The output will be:
- **Linux:** `target/release/libclass_counter_agent.so`
- **macOS:** `target/release/libclass_counter_agent.dylib`
- **Windows:** `target/release/class_counter_agent.dll`

---

## 7. Run With the JVM

```bash
java -agentpath:./target/release/libclass_counter_agent.so MyApp
```

Expected output:

```
[agent] Loading class counter agent
[agent] VM initialized
... your app output ...
[agent] VM shutting down
[agent] Total classes loaded: 136
```

**If the JVM crashes:**
- Run with `-Xcheck:jni` for JNI validation
- Run under `gdb` / `lldb`
- Add logging inside callbacks (sparingly)

Crashes are bugs — but they're diagnosable.

---

## 8. Production Hardening Checklist

Before shipping:

### Logging
- [ ] Prefer `stderr` or structured logging
- [ ] Avoid logging in hot callbacks (like `class_file_load_hook`)
- [ ] Add a "quiet" option to suppress startup messages

### Error Handling
- [ ] Never `panic!` in callbacks
- [ ] Treat `Err` as fatal only when necessary
- [ ] Log errors but don't crash the VM

### Performance
- [ ] Measure callback frequency under load
- [ ] Avoid JNI calls inside high-volume events
- [ ] Use atomics, not locks

### Shutdown
- [ ] Expect `vm_death` to be called late
- [ ] Avoid allocations during shutdown
- [ ] Don't assume other threads are still running

---

## 9. What NOT To Do in Your First Agent

These are powerful features that deserve their own guides:

- **Bytecode rewriting** — One wrong byte crashes the JVM
- **Heap walking** — Stop-the-world implications
- **Object tagging** — Complex lifecycle management
- **Thread suspension** — Deadlock minefield
- **Calling arbitrary Java code** — Phase restrictions

Start simple. Add complexity only when you understand the constraints.

---

## 10. Where to Go Next

Once this works reliably:

1. **Add metrics export** — Prometheus, StatsD, etc.
2. **Track method entry/exit** — Use `JVMTI_EVENT_METHOD_ENTRY`
3. **Experiment with bytecode hooks** — Modify classes at load time
4. **Integrate with async-profiler or perf** — Combine native and managed profiling

Each of these has real footguns — and deserves careful design.

---

## Final Advice

If you remember one thing:

> **A good JVMTI agent is boring.**
> Fast, quiet, predictable, and invisible.

Rust helps.
This crate helps.
But discipline matters most.

---

## Complete Working Example

The full source code for this guide is available at:
- [examples/class_logger.rs](../examples/class_logger.rs) — Similar pattern with class name logging

Run the example:

```bash
cargo build --release --example class_logger
java -agentpath:./target/release/examples/libclass_logger.so MyApp
```
