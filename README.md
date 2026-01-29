# jvmti-bindings

Safe, complete JNI and JVMTI bindings for Rust with **zero dependencies**.

Build production-grade JVM agents, profilers, and runtime tools in Rust with explicit safety boundaries and idiomatic ergonomics.

## Why Rust for JVMTI?

JVMTI is powerful but dangerous. One wrong move and you get:
- Segfaults from dangling references
- Memory leaks from forgotten deallocations
- Silent corruption from unchecked error codes
- Deadlocks from callbacks that break JVM invariants

This crate provides:
- **Explicit ownership** - References have clear lifetimes, RAII guards prevent leaks
- **Centralized unsafety** - All raw JVM interaction is auditable in one place
- **Forced error handling** - Every JVMTI call returns `Result`, no silent failures
- **Type-safe callbacks** - The `Agent` trait gives you correctly-typed event handlers

This is not a thin wrapper. It's a safety-oriented mental model for JVMTI in Rust.

## Design Goals

| Goal | How |
|------|-----|
| **Explicit safety model** | Unsafe operations centralized; API surfaces return `Result` |
| **No hidden dependencies** | No bindgen, no build-time JVM, no global allocators |
| **Complete JVMTI surface** | All 236 JNI + 156 JVMTI functions, mapped to Rust types |
| **Agent-first ergonomics** | Structured callbacks, capability management, RAII resources |
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

## What `export_agent!` Does (and Doesn't Do)

The macro generates the native entry points the JVM expects. Understanding what it does removes the magic:

**It does:**
- Generate `Agent_OnLoad` and `Agent_OnUnload` FFI entry points
- Initialize the JVMTI environment from the JavaVM pointer
- Parse the options string and pass it to your `on_load()`
- Store your agent instance globally (it must be `Sync + Send`)
- Wire up all JVMTI event callbacks to your `Agent` trait methods

**It does not:**
- Suppress JVM crashes from undefined JVMTI behavior
- Make callbacks re-entrant-safe (that's your responsibility)
- Automatically attach arbitrary native threads
- Catch panics across FFI boundaries (don't panic in callbacks)

## Safety Model

This crate enforces these invariants:

| Invariant | Enforcement |
|-----------|-------------|
| `JNIEnv` is thread-local | `JniEnv` wrapper is not `Send` |
| Local refs don't outlive their frame | `LocalRef<'a>` tied to `JniEnv` lifetime |
| Global refs are explicitly freed | `GlobalRef` releases on `Drop` |
| JVMTI memory uses JVMTI allocator | Wrapper methods handle alloc/dealloc |
| Errors are never ignored | All methods return `Result<T, jvmtiError>` |

### What Remains Unsafe

Some things cannot be made safe by design:

- **Bytecode transformation correctness** - If you emit invalid bytecode, the JVM will crash
- **Callback timing assumptions** - JVMTI events fire at specific JVM phases; misuse causes UB
- **Blocking in callbacks** - Long operations in GC callbacks can deadlock the JVM
- **Cross-thread reference sharing** - JNI local refs are thread-local; sharing them is UB

We document these constraints; we cannot prevent them at compile time.

## Is This Crate For You?

**Yes, if you are:**
- Building profilers, tracers, debuggers, or instrumentation tools
- Want Rust's type system around JVMTI's sharp edges
- Need a single crate that works across JDK 8-27
- Comfortable reading JVMTI documentation for advanced use cases

**Probably not, if you:**
- Only need basic JNI calls (consider `jni` crate instead)
- Are uncomfortable debugging native JVM crashes
- Need dynamic JVM attachment (attach API not yet wrapped)
- Want a pure-safe API with no `unsafe` anywhere

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

## Enabling JVMTI Events

Events require three steps - capabilities, callbacks, then enable:

```rust
use jvmti_bindings::env::Jvmti;
use jvmti_bindings::sys::jvmti;
use jvmti_bindings::get_default_callbacks;

fn on_load(&self, vm: *mut jni::JavaVM, _options: &str) -> jni::jint {
    let jvmti_env = Jvmti::new(vm).expect("Failed to get JVMTI");

    // 1. Request capabilities (must be done in on_load)
    let mut caps = jvmti::jvmtiCapabilities::default();
    caps.set_can_generate_all_class_hook_events(true);
    jvmti_env.add_capabilities(&caps).expect("add capabilities");

    // 2. Wire up callbacks (connects events to your Agent impl)
    let callbacks = get_default_callbacks();
    jvmti_env.set_event_callbacks(callbacks).expect("set callbacks");

    // 3. Enable specific events
    jvmti_env.set_event_notification_mode(
        true,
        jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK,
        std::ptr::null_mut(),
    ).expect("enable event");

    jni::JNI_OK
}
```

## Capabilities Quick Reference

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
| 17  | ✅ Tested | - |
| 21  | ✅ Tested | `IsVirtualThread`, virtual thread events |
| 27  | ✅ Verified | `ClearAllFramePops` |

Bindings are generated from JDK 27 headers with backwards compatibility to JDK 8.

## Project Status

| Aspect | Status |
|--------|--------|
| API stability | Pre-1.0, breaking changes possible |
| JDK coverage | 8, 11, 17, 21, 27 |
| JVMTI functions | 156/156 (100%) |
| JNI functions | 236/236 (100%) |
| Dependencies | Zero |
| Testing | Header verification, example agents |

## Examples

```bash
# Minimal agent - lifecycle events only
cargo build --release --example minimal

# Method counter - counts all method entries/exits
cargo build --release --example method_counter

# Class logger - logs every class load
cargo build --release --example class_logger
```

## Contributing

Issues and PRs welcome. For large changes, open an issue first to discuss.

## License

MIT OR Apache-2.0
