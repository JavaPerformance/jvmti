# Compatibility Matrix (JDK 8-27)

This crate is verified against OpenJDK headers through JDK 27.

| JDK | JNI Additions | JVMTI Additions | Notes |
|---|---|---|---|
| 8 | Baseline | Baseline callback table through `VMObjectAlloc` | All core JNI/JVMTI functions |
| 9 | GetModule | Module-related functions | JPMS support |
| 11 | - | `SetHeapSamplingInterval`, `SampledObjectAlloc` | Heap sampling support |
| 21 | IsVirtualThread | `VirtualThreadStart`, `VirtualThreadEnd` | Project Loom |
| 24 | GetStringUTFLengthAsLong | - | UTF length as long |
| 25 | - | ClearAllFramePops | Reserved function-table slot 67 repurposed |
| 27 | - | - | Current early-access header verified |

Class file parsing supports all standard attributes through Java 27.

## Event Callback ABI

`jvmtiEventCallbacks` is an append-only, versioned C structure. Reserved slots
are ABI-significant and cannot be omitted even though no event uses them. The
binding models all slots through event 88:

| Runtime generation | Last callback | Native prefix |
|---|---|---:|
| JDK 8-10 | `VMObjectAlloc` (84) | 35 pointer slots |
| JDK 11-20 | `SampledObjectAlloc` (86) | 37 pointer slots |
| JDK 21-27 | `VirtualThreadEnd` (88) | 39 pointer slots |

`Jvmti::set_event_callbacks` supplies the complete current table. JVMTI uses
the explicit byte-size argument for version compatibility; older JVMs copy the
prefix they understand and ignore the newer tail. Never remove or reorder a
reserved field.

The Rust ABI tests validate all event IDs, offsets, reserved slots, and prefix
sizes without requiring a local JDK:

```bash
cargo test --test jvmti_event_abi
```

The live proof builds a real agent, enables a callback before the reserved gap
(`MethodEntry`) and callbacks after it (`GarbageCollectionStart` and
`GarbageCollectionFinish`), then runs the workload under every installed JDK:

```bash
scripts/prove-event-callback-matrix.sh
```

Pass explicit JDK homes when they are not installed under `/opt`:

```bash
scripts/prove-event-callback-matrix.sh /path/to/jdk8 /path/to/jdk21 /path/to/jdk27
```
