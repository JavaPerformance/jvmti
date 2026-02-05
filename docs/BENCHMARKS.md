# Benchmarks

This repo uses Criterion for microbenchmarks.

## Run

```bash
cargo bench
```

If `gnuplot` is installed, Criterion will generate richer charts. Otherwise it will fall back to a built-in backend.

## Streaming JAR Benchmark

For an end-to-end benchmark (read + decompress + parse class files directly from a JAR), use the streaming tool:

```bash
cargo run --features bench-tools --bin jar_parse_bench -- /path/to/app.jar
```

This tool uses the optional `zip` dependency and is gated behind the `bench-tools` feature, so the library remains dependency-free by default.

## Reports

Criterion outputs HTML + SVG reports under:

```
target/criterion/report/index.html
target/criterion/<bench_name>/report/index.html
```

Example chart paths for the `classfile_parse_min` benchmark:

```
target/criterion/classfile_parse_min/report/typical.svg
target/criterion/classfile_parse_min/report/mean.svg
target/criterion/classfile_parse_min/report/median.svg
target/criterion/classfile_parse_min/report/slope.svg
target/criterion/classfile_parse_min/report/regression.svg
target/criterion/classfile_parse_min/report/pdf.svg
```

## Baselines

You can capture and compare baselines:

```bash
cargo bench -- --save-baseline main
cargo bench -- --baseline main
```

## Recent Results (Local Machine)

These numbers are from 2026-02-05 and will vary by hardware and OS cache state. They are included as an example baseline.

Small JAR (extracted files):

```
jar=/root/vliss/OpeningHours-0.0.1-SNAPSHOT.jar
class_files=121
total_mb=0.438
parse_time_ms=3.183 (warm cache)
```

Large JAR (extracted files):

```
jar=/root/.local/share/JetBrains/Toolbox/apps/android-studio/lib/app.jar
class_files=52040
total_mb=222.578
parse_time_ms=1293.089 (warm cache)
```

Large JAR (unzip + parse estimate):

```
unzip_time=1.461s
parse_time_ms=1293.089
estimated_total=~2.75s
```

Large JAR (cold cache parse, after `sync; echo 3 > /proc/sys/vm/drop_caches`):

```
parse_time_ms=2557.225
mb_per_s=87.04
```
