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

## Upgrading From 2.x

> **Important:** Version 3.0 is intentionally source-breaking from every 2.x
> release. Version 2.4.0 was not published: the ABI, callback, ownership, and
> lifecycle corrections originally planned for 2.4 required a major-version
> release under semantic versioning and therefore became 3.0.0.

Do not update an existing agent by changing only the dependency version.
Version 3.0 replaces the parallel `*_with_jvmti` callback surface with canonical
typed callbacks, changes lifecycle contexts and several ownership contracts,
corrects public raw ABI declarations, and raises the minimum Rust version to
1.85. Follow the complete [2.x to 3.0 migration guide](docs/MIGRATING_2_TO_3.md)
before upgrading production agents.

## Scope boundary

This crate is a **generic JNI/JVMTI binding and agent framework** (published on
crates.io). Bytecode instrumentation engines, spec transforms, stackmap-aware
BCI, and related policy live in the separate `bytecode-instrument` project. Do
not add that instrumentation technology here unless there is an explicit
decision to merge or port it. Agents can depend on both crates independently.

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
3. Class file parsing with all standard Java 8-27 attributes and opaque preservation of unknown attributes
4. A tiny but explicit public surface (`env`, `sys`, `classfile`, `prelude`)
5. Safety guidance, pitfalls, and compatibility documentation
6. Examples that mirror real JVMTI tooling patterns

## Why Rust for JVMTI?

C++ is the traditional choice, but Rust offers compelling advantages:

- **Memory safety without GC** — JVMTI agents run inside the JVM process; a segfault kills the application
- **Fearless concurrency** — JVMTI callbacks fire from multiple threads simultaneously
- **Zero-cost abstractions** — RAII guards and Result types add safety without runtime overhead
- **No third-party crates** — Normal builds, optional features, tests, tools, and benchmarks use only this crate and the Rust standard library
- **Modern tooling** — Cargo, docs.rs, and crates.io beat Makefiles and manual distribution

Java agents (`java.lang.instrument`) are simpler but can't access low-level features like heap iteration, breakpoints, or raw bytecode hooks.

## Design Goals

| Goal | How |
|------|-----|
| **Explicit safety model** | Unsafe operations centralized; APIs return `Result` |
| **Complete surface** | Complete JDK 28 JNI and JVM TI tables, mapped to Rust types |
| **Agent-first ergonomics** | Structured callbacks, capability management, RAII resources |
| **No hidden dependencies** | Consumers need no bindgen or build-time JVM, no global allocator is installed, and no third-party crate appears in any Cargo dependency graph |
| **Long-term compatibility** | Source-ABI verified against pinned OpenJDK 8-28 revisions; live callback-tested on installed runtimes through JDK 28 preview |

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

## Toolchain Contract

Version 3.0 requires Rust 1.85 or newer and uses Edition 2024. This is a source
toolchain requirement only: the generated native agent uses runtime-gated table
prefixes source-verified for JDK 8-28. Live callback delivery is verified on
installed runtimes through JDK 28. The JDK 28 run used preview build `28+7`, so
preview value-object behavior is not described as final Java SE 28 behavior.

## Raw FFI Access

If you need raw JNI/JVMTI functions, use:
1. `jvmti_bindings::sys::jni` and `jvmti_bindings::sys::jvmti` for raw types and vtables.
2. `JniEnv::raw()` and `Jvmti::raw()` to access the underlying raw pointers.
3. `CallbackContext` for the exact callback-scoped `jvmtiEnv*` and optional thread-local `JNIEnv*` supplied by the JVM.

`JniEnv` covers every fixed-signature JNI operation, including all typed
`jvalue` (`A`) call families. The C variadic and `va_list` slots remain raw-only
because stable Rust cannot construct or portably forward arbitrary C variadic
arguments; prefer the corresponding `A` operation.

## Attach and Threading Rules

1. `Agent_OnAttach` is supported via the `export_agent!` macro and `Agent::on_attach`; repeated attach reuses the same process-global agent and may invoke `on_attach` concurrently.
2. `JNIEnv` is thread-local and must only be used on its originating thread.
3. `GlobalRef` cleanup attaches to the JVM when needed, but you should still manage lifetimes explicitly.
4. Callback payloads are complete and canonical; use `context.jvmti()` and `context.jni()` instead of rediscovering environments from `JavaVM`.

## ClassLoader and JPMS Helpers

`JniEnv` includes neutral helpers for modern agent work:

- `define_class` for target-loader helper injection.
- `class_loader_parent` and `system_class_loader` for loader graph discovery.
- `module_name`, `module_packages`, and `module_class_loader` for JPMS metadata.
- `module_can_read`, `module_is_exported_to`, and `module_is_open_to` for visibility preflight.

