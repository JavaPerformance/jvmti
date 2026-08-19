# Benchmarks

The benchmark targets use only the Rust standard library. They deliberately do
not depend on Criterion, archive crates, plotting tools, or a global allocator.
This keeps the measured parser path close to the code shipped to users and
preserves the repository's zero-third-party-crate contract.

Benchmark results are machine-specific. Record the CPU, operating system, Rust
version, power policy, and command line whenever publishing a number.

## Classfile Parser Microbenchmark

Run the fixed minimal-class parser benchmark with an optimized build:

```bash
cargo bench --bench classfile_parse
```

The harness performs a 250 ms warm-up followed by a two-second measurement and
prints stable `key=value` fields:

```text
benchmark=classfile_parse_min
iterations=24720279
elapsed_ms=2000.000
ns_per_iteration=80.9
iterations_per_second=12360139.2
```

The exact values above are illustrative, not a release guarantee. Capture
several runs and compare medians rather than selecting one best run.

Example shell baseline:

```bash
mkdir -p target/bench-results
for run in 1 2 3 4 5; do
  cargo bench --bench classfile_parse \
    > "target/bench-results/classfile-${run}.txt"
done
```

## JAR Or Class-Directory Corpus Benchmark

The corpus tool separates archive extraction from class parsing:

```bash
cargo run --release --bin jar_parse_bench -- /path/to/application.jar
cargo run --release --bin jar_parse_bench -- /path/to/extracted/classes
```

For a JAR input, the tool invokes the JDK `jar` executable found under
`JAVA_HOME/bin` or on `PATH`, extracts into a unique directory under Cargo's
`target/` tree, then removes that directory on exit. Passing an already
extracted directory removes archive-tool and decompression time from the parser
measurement.

Output includes:

```text
input=/path/to/application.jar
class_files=1234
parsed_ok=1234 failed=0
total_mb=8.125
extract_time_ms=84.221
parse_time_ms=17.302
total_time_ms=101.523
ns_per_class=14021.1
parse_mb_per_s=469.60
```

Treat any non-zero `failed` count as a correctness failure before interpreting
throughput. For apples-to-apples parser comparisons, pre-extract the JAR once
and benchmark the directory so filesystem decompression does not dominate.

### Full runtime corpus gate

`scripts/check-classfile-corpus.sh` extracts complete modular runtime images
with `jimage` (and uses `rt.jar` on JDK 8), then requires every class to parse:

```bash
scripts/check-classfile-corpus.sh \
  /opt/openjdk-bin-8.492_p09 \
  /opt/openjdk-bin-11.0.31_p11 \
  /opt/openjdk-bin-17.0.19_p10 \
  /opt/openjdk-bin-21.0.11_p10 \
  /opt/openjdk-bin-25.0.3_p9 \
  /opt/openjdk-bin-27_alpha20 \
  /usr/lib64/openjdk-28
```

The 2026-08-18 pre-publication runs parsed 186,968 class files with zero
failures. That count covers all modules in the installed JDK 11, 17, 21, 25,
27, and 28 images plus JDK 8 `rt.jar`; it is correctness evidence for those
exact runtimes, not a claim that arbitrary malformed inputs are valid.

## Performance-Sensitive JNI Calls

Fixed class names, method names, field names, descriptors, and exception text
can use the allocation-free `*_cstr` methods with Rust C string literals:

```rust,ignore
let class = jni.find_class_cstr(c"java/lang/String")?;
let method = unsafe { jni.get_method_id_cstr(class, c"length", c"()I")? };
```

The existing `&str` methods remain ergonomic adapters and allocate only as
needed to validate and NUL-terminate dynamic input. Keep conversion outside a
high-frequency callback when the value is reusable.

## Live JVMTI Callback Dispatch Benchmark

Use the callback harness to measure method-entry delivery through a real JVM:

```bash
JAVA_HOME=/path/to/jdk scripts/benchmark-callback-dispatch.sh
```

It compares five paths using the same non-inlined Java method workload:

1. `baseline` - no agent.
2. `rust_idle` - Rust agent loaded, no events enabled.
3. `c_noop` - raw-C JVMTI method-entry callback with an empty body.
4. `rust_noop` - the complete Rust trampoline, callback-context construction,
   panic boundary, trait dispatch, and default empty handler.
5. `rust_counter` - the Rust path plus one relaxed atomic increment per event.

The raw-C variant is essential: baseline-to-C measures JVM TI event-delivery
cost, while C-to-Rust isolates the binding's dispatch overhead on the same JVM.
The harness rotates variant order between repetitions, verifies identical
workload checksums, and writes every observation to a TSV file under
`target/callback-dispatch-bench/`.

Workload size is configurable without changing code:

```bash
CALLBACK_BENCH_WARMUP=5000000 \
CALLBACK_BENCH_ITERATIONS=50000000 \
CALLBACK_BENCH_REPETITIONS=7 \
scripts/benchmark-callback-dispatch.sh
```

`CALLBACK_BENCH_ITERATIONS` is per worker. Exercise concurrent delivery with:

```bash
CALLBACK_BENCH_THREADS=8 \
CALLBACK_BENCH_ITERATIONS=5000000 \
CALLBACK_BENCH_REPETITIONS=5 \
scripts/benchmark-callback-dispatch.sh
```

The no-op variants measure read-only dispatch scalability. The counter variant
deliberately uses one shared relaxed atomic to reveal whether state contention
is measurable on the tested JVM and machine. Do not assume a difference when
the observed ranges overlap; use per-thread buffers or counters when real
profiler work makes shared state a demonstrated bottleneck.
With more than one worker, `ns_per_call` is elapsed wall time divided by total
calls across all workers. It is an aggregate throughput measure, not individual
callback latency. Short concurrent runs are particularly vulnerable to
scheduler and frequency noise; use larger workloads and at least five repeats.

Method-entry events can alter JVM compilation and inlining behavior. Do not
describe baseline-to-agent slowdown as Rust overhead. The `c_noop` to
`rust_noop` difference is the relevant implementation comparison; production
profiler work such as stack walking, metadata lookup, locking, allocation, or
I/O must be measured separately.

The first recorded reference run is documented in
[Callback Dispatch Benchmark - 2026-08-17](CALLBACK_DISPATCH_BENCHMARK_2026-08-17.md).
A concise table of the reference figures, allocation proof, and downstream
consumer validation is maintained in
[JVM TI 3.0 Performance Reference](PERFORMANCE_REFERENCE_3_0.md).

### Callback allocation proof

The callback benchmark measures time; the separate counting-allocator agent
proves whether normal Rust dispatch touches the Rust heap:

```bash
JAVA_HOME=/path/to/jdk scripts/prove-callback-allocation-free.sh
```

Counting starts only after agent construction, capability setup, callback
registration, and event enablement. It remains active through entry into
`VMDeath` and the script fails if it observes any allocation, reallocation, or
deallocation. This proves the crate's normal dispatch path, not arbitrary code
inside an application's callback and not allocations performed internally by
the JVM.

## Regression Discipline

1. Run correctness tests before benchmarks.
2. Use `--release` or Cargo's bench profile, never a debug binary.
3. Pin the same Rust version and CPU power policy for before/after runs.
4. Compare at least five runs and report the median plus range.
5. Separate JAR extraction time from parser time.
6. Keep benchmark inputs and their SHA-256 hashes with the result.
7. Run `scripts/check-zero-dependencies.sh` so benchmark tooling cannot quietly
   alter the dependency contract.
