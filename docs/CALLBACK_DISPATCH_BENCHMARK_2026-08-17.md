# Callback Dispatch Benchmark - 2026-08-17

## Result

On this machine and JVM, the complete Rust no-op callback path was within the
observed run-to-run variation of a raw-C no-op JVMTI callback. The median delta
was 0.230 ns per profiled Java call, or 0.3 percent. The measurement does not
support claiming that the Rust path is meaningfully slower than C.

| Variant | Median ns/call | Range ns/call | Median calls/s |
|---|---:|---:|---:|
| No agent | 1.443 | 1.438-1.462 | 693,085,444 |
| Rust agent, no events | 1.447 | 1.444-1.504 | 690,983,385 |
| Raw-C no-op `MethodEntry` | 71.308 | 71.080-72.984 | 14,023,608 |
| Rust no-op `MethodEntry` | 71.538 | 71.316-72.782 | 13,978,633 |
| Rust relaxed-atomic counter | 75.493 | 75.290-78.465 | 13,246,347 |

Derived observations:

1. Loading an idle Rust agent had no measurable steady-state effect.
2. Raw-C JVMTI delivery added about 69.9 ns over the no-agent workload.
3. Rust callback-context construction, panic containment, global-agent lookup,
   trait dispatch, and an empty handler added a median 0.230 ns over raw C.
4. A shared relaxed `AtomicU64` increment added a median 3.955 ns over the Rust
   no-op path.
5. The counter agent observed 22,297,338 method-entry callbacks per process for
   2 million warm-up and 20 million measured target calls. The remainder came
   from JVM and Java harness methods because notification was globally enabled.

The approximately 50-times baseline slowdown is primarily the cost of enabling
HotSpot's exact method-entry event machinery. It must not be described as Rust
overhead. Exact method callbacks are inherently intrusive; sampling, selective
thread notification, or instrumentation may be more appropriate for a
production profiler.

## Eight-Thread Scaling Check

A second run used eight platform threads, 1 million single-thread warm-up calls,
5 million measured calls per worker, and five fresh JVMs per variant. Here,
`ns/call` is elapsed wall time divided by 40 million aggregate calls; it is not
individual callback latency.

| Variant | Median aggregate ns/call | Range ns/call | Median aggregate calls/s |
|---|---:|---:|---:|
| No agent | 0.193 | 0.193-0.195 | 5,170,783,882 |
| Rust agent, no events | 0.196 | 0.193-0.200 | 5,097,060,130 |
| Raw-C no-op `MethodEntry` | 17.145 | 16.560-17.206 | 58,324,749 |
| Rust no-op `MethodEntry` | 17.285 | 16.631-17.423 | 57,854,216 |
| Rust relaxed-atomic counter | 17.402 | 16.873-19.641 | 57,465,470 |

The Rust no-op median was 0.140 aggregate ns/call, or 0.8 percent, above raw C;
their ranges overlap. The shared relaxed atomic was another 0.117 aggregate
ns/call at the median, also within the observed variation. This test therefore
does not establish a meaningful Rust-dispatch or atomic-contention penalty.

The no-op Rust path increased from 14.0 million calls/s with one worker to 57.9
million calls/s with eight workers, about 4.1-times rather than linear scaling.
Further concurrency profiling should first investigate HotSpot's exact-event
delivery ceiling before changing the Rust trampoline or counter design.

## Environment

1. CPU: AMD Ryzen 9 9950X, 16 cores, 2 threads per core.
2. CPU governor: `performance`.
3. Kernel: Linux 7.1.5-gentoo-x86_64, x86-64.
4. JVM: OpenJDK 21.0.12+8, 64-bit Server VM.
5. Rust: 1.85.0.
6. Warm-up: 2,000,000 target method calls per JVM.
7. Measurement: 20,000,000 target method calls per JVM.
8. Repetitions: five fresh JVMs per variant, with rotated variant order.
9. JVM controls: fixed 256 MiB heap, tiered compilation disabled, batch
   compilation, and explicit non-inlining of the target method.

Command:

```bash
JAVA_HOME=/etc/java-config-2/current-system-vm \
RUSTUP_TOOLCHAIN=1.85.0 \
scripts/benchmark-callback-dispatch.sh
```

## What The Variants Isolate

1. `baseline` establishes Java loop and non-inlined call cost.
2. `rust_idle` detects steady-state cost from loading the Rust shared library
   and running `Agent_OnLoad` without enabling events.
3. `c_noop` measures HotSpot/JVMTI event delivery with the smallest practical C
   callback.
4. `rust_noop` adds the production Rust trampoline and dispatch machinery.
5. `rust_counter` adds representative minimal profiler state mutation.

The harness verifies the same computation checksum for every run and preserves
all raw observations as TSV under `target/callback-dispatch-bench/`.

## Limitations

1. This is one CPU, operating system, and JVM build.
2. The process was not pinned to an isolated core and hardware performance
   counters were not collected.
3. Five repetitions establish a useful engineering baseline, not a formal
   statistical equivalence result.
4. `MethodEntry` is a high-frequency synchronous event. Results do not directly
   predict GC, allocation, monitor, JIT, class-load, or breakpoint callbacks.
5. The no-op comparison excludes stack walking, method metadata lookup,
   symbolization, allocation, locks, queues, output, and aggregation.

The next performance work should therefore benchmark real profiler policies,
especially per-thread buffering and batched aggregation. Optimizing the generic
Rust trampoline before those workloads are measured is unlikely to produce a
meaningful end-to-end gain.
