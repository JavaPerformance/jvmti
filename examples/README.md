# JVMTI Bindings Examples

This directory contains small, compilable programs that each demonstrate one
JVMTI, JNI, class-file, or embedding concern. They use only public
`jvmti-bindings` APIs and intentionally do not depend on external
instrumentation projects.

## Build And Run An Agent

Build one agent as a shared library:

```bash
cargo build --release --example thread_lifecycle
```

On Linux, load it at JVM startup:

```bash
java -agentpath:./target/release/examples/libthread_lifecycle.so MyApp
```

macOS uses `libthread_lifecycle.dylib`; Windows uses
`thread_lifecycle.dll`. Agent options follow the library path after `=`:

```bash
java -agentpath:./target/release/examples/libfield_watch.so=class=Lcom/example/State\;,field=value MyApp
```

Shells treat `;` specially, so quote or escape option strings when needed.
For a real agent project, copy the relevant source into a library crate with:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
jvmti-bindings = "3"
```

## Start Here

| Example | Purpose |
|---|---|
| `minimal` | Smallest load, VM event, and unload agent |
| `vm_lifecycle` | Distinguish VM start, initialization, and death |
| `attach_logger` | Handle `Agent_OnAttach` options |
| `class_logger` | Observe class bytes and names |
| `method_counter` | Configure method entry/exit and count callbacks |
| `profiler` | Minimal method-entry profiler skeleton |
| `tracer` | Minimal class-load tracer skeleton |

## Threads And Synchronization

| Example | Purpose | Notes |
|---|---|---|
| `thread_lifecycle` | Count platform-thread starts and ends | Allocation-free hot callbacks |
| `virtual_thread_lifecycle` | Count virtual-thread starts and ends | Requires a runtime exposing JVMTI virtual-thread support |
| `thread_inventory` | Count threads at VM initialization | Releases returned JNI local references |
| `monitor_contention` | Count waits, timeouts, and contended monitor acquisition | Requests monitor-event capability |

## Exceptions, Methods, And Fields

| Example | Purpose | Notes |
|---|---|---|
| `exception_tracker` | Count thrown and caught exceptions | High-volume event family |
| `method_exit_values` | Separate normal returns from exception pops | Does not guess the type of raw `jvalue` |
| `field_watch` | Install read/write watchpoints on one field | Options: `class=...`, `field=...` |
| `breakpoint` | Break at the first location of one method | Options: `class=...`, `method=...` |

Watchpoints and breakpoints resolve their target during `ClassPrepare`, where
name lookup is safe and bounded. A production attach-time agent must also
handle a target class that was loaded before attachment.

## Memory, GC, And Failure Signals

| Example | Purpose | Notes |
|---|---|---|
| `heap_sampler` | Sample allocations at a configured byte interval | Lower overhead than every-allocation events |
| `allocation_tracker` | Count every reported VM allocation and byte size | Intentionally expensive |
| `gc_observer` | Count GC starts and finishes | No JNI calls from GC callbacks |
| `resource_exhaustion` | Preserve count and flags for later reporting | No allocation or I/O in the exhaustion callback |
| `data_dump_request` | Receive JVM data-dump requests | Real agents should hand work to an agent-owned worker |

## Native And JIT Activity

| Example | Purpose | Notes |
|---|---|---|
| `native_method_bind` | Observe native binding decisions | Does not replace function addresses |
| `jit_code_events` | Count compiled loads, unloads, generated blocks, and bytes | Preserves typed JIT callback payloads |
| `event_abi_smoke` | Exercise callbacks around reserved table slots | Release-gate support example |

## JNI, Class Files, And Runtime Metadata

| Example | Purpose | Notes |
|---|---|---|
| `jni_string_roundtrip` | Use callback-scoped JNI and an RAII local frame | Includes NUL and Unicode MUTF-8 content |
| `modified_utf8_roundtrip` | Encode/decode Java Modified UTF-8 without a JVM | Run with `cargo run --example modified_utf8_roundtrip` |
| `classfile_inspector` | Parse selected class bytes without transforming them | Option: `prefix=com/example/` |
| `class_inventory` | Count loaded classes at VM initialization | Releases returned JNI local references |
| `version_support` | Print the verified JDK range and release profile | Run with `cargo run --example version_support` |
| `embed` | Start a JVM and call Java from Rust | Run with `cargo run --example embed --features embed` |

## Minecraft-Oriented Templates

These examples are deliberately standalone and mapping-agnostic. They do not
contain or derive from any private instrumentor or RASP implementation.

| Example | Purpose | Options |
|---|---|---|
| `minecraft_class_activity` | Count class files and bytes under a package prefix | `prefix=net/minecraft/` |
| `minecraft_tick_breakpoint` | Count calls to a chosen server tick method | `class=L...;,method=...` |
| `minecraft_bullet_time` | Hold F8 and scroll down to slow a chosen client tick or up to restore speed | `tick_class=...`, `scroll_class=...`, `keyboard_class=...`, `activation_key=297`, `step_ms=10`, `max_delay_ms=250` |

Minecraft class and method names vary by release, loader, and mapping set. Pass
the names for the exact runtime being observed. Breakpoints are useful for a
small diagnostic probe; production profiling usually needs sampling or narrow
instrumentation to avoid event overhead.

`minecraft_bullet_time` observes Java-side keyboard and scroll callbacks
supplied by the game/LWJGL and is therefore independent of the host window
system. By default, only F8 (`GLFW_KEY_F8`, value `297`) arms the mouse wheel.
Armed wheel input is consumed before normal hotbar handling. Its default
Mojang-mapped names are illustrative. The configured callback signatures and
local slots must match the exact game version and mapping set. It deliberately
sleeps on the selected tick thread, making it a toy and diagnostic demonstration
rather than a production agent.

Build it and load it when the client JVM starts:

```bash
cargo build --release --example minecraft_bullet_time
java -agentpath:/absolute/path/to/libminecraft_bullet_time.so [launcher options]
```

Use `libminecraft_bullet_time.dylib` on macOS and
`minecraft_bullet_time.dll` on Windows. Supply comma-separated agent options
after `=` when the runtime uses different mappings or local-variable slots:

```text
-agentpath:/path/to/agent=keyboard_method=keyPress,activation_key=297,step_ms=5,max_delay_ms=200
```

At runtime, hold F8 while moving the wheel. Scrolling down increases the delay;
scrolling up decreases it. An unarmed wheel event is not modified. If any
configured method or local-variable slot does not match, the VM-death summary
reports installation, read, and consumption counters for diagnosis.

On Linux or macOS, the repository's deterministic Java fixture exercises those
semantics end to end:

```bash
scripts/prove-minecraft-bullet-time-live.sh
```

## Callback Performance Proofs

| Example | Purpose |
|---|---|
| `callback_idle_bench` | Agent-loaded baseline without enabled hot callbacks |
| `callback_noop_bench` | No-op callback-dispatch cost |
| `callback_counter_bench` | Relaxed-atomic counter callback cost |
| `callback_allocation_audit` | Prove the selected callback path does not allocate |

Use the repository benchmark and release scripts rather than interpreting one
ad hoc JVM run as a stable performance result.

## Safety Rules

1. Treat every raw JNI/JVMTI handle as callback- and VM-scoped unless the JVM
   specification explicitly grants a longer lifetime.
2. Do not retain a callback `JNIEnv`; it belongs to the current JVM thread.
3. Request capabilities before enabling dependent events.
4. Keep high-frequency callbacks bounded and preferably allocation-free.
5. Do not decode a `jvalue` union without the corresponding Java descriptor.
6. Avoid JNI and non-GC-safe JVMTI operations in GC/object-free callbacks.
7. Match every owned JVMTI allocation, JNI reference, monitor, and embedded VM
   attachment with its documented release operation.
