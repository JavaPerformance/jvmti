# Independent Unsafe And FFI Review

Status: required before publishing 3.0.0

Candidate commit: to be recorded after the release-candidate commit exists

Independent reviewer: unassigned

Decision: pending

This is the handoff packet for an independent reviewer. Automated gates catch
known structural failures; they do not prove that every safety precondition is
true in every JVM execution.

## Review Scope

Review the exact release-candidate commit, including:

1. `src/sys/jni.rs` and `src/sys/jvmti.rs`: C ABI, nullability, table prefixes,
   calling conventions, variadic boundaries, open numeric domains, and
   platform-dependent capability bitfields.
2. `src/lib.rs` and `src/callbacks.rs`: panic containment, callback payload
   completeness, pointer lifetime, JNI availability, mutable callback outputs,
   repeated attach, and unload behavior.
3. `src/jni_wrapper.rs` and `src/jvmti_wrapper.rs`: argument forwarding,
   version gates, ownership transfer, array and critical leases, references,
   local frames, monitors, and error paths.
4. `src/embed.rs` and `src/dynamic_library.rs`: library lifetime, thread
   attachment, VM destruction, failed-destroy retention, and symbol typing.
5. `src/mutf8.rs` and `src/classfile.rs`: Modified UTF-8 fidelity, unpaired
   surrogates, integer overflow, recursion, input budgets, and allocation
   accounting.
6. `src/advanced/heap_graph.rs`: callback return controls, tags, user-data
   lifetimes, and traversal cleanup.

## Required Questions

- Can safe Rust manufacture an invalid native pointer, outlive an environment,
  cross a forbidden thread boundary, or cause a second release?
- Can any native count, length, table slot, callback field, or union value be
  interpreted before validation?
- Does every owned native resource have exactly one release path after partial
  failure, explicit close, retry, and drop?
- Does every callback preserve the originating `jvmtiEnv*`, optional
  `JNIEnv*`, complete payload, and mutable output contract?
- Can panic or unwinding cross any JNI/JVM TI entry point?
- Are all `Send`/`Sync` decisions justified by JVM guarantees rather than Rust
  pointer representation alone?
- Are JDK-version and platform gates evaluated before touching appended or
  reclaimed ABI slots?

## Mechanical Evidence To Re-run

```bash
scripts/run-release-gates.sh
scripts/check-jdk-abi.sh --all-releases
scripts/prove-event-callback-matrix.sh
scripts/prove-callback-allocation-free.sh
scripts/prove-mutf8-live.sh
scripts/prove-repeated-attach-live.sh
scripts/prove-heap-graph-live.sh
scripts/check-unsafe-surface.py
```

If an accepted change intentionally alters the unsafe surface, regenerate the
reviewed baseline only after completing the questions above:

```bash
scripts/check-unsafe-surface.py --write
```

The reviewer records findings, residual assumptions, reviewed commit, platform
scope, and an explicit accept/reject decision in this document or a linked
review. The release author then resolves findings without rewriting the review
history.