These are generic JNI conveniences. Higher-level instrumentation policy,
helper deployment, and compatibility planning remain outside this crate.

## Compatibility

See `docs/COMPATIBILITY.md` for the full JDK 8-28 matrix and the JDK 29 acceptance gate.

The event callback ABI has dedicated offset tests and a live multi-JDK agent
proof. Run `cargo test --test jvmti_event_abi` for the portable checks or
`scripts/prove-event-callback-matrix.sh` to exercise callback delivery on the
installed JVMs.

## Advanced Helpers

Feature-gated helpers live under `advanced`:
1. `heap-graph` for heap tagging and reference edge extraction.

Enable with:

```toml
[dependencies]
jvmti-bindings = { version = "3", features = ["heap-graph"] }
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
jvmti-bindings = "3"
```

### 3. Implement your agent

```rust
use jvmti_bindings::prelude::*;

#[derive(Default)]
struct MyAgent;

impl Agent for MyAgent {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        println!("[MyAgent] Loaded with options: {:?}", context.options_lossy());
        jni::JNI_OK
    }

    fn vm_init(&self, _context: CallbackContext<'_>, _event: ThreadEvent) {
        println!("[MyAgent] VM initialized");
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
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

### Dynamic Attach (Optional)

If you want to attach to an already running JVM, implement `Agent::on_attach` and load the agent with the JVM Attach API:

```rust,ignore
use jvmti_bindings::prelude::*;

#[derive(Default)]
struct AttachLogger;

