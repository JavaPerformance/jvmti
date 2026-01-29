# jvmti-bindings

Write JVM agents in Rust with explicit safety boundaries and production-grade ergonomics.

Complete JNI and JVMTI bindings plus higher-level abstractions designed for building profilers, tracers, debuggers, and runtime instrumentation — without writing C or C++.

This crate focuses on:
- Making ownership, lifetimes, and error handling explicit
- Reducing common JVMTI footguns
- Keeping unsafe behavior localized and auditable

It is intended for serious native JVM tooling, not just experimentation.

## Why This Exists

JVMTI is powerful — and notoriously easy to misuse.

Typical problems when writing agents:
- Unchecked error codes that silently corrupt state
- Invalid reference lifetimes causing segfaults
- Allocator mismatches leaking memory
- Thread-local `JNIEnv` misuse across callbacks
- Undocumented callback constraints causing deadlocks

Most existing Rust options either:
- Expose raw bindings with little guidance
- Rely on build-time bindgen
- Are incomplete or unmaintained (7+ years)
- Optimize for JNI, not JVMTI agents

This crate was designed around how agents are actually written, not around mirroring C headers.

## Design Goals

| Goal | How |
|------|-----|
| **Explicit safety model** | Unsafe operations centralized; APIs return `Result` |
| **Complete surface** | All 236 JNI + 156 JVMTI functions, mapped to Rust types |
| **Agent-first ergonomics** | Structured callbacks, capability management, RAII resources |
| **No hidden dependencies** | No bindgen, no build-time JVM, no global allocators |
| **Long-term compatibility** | Verified against OpenJDK headers, JDK 8 through 27 |

## Quick Start

### 1. Create your crate

```bash
cargo new --lib my_agent
cd my_agent
```

### 2. Configure Cargo.toml

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
jvmti-bindings = "0.1"
```

### 3. Implement your agent

```rust
use jvmti_bindings::{Agent, export_agent};
use jvmti_bindings::sys::jni;

#[derive(Default)]
struct MyAgent;

impl Agent for MyAgent {
    fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
        println!("[MyAgent] Loaded with options: {}", options);
        jni::JNI_OK
    }

    fn vm_init(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread) {
        println!("[MyAgent] VM initialized");
    }

    fn vm_death(&self, _jni: *mut jni::JNIEnv) {
        println!("[MyAgent] VM shutting down");
    }
}

export_agent!(MyAgent);
```

### 4. Build and run

```bash
cargo build --release

# Linux
java -agentpath:./target/release/libmy_agent.so=myoptions MyApp

# macOS
java -agentpath:./target/release/libmy_agent.dylib=myoptions MyApp

# Windows
java -agentpath:./target/release/my_agent.dll=myoptions MyApp
```

## What `export_agent!` Does

The macro generates the native entry points the JVM expects.

**It does:**
- Generate `Agent_OnLoad` / `Agent_OnUnload` FFI entry points
- Initialize JNI and JVMTI environments
- Register callbacks before enabling events
- Convert JVMTI error codes into `Result<T, JvmtiError>`
- Store your agent instance globally (must be `Sync + Send`)

**It does not:**
- Hide undefined JVMTI behavior
- Make callbacks re-entrant or async-safe
- Attach arbitrary native threads automatically
- Prevent JVM crashes from invalid JVMTI usage

The goal is clarity, not magic.

## Safety Model

This crate enforces the following invariants:

| Invariant | Enforcement |
|-----------|-------------|
| `JNIEnv` is thread-local | `JniEnv` wrapper is not `Send` |
| Local refs don't escape | `LocalRef<'a>` tied to `JniEnv` lifetime |
| Global refs are freed | `GlobalRef` releases on `Drop` |
| JVMTI memory properly freed | Wrapper methods handle alloc/dealloc |
| Errors are never ignored | All methods return `Result<T, jvmtiError>` |

### What Remains Unsafe

Some things cannot be made safe by design:

- **Bytecode transformation correctness** — invalid bytecode crashes the JVM
- **Callback timing assumptions** — JVMTI events fire at specific phases
- **Blocking in callbacks** — long operations in GC callbacks deadlock
- **Cross-thread reference sharing** — JNI local refs are thread-local

