# Agent Starter Template

This is a minimal JVM agent template using `jvmti-bindings`.

It requires Rust 1.85 or newer and uses Edition 2024.

## Build

1. Build the agent:
   cargo build --release

2. Run with your JVM:
   java -agentpath:./target/release/libmy_agent.so MyApp

3. The same library supports dynamic attach through `Agent_OnAttach`. The
   starter shares initialization between startup and attach while the library
   reuses one process-global agent across repeated attach requests.

## Notes

1. Update `name` and `version` in `Cargo.toml`.
2. Add your logic to `src/lib.rs`.
3. Review safety rules in `docs/SAFETY.md` and `docs/PITFALLS.md`.
4. Callback arguments are scoped wrapper values. Do not retain the context,
   event, JNI local references, or borrowed class bytes after the callback.
5. Decode JNI, JVM TI, and class-file strings with the crate's Modified UTF-8
   helpers (`name_str`, `mutf8`, or exact UTF-16 APIs), not `CStr::to_str`.
