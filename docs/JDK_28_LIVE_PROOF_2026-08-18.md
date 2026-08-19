# JDK 28 Preview Runtime Proof

Date: 2026-08-18
Historical 3.0.0-era evidence snapshot. Revision:
`46d6a064399259a6c0115f0fc5e90f006fada54e`

## Scope And Verdict

The 3.0.0 line passed its required semantic/live matrix on an installed JDK
28 preview runtime. This closed the preview-runtime proof gap tracked by GitHub
issue 4. It does not claim that preview value-object behavior is final Java SE
28 behavior.

## Exact Runtime Identity

| Field | Value |
| --- | --- |
| Package | Gentoo `dev-java/openjdk-28_alpha7` |
| Runtime | OpenJDK `28+7`, 64-bit Server VM |
| Runtime source archive | `https://github.com/openjdk/jdk/archive/jdk-28+7.tar.gz` |
| Upstream source tag | `jdk-28+7` |
| Peeled upstream commit | `4086d114ed3fe82edb9005521cc6ede340ea0299` |
| Gentoo repository revision recorded by package | `421326eec827cfccfc023c92a29b02ccf91bb201` |
| Installed `bin/java` SHA-256 | `8f33bbb41d50d9b8b372ba8f4ad77bf97b6ccfb0686d104e419aee534be528b4` |
| Host | Linux x86-64, Gentoo |

The crate's source-ABI gate separately pins the newer JDK 28 build 11 source.
The live proof therefore exercises build 7 runtime behavior while the compile-
time table/signature inventory remains checked against build 11.

## Results

| Required proof | Command | Result |
| --- | --- | --- |
| Callback registration and payload delivery | `scripts/prove-event-callback-matrix.sh /usr/lib64/openjdk-28` | Pass: 3 ABI tests; 3,156 method entries; 4 GC starts; 4 GC finishes |
| Java Modified UTF-8 | `scripts/prove-mutf8-live.sh /usr/lib64/openjdk-28` | Pass: NUL, supplementary character, Japanese text, and unpaired-surrogate behavior |
| Repeated dynamic attach | `scripts/prove-repeated-attach-live.sh /usr/lib64/openjdk-28` | Pass: the same process-global agent handled attach 1 and attach 2 |
| Startup and dynamic-attach policy | `scripts/prove-attach-policy-live.sh /usr/lib64/openjdk-28` | Pass: startup allowed, explicit denial enforced, explicit enablement worked |
| Heap graph callbacks | `scripts/prove-heap-graph-live.sh /usr/lib64/openjdk-28` | Pass: 28,242 objects tagged and 76,216 graph edges observed |
| Complete runtime class corpus | `scripts/check-classfile-corpus.sh /usr/lib64/openjdk-28` | Pass: 27,377 parsed, 0 failed, 129.579 MiB |

In addition, the `bytecode-instrument` downstream dogfood loaded the same 3.0
candidate into this JDK 28 runtime, collected 577 classes across three loaders,
transformed a real application class, executed entry/exit probes, and preserved
the checked application result.

## Evidence Location

Local raw logs for this run are under:

```text
target/jdk28-release-proof-20260818/
```

The directory contains the runtime identity, event, MUTF-8, repeated-attach,
attach-policy, heap-graph, and classfile-corpus logs. Generated test agents and
scratch inputs remain under the repository's `target/` tree.

## Claim Boundary

This proves the generic callback, lifecycle, string, heap-callback, attach, and
classfile contracts required by the JDK 28 preview gate. It does not include a
specialized inline/value-class application fixture, and it does not convert
preview APIs or semantics into a final-version guarantee. `HasIdentity` and the
value-object JVM TI capability remain runtime-gated preview features.
