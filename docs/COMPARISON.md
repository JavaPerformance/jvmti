# Comparison With Alternatives

This document is a pragmatic, evidence-based comparison of Rust crates in the JNI/JVMTI space.
It focuses on *documented* capabilities and stated design goals rather than speculation.

Last verified: 2026-02-05

## Quick Guidance

- If you are building **JVMTI agents** (profilers, tracers, debuggers), this crate is designed for that use case.
- If you only need **JNI** for native methods or embedding a JVM, a JNI-focused crate may be sufficient.
- If you want **code generation** or a higher-level Java/Rust interop framework, use a generator-style crate.

## Feature Parity Snapshot (Documented)

Legend: ✅ documented, ⚠️ partial/limited docs, ❔ not documented

| Crate | JNI | JVMTI | Safety/ergonomics focus | Notes |
|---|---|---|---|---|
| **jvmti-bindings** | ✅ | ✅ | Explicit safety boundaries, agent-first ergonomics | Full JNI + JVMTI, safe wrappers, classfile parser |
| **jni** | ✅ | ❔ | Safe JNI bindings | JNI-only focus (native methods, calling Java, embedding JVM) |
| **jni-simple** | ✅ | ✅ | Minimal wrapper, unsafe-first | JNI is mature; JVMTI described as low maturity and low test coverage |
| **rust-jni** | ✅ | ❔ | Safety and typed interop | JNI-focused safe interop with macros |
| **jni-sys-new** | ✅ | ❔ | Raw definitions | Low-level `jni.h` definitions |
| **jvmti-sys** | ❔ | ✅ | Raw definitions | Low-level `jvmti.h` definitions |
| **java-bindgen** | ✅ | ❔ | Generator + CLI | Generates JNI bindings and Java glue |

## Notes On Alternatives

- **jni** focuses on JNI bindings for implementing native methods, calling Java, and embedding the JVM.
- **jni-simple** is explicitly a “no-magic” binding for JNI and JVMTI; it documents mature JNI coverage but low JVMTI maturity.
- **rust-jni** emphasizes safe interoperation between Rust and Java using JNI, with helper macros.
- **jni-sys-new** provides low-level Rust definitions corresponding to `jni.h`.
- **jvmti-sys** provides low-level Rust definitions corresponding to `jvmti.h`.
- **java-bindgen** is a JNI bindings generator and CLI tool for producing Java/Rust glue.

## How To Use This Comparison

1. Decide whether you need JVMTI or only JNI.
2. Decide whether you want **raw bindings** or **ergonomic wrappers**.
3. Decide whether you want **manual FFI** or **code generation**.
4. Pick the crate that aligns to those constraints.

If you want this matrix expanded (benchmarks, API coverage counts, examples, CI status, and docs completeness), open an issue and we’ll extend it.
