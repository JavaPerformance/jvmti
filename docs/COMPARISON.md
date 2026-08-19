# Comparison With Alternatives

This document is a pragmatic, evidence-based comparison of Rust crates in the
JNI/JVMTI space. It focuses on documented capabilities and stated design
goals rather than download counts or unverified claims.

Last verified: 2026-08-19

## Quick Guidance

- If you are building **JVMTI agents** (profilers, tracers, debuggers), this
  crate is the intended choice: complete JNI + JVM TI tables, agent lifecycle,
  ownership wrappers, and a class-file parser.
- If you only need **JNI** for native methods, Android, or calling Java from
  Rust, use [`jni`](https://crates.io/crates/jni) (jni-rs). This crate’s JNI
  surface is complete for agents (`A`-form calls, RAII leases, MUTF-8) but is
  not a replacement for that ecosystem’s typed handles, `call_method` sugar,
  native-method macros, or Android documentation.
- If you want **code generation** or a higher-level Java/Rust interop
  framework, use a generator-style crate.

## Feature Parity Snapshot (Documented)

Legend: documented, partial/limited docs, not documented as a product goal

| Crate | JNI | JVMTI | Notes |
|---|---|---|---|
| **jvmti-bindings** | Complete fixed-signature + `A` families through JDK 28 | Complete tables, callbacks, live proofs | Agent-first; zero third-party crates; class-file parser |
| **jni** (jni-rs) | Mature, typed, widely used | Not a goal | Native methods, Android, embed-and-call-Java |
| **jni-simple** | Thin handwritten JNI | Present; authors describe low maturity | Explicitly “no magic” |
| **jvmti2** | Via JNI deps / sys | Agent-oriented safe wrapper | Lifetime-tracked environment, RAII allocations, and `jni` integration |
| **jvmti-sys** / **jvm-ti-sys** | No | Raw `jvmti.h` | Definitions only |
| **jni-sys** / **jni-sys-new** | Raw `jni.h` | No | Definitions only |
| **java-bindgen** | Generated glue | No | Codegen + CLI |

## How To Use This Comparison

1. Decide whether you need JVMTI or only JNI.
2. Decide whether you want raw bindings, agent wrappers, or code generation.
3. Pick the crate that matches that job. Do not expect one crate to win every
   Java/Rust interop shape.

If you want this matrix expanded (benchmarks, API coverage counts, examples,
CI status, and docs completeness), open an issue.
