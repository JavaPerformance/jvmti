# jvmti

Complete JNI and JVMTI bindings for Rust with **zero dependencies**.

Build JVM agents in pure Rust.

## Features

- **Complete Coverage** - All 236 JNI functions, all 156 JVMTI functions
- **Zero Dependencies** - No external crates required
- **Ergonomic API** - High-level wrappers with Rust-friendly types
- **Type-Safe** - `Result` returns, RAII reference guards
- **JDK 8-27 Compatible** - Verified against JDK 27 headers

## Quick Start

### 1. Create a new library

```bash
cargo new --lib my_agent
cd my_agent
```

### 2. Configure Cargo.toml

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
jvmti = "0.1"
```

### 3. Write your agent (src/lib.rs)

```rust
use jvmti::{Agent, export_agent};
use jvmti::sys::jni;

#[derive(Default)]
struct MyAgent;

impl Agent for MyAgent {
    fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
        println!("[MyAgent] Loaded with options: {}", options);
        jni::JNI_OK
    }

    fn vm_init(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread) {
        println!("[MyAgent] VM initialized!");
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
│   env::Jvmti - JVMTI operations (153 methods)            │
│   env::JniEnv - JNI operations (60+ methods)             │
│   env::LocalRef, GlobalRef - RAII reference guards       │
├─────────────────────────────────────────────────────────┤
│              Raw FFI Bindings (sys module)               │
│   sys::jni - JNI types, vtable (236 functions)           │
│   sys::jvmti - JVMTI types, vtable (156 functions)       │
└─────────────────────────────────────────────────────────┘
```

## Modules

| Module | Purpose |
|--------|---------|
| `sys::jni` | Raw JNI types, constants, vtable |
| `sys::jvmti` | Raw JVMTI types, capabilities, events, vtable |
| `env` | **High-level wrappers** - start here! |
| `env::Jvmti` | JVMTI environment wrapper |
| `env::JniEnv` | JNI environment wrapper |
| `env::LocalRef` | RAII guard for local references |
| `env::GlobalRef` | RAII guard for global references |

## The Agent Trait

Implement `Agent` to handle JVMTI events. All methods have default no-op implementations.

```rust
pub trait Agent: Sync + Send {
    // Lifecycle
    fn on_load(&self, vm: *mut JavaVM, options: &str) -> jint;
    fn on_unload(&self) {}

    // VM Events
    fn vm_init(&self, jni: *mut JNIEnv, thread: jthread) {}
    fn vm_death(&self, jni: *mut JNIEnv) {}
    fn vm_start(&self, jni: *mut JNIEnv) {}

    // Thread Events
    fn thread_start(&self, jni: *mut JNIEnv, thread: jthread) {}
    fn thread_end(&self, jni: *mut JNIEnv, thread: jthread) {}

    // Class Events
    fn class_load(&self, jni: *mut JNIEnv, thread: jthread, klass: jclass) {}
    fn class_prepare(&self, jni: *mut JNIEnv, thread: jthread, klass: jclass) {}
    fn class_file_load_hook(&self, /* ... */) {}

    // Method Events
    fn method_entry(&self, jni: *mut JNIEnv, thread: jthread, method: jmethodID) {}
    fn method_exit(&self, jni: *mut JNIEnv, thread: jthread, method: jmethodID) {}

    // Exception Events
    fn exception(&self, /* ... */) {}
    fn exception_catch(&self, /* ... */) {}

    // GC Events
    fn garbage_collection_start(&self) {}
    fn garbage_collection_finish(&self) {}

    // ... and 20+ more events
}
```

## Enabling Events

Events require three steps:

```rust
fn on_load(&self, vm: *mut jni::JavaVM, _options: &str) -> jni::jint {
    use jvmti::env::Jvmti;
    use jvmti::sys::jvmti;

    let jvmti_env = Jvmti::new(vm).expect("Failed to get JVMTI");

    // 1. Request capabilities
    let mut caps = jvmti::jvmtiCapabilities::default();
    caps.set_can_generate_method_entry_events(true);
    jvmti_env.add_capabilities(&caps).expect("capabilities");

    // 2. Set up callbacks (connects events to your Agent impl)
    let callbacks = jvmti::get_default_callbacks();
    jvmti_env.set_event_callbacks(callbacks).expect("callbacks");

    // 3. Enable specific events
    jvmti_env.set_event_notification_mode(
        true,  // enable
        jvmti::JVMTI_EVENT_METHOD_ENTRY,
        std::ptr::null_mut()  // all threads
    ).expect("enable event");

    jni::JNI_OK
}
```

## Using the JNI Wrapper

The `env::JniEnv` wrapper provides ergonomic JNI access:

```rust
use jvmti::env::JniEnv;
use jvmti::sys::jni;

fn vm_init(&self, jni_ptr: *mut jni::JNIEnv, _thread: jni::jthread) {
    let jni = unsafe { JniEnv::from_raw(jni_ptr) };

    // Find classes
    let string_class = jni.find_class("java/lang/String").unwrap();

    // Create strings
    let greeting = jni.new_string_utf("Hello from Rust!").unwrap();

    // Call methods
    let length_method = jni.get_method_id(string_class, "length", "()I").unwrap();
    let len = jni.call_int_method(greeting, length_method, &[]);
    println!("String length: {}", len);

    // Handle exceptions
    if jni.exception_check() {
        jni.exception_describe();
        jni.exception_clear();
    }
}
```

## Common Capabilities

| Capability | Required For |
|------------|--------------|
| `can_generate_all_class_hook_events` | `class_file_load_hook` |
| `can_generate_method_entry_events` | `method_entry` |
| `can_generate_method_exit_events` | `method_exit` |
| `can_generate_exception_events` | `exception`, `exception_catch` |
| `can_generate_field_access_events` | `field_access` |
| `can_generate_field_modification_events` | `field_modification` |
| `can_generate_single_step_events` | `single_step` |
| `can_generate_breakpoint_events` | `breakpoint` |
| `can_generate_frame_pop_events` | `frame_pop` |
| `can_tag_objects` | Object tagging, heap iteration |
| `can_retransform_classes` | `retransform_classes()` |
| `can_redefine_classes` | `redefine_classes()` |

## Common Events

| Event | Fires When |
|-------|-----------|
| `JVMTI_EVENT_VM_INIT` | JVM initialization complete |
| `JVMTI_EVENT_VM_DEATH` | JVM shutting down |
| `JVMTI_EVENT_CLASS_FILE_LOAD_HOOK` | Class bytecode loaded (can modify!) |
| `JVMTI_EVENT_CLASS_PREPARE` | Class fully loaded and linked |
| `JVMTI_EVENT_METHOD_ENTRY` | Method entered |
| `JVMTI_EVENT_METHOD_EXIT` | Method exited |
| `JVMTI_EVENT_EXCEPTION` | Exception thrown |
| `JVMTI_EVENT_THREAD_START` | Thread started |
| `JVMTI_EVENT_THREAD_END` | Thread ended |
| `JVMTI_EVENT_GARBAGE_COLLECTION_START` | GC starting |
| `JVMTI_EVENT_GARBAGE_COLLECTION_FINISH` | GC finished |

## JDK Compatibility

| JDK | JNI | JVMTI | Notable Additions |
|-----|-----|-------|-------------------|
| 8   | 232 | 153   | Baseline |
| 9   | 233 | 156   | GetModule, AddModuleReads/Exports/Opens |
| 11  | 233 | 156   | SetHeapSamplingInterval |
| 21  | 234 | 156   | IsVirtualThread, virtual thread events |
| 24  | 235 | 156   | GetStringUTFLengthAsLong |
| 27  | 235 | 156   | ClearAllFramePops |

## Examples

The crate includes runnable examples:

```bash
# Minimal agent - just prints lifecycle events
cargo build --release --example minimal
java -agentpath:./target/release/examples/libminimal.so MyApp

# Method counter - counts method entries/exits
cargo build --release --example method_counter
java -agentpath:./target/release/examples/libmethod_counter.so MyApp

# Class logger - logs all class loads
cargo build --release --example class_logger
java -agentpath:./target/release/examples/libclass_logger.so MyApp
```

## Thread Safety

Your agent must be `Sync + Send` because:
- JVMTI events fire from multiple JVM threads
- The same agent instance handles all events

Use appropriate synchronization:

```rust
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

#[derive(Default)]
struct MyAgent {
    // Atomics for simple counters
    method_count: AtomicU64,

    // Mutex for complex state
    class_names: Mutex<Vec<String>>,
}
```

## Why This Crate?

Existing JVMTI crates (`jvmti-rs`, `jvmti`) are:
- Abandoned (7+ years old)
- Incomplete (missing many functions)
- Have external dependencies

This crate provides:
- Complete JNI/JVMTI coverage
- Zero dependencies
- Verified against modern JDK headers
- Active maintenance
- Comprehensive documentation

## License

MIT OR Apache-2.0
