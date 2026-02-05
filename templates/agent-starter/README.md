# Agent Starter Template

This is a minimal JVM agent template using `jvmti-bindings`.

## Build

1. Build the agent:
   cargo build --release

2. Run with your JVM:
   java -agentpath:./target/release/libmy_agent.so MyApp

## Notes

1. Update `name` and `version` in `Cargo.toml`.
2. Add your logic to `src/lib.rs`.
3. Review safety rules in `docs/SAFETY.md` and `docs/PITFALLS.md`.