impl Agent for AttachLogger {
    fn on_attach(&self, context: AgentLoadContext<'_>) -> jni::jint {
        println!("[AttachLogger] attached with options: {:?}", context.options_lossy());
        let Ok(_jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        jni::JNI_OK
    }
}

export_agent!(AttachLogger);
```

Attach it with `jcmd` (example):

```bash
jcmd <pid> JVMTI.agent_load /abs/path/to/libattach_logger.so "opt1=val1"
```

`JVMTI.agent_load` expects an **absolute** path to the native agent and an optional option string.

## Class File Parsing

This crate now includes a zero-dependency class file parser that understands all standard attributes from Java 8 through Java 27. Use it inside `ClassFileLoadHook` to inspect or transform class metadata.

`ClassFile::parse` applies conservative input-size, cumulative allocation,
annotation-nesting, and recursive-attribute limits for untrusted input. Use
`ClassFile::parse_with_limits` and `ClassFileParseLimits` only when a tool has a
justified need for different bounds.

```rust
use jvmti_bindings::classfile::ClassFile;

fn parse_class(bytes: &[u8]) {
    let Ok(classfile) = ClassFile::parse(bytes) else {
        return;
    };
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
    let Ok(classfile) = ClassFile::parse(bytes) else {
        return;
    };
    walk_attributes(&classfile.attributes);
    for field in &classfile.fields {
        walk_attributes(&field.attributes);
    }
    for method in &classfile.methods {
        walk_attributes(&method.attributes);
    }
}
```

## Embedding A JVM (Optional)

If you want to **embed** a JVM inside a Rust process (not just build an agent), enable the `embed` feature and use `JavaVmBuilder`:

```rust,ignore
use jvmti_bindings::prelude::*;

let builder = JavaVmBuilder::default()
    .option("-Xms64m")?
    .option("-Xmx256m")?
    .option("-Djava.class.path=./myapp.jar")?;

let vm = builder.create()?; // uses JAVA_HOME or JVM_LIB_PATH
let env = unsafe { vm.creator_env() }; // only valid on the creating thread

// ... call JNI through env ...

std::thread::scope(|s| {
    s.spawn(|| {
        let result = vm.with_attached_current_thread(|env| {
            let _string = env.find_class("java/lang/String");
        });
        if let Err(code) = result {
            eprintln!("AttachCurrentThread failed: {}", jni::result_name(code));
        }
    });
});

vm.destroy()?;
```

The `embed` feature uses the crate's small platform loader (`dlopen`/`dlsym` on
Unix and `LoadLibraryW`/`GetProcAddress` on Windows). It adds no crate
dependency. See `docs/EMBEDDING.md` and `examples/embed.rs` for details.

## Allocation-Free JNI Names

The familiar `&str` JNI helpers remain convenient adapters. Hot paths can avoid
repeated `CString` allocation by passing a prevalidated `&CStr`:

```rust,ignore
let string_class = jni.find_class_cstr(c"java/lang/String")?;
let length = unsafe { jni.get_method_id_cstr(string_class, c"length", c"()I")? };
```

Use the `*_cstr` methods for fixed class names, member names, and descriptors in
high-frequency callbacks. The `&str` variants retain the same behavior and are
appropriate when names are dynamic or the call is not performance-sensitive.
Pre-encoded UTF-16 can similarly use `new_string_utf16` without a temporary
Rust buffer.

## JNI Paired Operations

JNI functions that lend native array or string storage return allocation-free
RAII guards instead of an unmatched pointer/release pair:

```rust,ignore
let mut values = unsafe { jni.get_int_array_elements(array) }
    .expect("JVM returned array elements");
values[0] = 42;
// Normal drop copies back when required and releases exactly once.
```

`PrimitiveArrayElements`, `PrimitiveArrayCritical`, and `StringCritical`
provide explicit `close()` methods and release on drop. `abort()` requests that
copied array storage not be written back, but cannot undo writes when the JVM
returned pinned storage. Do not block or make arbitrary JNI calls while a
critical guard is live.

`push_local_frame` returns a `LocalFrame` that pops on drop and can explicitly
promote one reference to the previous frame. `monitor_enter` returns a
`JavaMonitorGuard` that exits on drop. Their `*_raw` methods remain unsafe for
integrations that deliberately manage the matching operation themselves.

JNI, JVM TI, and class-file `CONSTANT_Utf8` strings use Java Modified UTF-8,
not ordinary UTF-8. The `mutf8` module provides strict, lossy, and exact UTF-16
conversions. Use exact UTF-16 APIs when unpaired Java surrogates must survive a
round trip.

## Examples

Included examples (build as `cdylib` agents):
1. `examples/minimal.rs`
2. `examples/class_logger.rs`
3. `examples/profiler.rs`
4. `examples/tracer.rs`
5. `examples/heap_sampler.rs`
6. `examples/attach_logger.rs` (dynamic attach via `Agent_OnAttach`)

Embedding example (binary):
`examples/embed.rs` (run with `cargo run --example embed --features embed`)

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
- Register callbacks or enable events
- Prevent JVM crashes from invalid JVMTI usage

Every event has one canonical callback. `CallbackContext` carries the exact
callback-scoped JVM TI environment and, only where the JVM supplies one, the
thread-local JNI environment. The event value carries the complete native
payload. For example:

```rust,ignore
fn compiled_method_load(
    &self,
    context: CallbackContext<'_>,
    event: CompiledMethodLoadEvent<'_>,
) {
    if let Ok((name, signature, _generic)) = context.jvmti().get_method_name(event.method()) {
        let lines = context.jvmti().get_line_number_table(event.method()).unwrap_or_default();
        println!("compiled {name}{signature}: {} line entries", lines.len());
    }
}
```

`get_line_number_table` additionally requires the `can_get_line_numbers`
capability to be requested before the VM enters the live phase.

The goal is clarity, not magic.

## Safety Model

This crate enforces the following invariants:

| Invariant | Enforcement |
|-----------|-------------|
| `JNIEnv` is thread-local | `JniEnv` wrapper is not `Send` |
| Owned local refs are scoped | `LocalRef<'a>` is tied to `JniEnv`; raw JNI/JVM TI object returns still require explicit local-reference management |
| Global refs are freed | `GlobalRef` and `WeakGlobalRef` release on `Drop`; fallible construction and `close()` expose lifecycle failures |
| JVMTI memory properly freed | `JvmtiAllocation` deallocates on drop; raw ownership transfer is explicit and unsafe |
| Raw monitors are released | `RawMonitor` destroys on drop; entered `RawMonitorGuard` exits on drop; explicit failure retains ownership for one fallback attempt |
| JNI paired operations are owned | Array/critical leases, local frames, and entered Java monitors close on drop; raw unmatched operations remain explicit unsafe escape hatches |
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
- Need one crate with source-verified JDK 8-28 native layouts and runtime gates
- Comfortable reading JVMTI docs for advanced use cases

**Probably not, if you:**
- Only need basic JNI calls (consider the `jni` crate)
- Are uncomfortable debugging native JVM crashes
- Need only pure-Java instrumentation (`java.lang.instrument` may be simpler)
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
│   JniEnv     - JNI operations plus ClassLoader/JPMS      │
│   LocalRef   - RAII guard, prevented from escaping       │
│   GlobalRef  - RAII guard, releases on drop              │
│   JNI leases - Array/string native storage RAII guards    │
├─────────────────────────────────────────────────────────┤
│              Class File Parser (classfile)               │
│   ClassFile  - All standard Java 8-27 attributes         │
├─────────────────────────────────────────────────────────┤
│              Convenience Imports (prelude)               │
│   prelude::* - Agent, env, sys, helpers                  │
├─────────────────────────────────────────────────────────┤
│              Raw FFI Bindings (sys module)               │
│   sys::jni   - Complete JDK 28 JNI vtable                │
│   sys::jvmti - Complete JVMTI vtable (156 functions)     │
└─────────────────────────────────────────────────────────┘
```

## Enabling Events

Events require three steps — capabilities, callbacks, then enable:

```rust
use jvmti_bindings::prelude::*;

fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
    let Ok(jvmti_env) = context.vm().jvmti() else {
        return jni::JNI_ERR;
    };

