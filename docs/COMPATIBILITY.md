# Compatibility Matrix (JDK 8-28 Source ABI And Live Runtime)

## Rust Toolchain Versus JVM Runtime

Version 3.0 requires Rust 1.85 or newer and uses Edition 2024. That requirement
applies when compiling the Rust crate; it does not raise the minimum Java
runtime. One resulting native agent uses the audited JDK 8-28 table prefixes,
subject to the runtime feature gates described below. Live callback delivery
has been exercised on installed JDK 8, 11, 17, 21, 25, 27, and 28 runtimes.
JDK 28 evidence uses a preview `28+7` runtime; preview value-object behavior is
not described as final Java SE 28 behavior.

The repository tests the declared Rust 1.85 floor and current stable Rust
separately. Raising the MSRV is a deliberate compatibility change and must be
recorded in the changelog and migration guide.

Version 3.0 has an explicit compatibility profile for every Java feature
release from JDK 8 through JDK 28. The proof is not based on selected LTS
releases or on the assumption that an unchanged structure size means unchanged
behavior. For every release, the profile records:

- the exact JNI interface revision and function-table prefix;
- the exact JVM TI interface revision and function-table prefix;
- the exact event-callback prefix;
- newly consumed capability bits, events, functions, and errors;
- semantic changes whose C layout is unchanged;
- source-only native changes whose binary layout is unchanged; and
- operational policy changes affecting load, attach, native access, AOT, or
  JNI behavior.

The scope is every change relevant to a native JNI/JVM TI agent or embedded
VM, not unrelated Java language or library additions.

## Audited Release Ledger

The slot counts below are pointer slots, not bytes. `JVM TI 1.2.1` is the exact
JDK 8 `JVMTI_VERSION`; JDK 10 and 12 intentionally reuse the preceding JVM TI
interface revision. Consequently, a JVM TI version identifies an interface
milestone, not always the exact Java feature release.

| JDK | JNI revision / slots | JVM TI revision / slots | Callback slots | Native-agent-affecting delta first present in this release |
|---:|---|---|---:|---|
| 8 | 1.8 / 233 | 1.2.1 / 155 | 35 | Supported baseline |
| 9 | 9 / 234 | 9 / 155 | 35 | JNI `GetModule`; JVM TI module functions consume reserved slots; early-event capabilities; primordial/start/current-thread and modifiable-class semantics |
| 10 | 10 / 234 | 9 / 155 | 35 | JNI interface revision only |
| 11 | 10 / 234 | 11 / 156 | 37 | Heap-sampling function, capability, and event; error 72; immutable nest attributes |
| 12 | 10 / 234 | 11 / 156 | 37 | No JNI/JVM TI contract delta |
| 13 | 10 / 234 | 13 / 156 | 37 | Redefine-any-class and `PopFrame` semantics |
| 14 | 10 / 234 | 14 / 156 | 37 | Record attribute immutable during redefine/retransform |
| 15 | 10 / 234 | 15 / 156 | 37 | Permitted-subclasses attribute immutable during redefine/retransform |
| 16 | 10 / 234 | 16 / 156 | 37 | JVM TI C structure tags renamed without an ABI change |
| 17 | 10 / 234 | 17 / 156 | 37 | Failed attach may skip unload; JVM TI 1.0 heap functions deprecated |
| 18 | 10 / 234 | 18 / 156 | 37 | JVM TI interface revision only |
| 19 | 19 / 235 | 19 / 156 | 39 | Preview virtual-thread JNI/JVM TI functions, capability, events, and error 73 |
| 20 | 20 / 235 | 20 / 156 | 39 | JNI and JVM TI interface revisions only |
| 21 | 21 / 235 | 21 / 156 | 39 | Virtual threads final; live/dynamic agent startup warnings |
| 22 | 21 / 235 | 22 / 156 | 39 | JVM TI interface revision only |
| 23 | 21 / 235 | 23 / 156 | 39 | JVM TI interface revision only |
| 24 | 24 / 236 | 24 / 156 | 39 | JNI long modified-UTF-8 length; native-access warnings; AOT-cache/agent constraints |
| 25 | 24 / 236 | 25 / 156 | 39 | `ClearAllFramePops` consumes reserved JVM TI slot 67 |
| 26 | 24 / 236 | 26 / 156 | 39 | JNI final-field mutation diagnostics and undefined-behavior warning |
| 27 | 24 / 236 | 27 / 156 | 39 | JVM TI interface revision only |
| 28 | 28 / 237 | 28 / 156 | 39 | Preview JNI `HasIdentity`; value-object capability and changed allocation, tag, free, monitor, local, modifier, and early-return semantics |

This ledger is represented by `version::RELEASE_PROFILES`; callers can inspect
one profile with `release_profile(jdk)` and one adjacent transition with
`release_delta(jdk)`.

## Why One Latest Layout Is Safe

The crate declares the latest known raw structures so one binary can use newer
features. It does **not** construct a Rust reference to an older VM's shorter
function table. JNI and JVM TI calls project the requested field from the raw
table pointer and read only that field-sized slot. A version gate runs before
any appended or reclaimed slot is projected.

