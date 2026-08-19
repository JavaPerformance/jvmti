# Independent Unsafe And FFI Review

3.0.0 decision: pre-publication independent review waived by the release owner;
mechanical gates passed, but waiver was not represented as acceptance

3.0.2 status: exact code candidate accepted by independent review; the
evidence-only follow-up is awaiting reviewer confirmation

3.0.2 safety-fix base commit: `3ccb3ff456c1ef4492ea1d1918863a88b62a3b35`

3.0.2 reviewed code-candidate commit:
`769f05c229491ec2bdc1f8c7c98a04a6d368e806`

3.0.2 immutable external review:
[`JavaPerformance/jvmti` PR #6 review record](https://github.com/JavaPerformance/jvmti/pull/6#issuecomment-5339531055)

3.0.2 reviewer: Grok, independent of the release author

3.0.2 reviewed platform scope: Linux x86-64; OpenJDK 21.0.12; Rust 1.85.0

3.0.2 decision: **ACCEPT** the code candidate; confirm the evidence-only delta
before release

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
scope, and an explicit accept/reject decision in an immutable external review.
After acceptance, the release author regenerates the unsafe baseline and makes
an evidence-only commit. The reviewer then confirms that delta and records the
exact final commit in the same external review. A tracked file cannot contain
the hash of its own commit, so the external record, signed tag, and hosted build
evidence are the authoritative commit identity. Findings are resolved without
rewriting review history.

## 3.0.2 Post-Publication Findings

Independent review on public issue `#3` identified two correctness defects in
the published 3.0.0/3.0.1 unsafe boundary:

1. The `NativeMethodBind` trampoline replaced the VM-provided mutable output
   with null before dispatch. The 3.0.2 candidate instead leaves the output
   unchanged for absent and no-op handlers, preserves deliberate replacement,
   and restores the original value after a contained handler panic.
2. The safe heap-tagging helper could overflow in its native callback. The
   3.0.2 candidate uses checked non-zero progression, records range exhaustion
   in callback state, aborts traversal, and returns a typed JVM TI error. The
   adjacent edge collector now reserves storage fallibly before mutation.
3. `Jvmti::dispose_environment` consumed one wrapper but remained safe even
   though native disposal can invalidate callback-scoped wrappers still active
   for that environment. The candidate makes disposal unsafe and documents the
   callback-drain, resource-cleanup, and no-future-use preconditions.
4. `ClassFileLoadHook` could leave a completed transformation output installed
   after the handler subsequently panicked. The candidate treats that output as
   transactional: the panic path deallocates the pending JVM TI buffer and
   clears both output fields before returning to the VM.
5. The untrusted class-file parser accepted a terminal `CONSTANT_Long` or
   `CONSTANT_Double` without its required second constant-pool slot. Both tags
   now fail with a typed invalid-index error before their payload is consumed.

The focused regression matrix covers absent/default/redirecting/panicking
native-bind handlers, zero crossing, range exhaustion without object mutation,
propagation of callback failure through a mocked JVM TI traversal, rollback of
a panicking class transformation with exactly one deallocation, explicit unsafe
environment disposal, and malformed terminal two-slot constants. The complete
release, ABI, live-JVM, sanitizer, and 3.x compatibility gates must be rerun on
the final commit before publication.

The live boundary proofs are reproducible with:

```bash
scripts/prove-native-method-bind-live.sh /path/to/jdk8 /path/to/jdk21
scripts/prove-heap-graph-live.sh /path/to/jdk8 /path/to/jdk21
```

## 3.0.2 Independent Acceptance

The independent reviewer accepted exact commit
`769f05c229491ec2bdc1f8c7c98a04a6d368e806` after confirming that all five
post-publication findings above were closed. On Linux x86-64 with OpenJDK
21.0.12, the reviewer reported successful results for:

- `cargo +1.85.0 test --locked --all-features`;
- `cargo fmt --all -- --check`;
- the live `NativeMethodBind` preservation proof;
- the live heap-graph proof;
- the live Modified UTF-8 proof; and
- the live event-callback matrix.

The unsafe-surface baseline was deliberately excluded from the accepted code
candidate so that acceptance preceded regeneration. The release author may now
regenerate that baseline, commit only this review packet and the baseline, and
run the complete release and cross-JDK ABI gates. The reviewer must confirm the
resulting evidence-only delta before release.

The acceptance retains these residual assumptions and limitations:

1. JVM callback pointer-and-length pairs are trusted for the callback
   invocation.
2. `catch_unwind` cannot contain `panic = "abort"` or foreign exceptions.
3. Failure while deallocating a rolled-back class transformation may leak the
   buffer, but does not publish it to the VM or create a second release path.
4. A synchronously aborted heap walk may leave tags written before the abort.
5. `GLOBAL_AGENT` remains process-lifetime and is not dropped on unload.
6. Hosted Linux AArch64, macOS, Windows, sanitizer, and full supported-header
   ABI evidence remains a gate on the final evidence commit.