    // Requests capabilities, wires callbacks, and enables ClassFileLoadHook.
    if jvmti_env.configure_class_file_load_hook_agent().is_err() {
        return jni::JNI_ERR;
    }

    jni::JNI_OK
}
```

Lower-level helpers are available when you need explicit control:

```rust
let caps = jvmti::jvmtiCapabilities::for_method_trace();
jvmti_env.add_capabilities(&caps)?;
jvmti_env.set_default_agent_callbacks()?;
jvmti_env.enable_method_entry_exit_events()?;
```

For diagnostics:

```rust
eprintln!("jni={}", describe_jni_result(jni::JNI_EDETACHED));
eprintln!("jvmti={}", jvmti::error_name(jvmti::jvmtiError::MUST_POSSESS_CAPABILITY));
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

| JDK range | Status | Notable additions |
|-----|--------|-------------------|
| 8 | ✅ Tested | JVM TI 1.2.1 baseline |
| 9-18 | ✅ Every release ABI-verified | Modules, heap sampling, errors, and semantic/source-only changes |
| 19-23 | ✅ Every release ABI-verified | Preview then final virtual-thread JNI/JVM TI surface |
| 24-27 | ✅ Every release ABI-verified | Long modified-UTF-8 length, native policy changes, `ClearAllFramePops` |
| 28 | ✅ Source ABI and live preview-runtime verified | Preview value-object identity/capability surface |

The latest Rust layout is compared with C headers for every JDK feature release
from 8 through 28. The release ledger separately records table prefixes,
interface revisions, callbacks, capabilities, events, errors, semantic-only
changes, source-only changes, and operational policy. Runtime gates prevent
access to table tails, reclaimed slots, and capability bits on older JVMs.

## Project Status

| Aspect | Status |
|--------|--------|
| API stability | 3.0 candidate; subsequent changes follow SemVer |
| JVMTI coverage | 156/156 (100%) |
| JNI coverage | Complete through pinned JDK 28 source (237 table slots); live preview-runtime evidence through JDK 28 |
| Dependencies | Zero third-party crates across all features and development targets |
| Rust toolchain | Rust 1.85+; Edition 2024 |
| Testing | Classfile parser, doctests, all-feature builds, example agents |

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
- [**2.x to 3.0 Migration**](docs/MIGRATING_2_TO_3.md) — complete callback, ownership, unsafe-wrapper, and raw-ABI migration
- [**Contributor Style Guide**](docs/STYLE_GUIDE.md) — Prelude-first and API consistency
- [**Public API Report**](docs/PUBLIC_API_REPORT.md) — Snapshot of the public surface
- [**API Report Script**](scripts/public_api_report.sh) — Regenerate the report with rustdoc JSON
- [**Changelog**](CHANGELOG.md) — Release notes and breaking changes
- [**Comparison With Alternatives**](docs/COMPARISON.md) — Feature parity and positioning
- [**Benchmarks**](docs/BENCHMARKS.md) — Dependency-free parser, callback-dispatch, and allocation measurements
- [**3.0 Performance Reference**](docs/PERFORMANCE_REFERENCE_3_0.md) — Concise raw-C comparison, callback throughput, allocation proof, and downstream dogfood status
- [**Embedding A JVM**](docs/EMBEDDING.md) — Start a JVM from Rust and attach threads
- [**Dynamic Attach**](docs/ATTACH.md) — Agent_OnAttach example and notes
- [**Safety and FFI Checklist**](docs/SAFETY.md) — Safety rules and audit checklist
- [**Independent Unsafe/FFI Review**](docs/UNSAFE_FFI_REVIEW.md) — reviewer scope and acceptance record
- [**Pitfalls and Footguns**](docs/PITFALLS.md) — Common JVMTI/JNI traps
- [**Compatibility Matrix**](docs/COMPATIBILITY.md) — JDK 8-28 coverage and JDK 29 gate
- [**JDK 28 Live Proof**](docs/JDK_28_LIVE_PROOF_2026-08-18.md) — Exact preview runtime identity, live semantic matrix, and claim boundary
- [**Versioning Policy**](docs/VERSIONING.md) — API stability and SemVer plan
- [**Release Procedure**](docs/RELEASING.md) — clean candidate, attestations, SBOM, and 3.x compatibility
- [**Definitive 3.0 Release Gates**](docs/DEFINITIVE_3_0_RELEASE_GATES.md) — mechanical ABI, lifecycle, ownership, and publication criteria
- [**API Reference**](https://docs.rs/jvmti-bindings) — Complete API documentation on docs.rs

## License

MIT OR Apache-2.0