`Jvmti::get_jni_function_table` returns an opaque owning allocation rather than
pretending that an older, shorter allocation is a complete JDK 28 table. Its
`known_byte_len` reports the audited prefix when the interface milestone is
known. Event registration similarly passes the exact callback prefix expected
by the runtime rather than always passing the JDK 28 structure size.

There is therefore no family of heap-layout structures selected at runtime:
the audited heap structures did not change layout across JDK 8-28. What did
change is represented where it belongs: table prefixes, capability/event
gates, nullable payloads, and release-specific semantic documentation.

## Different Changes Need Different Guards

| Change class | Examples | 3.0 guard or representation |
|---|---|---|
| Stable ABI correction | Timer, stack, heap-reference, extension structures | Exact C size, alignment, offset, and signature probes |
| Appended JNI table tail | `GetModule`, `IsVirtualThread`, long UTF length, `HasIdentity` | Negotiate JNI version before a prefix-safe slot read |
| Reclaimed JVM TI slot | Sampling interval and `ClearAllFramePops` | Negotiate JVM TI interface milestone before a prefix-safe slot read |
| Reserved capability consumed | Early events, sampling, virtual threads, value objects | Reject before add/relinquish-capabilities reaches the VM |
| Growing callback table | Sampling and virtual-thread events | Register the runtime-specific prefix and preserve reserved slots |
| Growing event/error domain | Events 86-88 and errors 72-73 | Open numeric domain plus release gate for known additions |
| Semantic evolution | Redefinition restrictions and value-object behavior | Explicit release ledger, honest nullable payloads, method documentation |
| Source-only evolution | JDK 16 C structure-tag rename | Recorded but no artificial Rust ABI split |
| Runtime policy | Dynamic attach, native access, AOT cache, final fields | Deployment documentation and diagnostics; never inferred from table size |

## Event Callback ABI

`jvmtiEventCallbacks` is a version-growing C structure. Reserved slots are
ABI-significant and cannot be omitted.

| Runtime generation | Last callback | Native prefix |
|---|---|---:|
| JDK 8-10 | `VMObjectAlloc` (84) | 35 pointer slots |
| JDK 11-18 | `SampledObjectAlloc` (86) | 37 pointer slots |
| JDK 19-28 | `VirtualThreadEnd` (88) | 39 pointer slots |

All 34 non-reserved callbacks are invoked by the Rust sentinel suite. It
verifies every payload field, the exact callback JVM TI environment, whether
JNI is present for the phase, mutable callback outputs, and panic containment.

The pinned current-source gate also assigns every native field to the Rust
field type at compile time: 237 JNI native-table slots, 8 JNI invocation-table
slots, 156 JVM TI function-table slots, and 39 JVM TI callback-table slots.
That is 440 exact signatures. A separate gate checks 31 public native records
and 562 field offsets in addition to size, alignment, record kind, and order.

The ABI matrix includes Linux x86-64 and AArch64. A host C-to-Rust-to-C proof
for JNI `...V` calls verifies the platform `va_list` calling convention rather
than assuming it is pointer-shaped. Capability tests also model both little-
and big-endian C bitfield numbering and preserve unknown future bits.

## Exact Release Versus Interface Milestone

`GetVersionNumber` can establish whether a JVM TI interface feature is safe to
use. It cannot always identify the exact Java release: JDK 10 reports JVM TI 9
and JDK 12 reports JVM TI 11. `CallbackContext::jvmti_interface_feature` and
`jvmti_interface_support` are named accordingly. Exact release policy must come
from an explicit deployment/runtime version source rather than a guessed
conversion.

## JDK 29 Policy

As of 2026-08-18 there is no separately identifiable OpenJDK 29 project,
branch, tag, or header set to verify. JDK 29 support is not inferred from JDK
28 source. Before claiming JDK 29 support:

1. Pin an official JDK 29 source revision and record it.
2. Generate `jni.h` and `jvmti.h` from that revision.
3. Diff JNI headers and JVM TI XML against JDK 28.
4. Add every structural, semantic, source, and policy delta to the release ledger.
5. Run the C/Rust ABI probe, callback agent, policy tests, and external-consumer proof.

## Reproducing the Proof

```bash
# Exact C/Rust proof against every pinned JDK 8-28 header generation.
scripts/check-jdk-abi.sh --all-releases

# Complete latest-source signature, layout, constant, inventory, and bit proof.
scripts/check-pinned-jdk-abi.sh 28

# Portable feature, ownership, layout, and callback tests.
cargo test --test abi_conformance --test jvmti_event_abi \
  --test callback_fidelity --test version_gating --test ownership

# Real callback delivery on installed JVMs.
scripts/prove-event-callback-matrix.sh
scripts/prove-mutf8-live.sh
scripts/prove-repeated-attach-live.sh
scripts/prove-heap-graph-live.sh
scripts/prove-callback-allocation-free.sh
```

The exact JDK 28 runtime identity, checksums, commands, results, and claim
boundary are recorded in
[`JDK_28_LIVE_PROOF_2026-08-18.md`](JDK_28_LIVE_PROOF_2026-08-18.md).

The external-header test is opt-in so crates.io consumers do not need a local
JDK or C compiler merely to build the crate. Repository conformance CI uses an
external `bindgen` executable as an independent oracle; bindgen is not a Cargo
dependency and is not needed by crate consumers.
