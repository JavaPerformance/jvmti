# Contributor Style Guide

This guide keeps the API and docs consistent and easy to use.

## Import Style

1. Prefer `use jvmti_bindings::prelude::*;` in examples and docs.
2. Use `env` for safe wrappers and `sys` only for raw FFI types.
3. Avoid importing `sys` in casual examples unless required.

## API Design Rules

1. Keep public APIs small and focused.
2. Return owned Rust types instead of raw JVMTI structs.
3. Avoid `unwrap()` in any code path reachable from callbacks.
4. Prefer `Result<T, jvmtiError>` for JVMTI operations.
5. Add doc comments explaining safety assumptions when needed.

## Docs and Examples

1. Examples should compile as `cdylib` agents.
2. Show capability request, callback registration, and event enablement.
3. Mention thread-local `JNIEnv` constraints in any example using JNI.

## Feature Flags

1. Advanced helpers must be behind feature flags.
2. Document any feature-gated behavior in README and docs.
