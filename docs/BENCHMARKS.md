# Benchmarks

This repo uses Criterion for microbenchmarks.

## Run

```bash
cargo bench
```

If `gnuplot` is installed, Criterion will generate richer charts. Otherwise it will fall back to a built-in backend.

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