Rust helps — but JVMTI is still a sharp tool.

## Is This For You?

**Yes, if you are:**
- Building profilers, tracers, debuggers, or instrumentation
- Want Rust's type system around JVMTI's sharp edges
- Need a single crate that works across JDK 8–27
- Comfortable reading JVMTI docs for advanced use cases

**Probably not, if you:**
- Only need basic JNI calls (consider the `jni` crate)
- Are uncomfortable debugging native JVM crashes
- Need dynamic attach API (not yet wrapped)
- Want zero `unsafe` anywhere

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Your Agent Code                       │
│         impl Agent for MyAgent { ... }                   │
├─────────────────────────────────────────────────────────┤
│                   Agent Trait + Macros                   │
│      Agent, export_agent!, get_default_callbacks()       │
├─────────────────────────────────────────────────────────┤
│              High-Level Wrappers (env module)            │
│   Jvmti      - JVMTI operations (153 methods)            │
│   JniEnv     - JNI operations (60+ methods)              │
│   LocalRef   - RAII guard, prevented from escaping       │
│   GlobalRef  - RAII guard, releases on drop              │
├─────────────────────────────────────────────────────────┤
│              Raw FFI Bindings (sys module)               │
│   sys::jni   - Complete JNI vtable (236 functions)       │
│   sys::jvmti - Complete JVMTI vtable (156 functions)     │
└─────────────────────────────────────────────────────────┘
```

## Enabling Events

Events require three steps — capabilities, callbacks, then enable:

```rust
use jvmti_bindings::env::Jvmti;
use jvmti_bindings::sys::jvmti;
use jvmti_bindings::get_default_callbacks;

fn on_load(&self, vm: *mut jni::JavaVM, _options: &str) -> jni::jint {
    let jvmti_env = Jvmti::new(vm).expect("Failed to get JVMTI");

    // 1. Request capabilities (must happen in on_load)
    let mut caps = jvmti::jvmtiCapabilities::default();
    caps.set_can_generate_all_class_hook_events(true);
    jvmti_env.add_capabilities(&caps).expect("capabilities");

    // 2. Wire callbacks to your Agent impl
    let callbacks = get_default_callbacks();
    jvmti_env.set_event_callbacks(callbacks).expect("callbacks");

    // 3. Enable specific events
    jvmti_env.set_event_notification_mode(
        true,
        jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK,
        std::ptr::null_mut(),
    ).expect("enable");

    jni::JNI_OK
}
```

## Capabilities Reference

| Capability | Required For |
|------------|--------------|
| `can_generate_all_class_hook_events` | `class_file_load_hook` |
| `can_generate_method_entry_events` | `method_entry` |
| `can_generate_method_exit_events` | `method_exit` |
| `can_generate_exception_events` | `exception`, `exception_catch` |
| `can_tag_objects` | Object tagging, heap iteration |
| `can_retransform_classes` | `retransform_classes()` |
| `can_redefine_classes` | `redefine_classes()` |
| `can_get_bytecodes` | `get_bytecodes()` |
| `can_get_line_numbers` | `get_line_number_table()` |
| `can_access_local_variables` | `get_local_*()`, `set_local_*()` |

## JDK Compatibility

| JDK | Status | Notable Additions |
|-----|--------|-------------------|
| 8   | ✅ Tested | Baseline |
| 11  | ✅ Tested | `SetHeapSamplingInterval` |
| 17  | ✅ Tested | — |
| 21  | ✅ Tested | Virtual thread support |
| 27  | ✅ Verified | `ClearAllFramePops` |

Bindings generated from JDK 27 headers, backwards compatible to JDK 8.

## Project Status

| Aspect | Status |
|--------|--------|
| API stability | Pre-1.0, breaking changes possible |
| JVMTI coverage | 156/156 (100%) |
| JNI coverage | 236/236 (100%) |
| Dependencies | Zero |
| Testing | Header verification, example agents |

## Examples

```bash
# Minimal agent — lifecycle events only
cargo build --release --example minimal

# Method counter — counts all method entries/exits
cargo build --release --example method_counter

# Class logger — logs every class load
cargo build --release --example class_logger
```

## License

MIT OR Apache-2.0
