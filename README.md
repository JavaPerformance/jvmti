# jvmti-bindings
[![Crates.io](https://img.shields.io/crates/v/jvmti-bindings.svg)](https://crates.io/crates/jvmti-bindings)
[![Docs.rs](https://docs.rs/jvmti-bindings/badge.svg)](https://docs.rs/jvmti-bindings)
[![CI](https://github.com/JavaPerformance/jvmti/actions/workflows/ci.yml/badge.svg)](https://github.com/JavaPerformance/jvmti/actions/workflows/ci.yml)

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

## Comparison with Alternatives

If you only need JNI to call into Java from Rust applications, crates like `jni` or `jni-simple` are often sufficient. This crate is purpose-built for **JVMTI agents** (profilers, tracers, debuggers, instrumentation) and emphasizes:

1. Full JNI + JVMTI coverage (agent-first focus)
2. Safe, owned return types in the high-level `env` wrappers
3. Class file parsing with all standard Java 8-27 attributes
4. A tiny but explicit public surface (`env`, `sys`, `classfile`, `prelude`)
5. Safety guidance, pitfalls, and compatibility documentation
6. Examples that mirror real JVMTI tooling patterns

## Why Rust for JVMTI?

C++ is the traditional choice, but Rust offers compelling advantages:

- **Memory safety without GC** — JVMTI agents run inside the JVM process; a segfault kills the application
- **Fearless concurrency** — JVMTI callbacks fire from multiple threads simultaneously
- **Zero-cost abstractions** — RAII guards and Result types add safety without runtime overhead
- **No runtime dependencies** — Deploy a single `.so`/`.dylib`/`.dll` with no external libraries
- **Modern tooling** — Cargo, docs.rs, and crates.io beat Makefiles and manual distribution

Java agents (`java.lang.instrument`) are simpler but can't access low-level features like heap iteration, breakpoints, or raw bytecode hooks.

## Design Goals

| Goal | How |
|------|-----|
| **Explicit safety model** | Unsafe operations centralized; APIs return `Result` |
| **Complete surface** | All 236 JNI + 156 JVMTI functions, mapped to Rust types |
| **Agent-first ergonomics** | Structured callbacks, capability management, RAII resources |
| **No hidden dependencies** | No bindgen, no build-time JVM, no global allocators |
| **Long-term compatibility** | Verified against OpenJDK headers, JDK 8 through 27 |

## Safety and FFI

This crate is built around explicit safety boundaries. See `docs/SAFETY.md` and `docs/PITFALLS.md` for the full checklist.

Key rules:
1. Never use `JNIEnv` across threads.
2. Never panic across JNI/JVMTI callbacks.
3. Always deallocate JVMTI buffers with `Deallocate`.
4. Avoid JNI calls in GC callbacks.

## Public API

The supported public surface is intentionally small. For most users:
1. Use `env` for safe wrappers.
2. Use `prelude` for standard imports.
3. Use `sys` only for raw FFI work.

Details: `docs/PUBLIC_API.md`.

## Raw FFI Access

If you need raw JNI/JVMTI functions, use:
1. `jvmti_bindings::sys::jni` and `jvmti_bindings::sys::jvmti` for raw types and vtables.
2. `JniEnv::raw()` and `Jvmti::raw()` to access the underlying raw pointers.

## Attach and Threading Rules

1. `Agent_OnAttach` is supported via the `export_agent!` macro and `Agent::on_attach`.
2. `JNIEnv` is thread-local and must only be used on its originating thread.
3. `GlobalRef` cleanup attaches to the JVM when needed, but you should still manage lifetimes explicitly.

## Compatibility

See `docs/COMPATIBILITY.md` for a full JDK 8-27 matrix.

## Advanced Helpers

Feature-gated helpers live under `advanced`:
1. `heap-graph` for heap tagging and reference edge extraction.

Enable with:

```toml
[dependencies]
jvmti-bindings = { version = "2", features = ["heap-graph"] }
```

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
jvmti-bindings = "2"
```

### 3. Implement your agent

```rust
use jvmti_bindings::prelude::*;

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

## Class File Parsing

This crate now includes a zero-dependency class file parser that understands all standard attributes from Java 8 through Java 27. Use it inside `ClassFileLoadHook` to inspect or transform class metadata.

```rust
use jvmti_bindings::classfile::ClassFile;

fn parse_class(bytes: &[u8]) {
    let classfile = ClassFile::parse(bytes).expect("valid class file");
    println!("major version = {}", classfile.major_version);
    println!("attributes = {}", classfile.attributes.len());
}
```

Nested attributes are preserved and exposed (method `Code` attributes, record component attributes, and more). You can traverse them like this:

```rust
use jvmti_bindings::classfile::{AttributeInfo, ClassFile, RecordComponent};

fn walk_attributes(attrs: &[AttributeInfo]) {
    for attr in attrs {
        match attr {
            AttributeInfo::Code(code) => walk_attributes(&code.attributes),
            AttributeInfo::Record { components } => {
                for RecordComponent { attributes, .. } in components {
                    walk_attributes(attributes);
                }
            }
            _ => {}
        }
    }
}

fn parse_class(bytes: &[u8]) {
    let classfile = ClassFile::parse(bytes).expect("valid class file");
    walk_attributes(&classfile.attributes);
    for field in &classfile.fields {
        walk_attributes(&field.attributes);
    }
    for method in &classfile.methods {
        walk_attributes(&method.attributes);
    }
}
```

## Examples

Included examples (build as `cdylib` agents):
1. `examples/minimal.rs`
2. `examples/class_logger.rs`
3. `examples/profiler.rs`
4. `examples/tracer.rs`
5. `examples/heap_sampler.rs`

## Agent Starter Template

See `templates/agent-starter/` for a ready-to-copy agent crate.

## CI

The repository includes a GitHub Actions workflow that builds and tests on Linux, macOS, and Windows.

## What `export_agent!` Does

The macro generates the native entry points the JVM expects.

**It does:**
- Generate `Agent_OnLoad` / `Agent_OnUnload` / `Agent_OnAttach` entry points
- Create your agent instance and store it globally (must be `Sync + Send`)
- Pass the options string to your `on_load` / `on_attach` implementation

**It does not:**
- Hide undefined JVMTI behavior
- Make callbacks re-entrant or async-safe
- Attach arbitrary native threads automatically
- Obtain the JVMTI environment for you
- Register callbacks or enable events
- Prevent JVM crashes from invalid JVMTI usage

The goal is clarity, not magic.

## Safety Model

This crate enforces the following invariants:

| Invariant | Enforcement |
|-----------|-------------|
| `JNIEnv` is thread-local | `JniEnv` wrapper is not `Send` |
| Local refs don't escape | `LocalRef<'a>` tied to `JniEnv` lifetime |
| Global refs are freed | `GlobalRef` releases on `Drop` |
| JVMTI memory properly freed | High-level JVMTI methods deallocate buffers they allocate |
| Errors are explicit | JVMTI methods return `Result`, JNI helpers use `Option`/`Result` |

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
- Need dynamic attach (use `Agent::on_attach` / `Agent_OnAttach`)
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
│   Jvmti      - JVMTI operations (150+ methods)           │
│   JniEnv     - JNI operations (60+ methods)              │
│   LocalRef   - RAII guard, prevented from escaping       │
│   GlobalRef  - RAII guard, releases on drop              │
├─────────────────────────────────────────────────────────┤
│              Class File Parser (classfile)               │
│   ClassFile  - All standard Java 8-27 attributes         │
├─────────────────────────────────────────────────────────┤
│              Convenience Imports (prelude)               │
│   prelude::* - Agent, env, sys, helpers                  │
├─────────────────────────────────────────────────────────┤
│              Raw FFI Bindings (sys module)               │
│   sys::jni   - Complete JNI vtable (236 functions)       │
│   sys::jvmti - Complete JVMTI vtable (156 functions)     │
└─────────────────────────────────────────────────────────┘
```

## Enabling Events

Events require three steps — capabilities, callbacks, then enable:

```rust
use jvmti_bindings::prelude::*;

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
    jvmti_env.enable_event(
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

## Documentation

- [**Your First Production Agent**](docs/FIRST_AGENT.md) — Step-by-step guide with production hardening
- [**Public API Surface**](docs/PUBLIC_API.md) — What is stable and supported
- [**API Stability Checklist**](docs/API_STABILITY.md) — Pre-1.0 stability rules
- [**Contributor Style Guide**](docs/STYLE_GUIDE.md) — Prelude-first and API consistency
- [**Public API Report**](docs/PUBLIC_API_REPORT.md) — Snapshot of the public surface
- [**API Report Script**](scripts/public_api_report.sh) — Regenerate the report with rustdoc JSON
- [**Changelog**](CHANGELOG.md) — Release notes and breaking changes
- [**Comparison With Alternatives**](docs/COMPARISON.md) — Feature parity and positioning
- [**Safety and FFI Checklist**](docs/SAFETY.md) — Safety rules and audit checklist
- [**Pitfalls and Footguns**](docs/PITFALLS.md) — Common JVMTI/JNI traps
- [**Compatibility Matrix**](docs/COMPATIBILITY.md) — JDK 8-27 coverage
- [**Versioning Policy**](docs/VERSIONING.md) — API stability and SemVer plan
- [**API Reference**](https://docs.rs/jvmti-bindings) — Complete API documentation on docs.rs

## License

MIT OR Apache-2.0
