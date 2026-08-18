# JVM TI 3.0 Performance Reference

Date: 2026-08-18
Candidate revision: `46d6a064399259a6c0115f0fc5e90f006fada54e`

## Executive Result

On the measured JDK 21 workload, the complete Rust no-op `MethodEntry`
callback path was statistically indistinguishable from a minimal raw-C JVMTI
callback. The single-thread median difference was `0.230 ns/callback` (`0.3%`)
and the eight-thread aggregate difference was `0.140 ns/callback` (`0.8%`);
the observed ranges overlap in both cases.

Normal callback dispatch also performed no Rust heap allocation, reallocation,
or deallocation across more than 22 million single-thread callbacks and more
than 162 million eight-thread callbacks.

These results establish that the binding's callback trampoline is not the
dominant cost in this workload. They do not make arbitrary profiler callbacks
free: stack walking, symbolization, locks, allocation, queues, and output remain
the agent author's responsibility.

## Single-Thread Callback Delivery

Five-run medians from one non-inlined Java method workload:

| Variant | Median ns/call | Observed range | Median calls/s |
| --- | ---: | ---: | ---: |
| No agent | 1.443 | 1.438-1.462 | 693,085,444 |
| Rust agent loaded, events disabled | 1.447 | 1.444-1.504 | 690,983,385 |
| Raw-C no-op `MethodEntry` | 71.308 | 71.080-72.984 | 14,023,608 |
| Rust no-op `MethodEntry` | 71.538 | 71.316-72.782 | 13,978,633 |
| Rust callback plus relaxed atomic increment | 75.493 | 75.290-78.465 | 13,246,347 |

The approximately 50-fold gap between the Java baseline and either no-op
callback is primarily HotSpot's exact method-entry event machinery. It must not
be presented as Rust overhead. Raw C versus Rust is the relevant implementation
comparison.

## Eight-Thread Callback Delivery

Five-run medians, with elapsed wall time divided by total callbacks across all
workers:

| Variant | Median aggregate ns/call | Observed range | Median aggregate calls/s |
| --- | ---: | ---: | ---: |
| No agent | 0.193 | 0.193-0.195 | 5,170,783,882 |
| Rust agent loaded, events disabled | 0.196 | 0.193-0.200 | 5,097,060,130 |
| Raw-C no-op `MethodEntry` | 17.145 | 16.560-17.206 | 58,324,749 |
| Rust no-op `MethodEntry` | 17.285 | 16.631-17.423 | 57,854,216 |
| Rust callback plus relaxed atomic increment | 17.402 | 16.873-19.641 | 57,465,470 |

These are aggregate throughput figures, not per-callback latency. The Rust
no-op path scaled from about 14.0 million callbacks/s with one worker to about
57.9 million callbacks/s with eight workers on this machine.

## Allocation Proof

The counting allocator starts after agent construction, capability setup,
callback registration, and event enablement, and remains active through entry
into `VMDeath`.

| Workload | Callbacks | Allocations | Reallocations | Deallocations |
| --- | ---: | ---: | ---: | ---: |
| Single worker | 22,301,473 | 0 | 0 | 0 |
| Eight workers | 162,344,903 | 0 | 0 | 0 |

This counts Rust allocator traffic from normal dispatch. It does not measure
allocations internal to HotSpot or arbitrary allocations performed by a user
callback. Panic reporting is not promised to be allocation-free.

## Downstream Dogfood

`bytecode-instrument` was migrated from the 2.x callback surface to this exact
3.0 candidate and exercised as a real consumer. The proof includes:

- all-feature Rust tests and JVM verifier tests;
- Clippy with warnings denied;
- callback-scoped live class, loader, module, and JPMS metadata collection;
- real `ClassFileLoadHook` transformation and JVM-owned transformed-byte
  handoff;
- the production agent template loaded on OpenJDK 8, 11, 17, 21, 25, 27, and
  28, with entry/exit callbacks and application-value checks on every runtime;
- a separate raw benchmark-agent migration, which exposed and documented its
  remaining unsafe JNI/JVMTI assumptions.

This is functional and compatibility evidence, not a new performance
measurement of `bytecode-instrument` itself.

## Measurement Environment

- CPU: AMD Ryzen 9 9950X, 16 cores and 32 hardware threads.
- Kernel: Linux 7.1.5-gentoo-x86_64.
- JVM: OpenJDK 21.0.12+8, 64-bit Server VM.
- Rust: 1.85.0 for the callback reference run.
- CPU policy: performance governor.
- Repetitions: five per variant, with rotated variant order.

## Reproduce And Inspect

```bash
JAVA_HOME=/path/to/jdk scripts/benchmark-callback-dispatch.sh
JAVA_HOME=/path/to/jdk scripts/prove-callback-allocation-free.sh
```

The full methodology, raw ranges, limitations, and ownership audit remain the
canonical detailed records:

- [`CALLBACK_DISPATCH_BENCHMARK_2026-08-17.md`](CALLBACK_DISPATCH_BENCHMARK_2026-08-17.md)
- [`ALLOCATION_AND_OWNERSHIP_AUDIT_2026-08-17.md`](ALLOCATION_AND_OWNERSHIP_AUDIT_2026-08-17.md)
- [`BENCHMARKS.md`](BENCHMARKS.md)
