# Forward Compatibility: Java 26-29, JNI, JVM TI, and JNA

Status: primary-source research complete; 3.0 compatibility layer implemented locally  
Research date: 2026-08-17  
Purpose: define the 3.0.0 API so foreseeable Java evolution can be handled in
minor releases rather than another major release

## Executive conclusion

The known Java roadmap does not justify abandoning JNI or JVM TI. It does
justify changing this crate's public abstractions before 3.0.0:

1. Java is moving toward integrity by default. Dynamic agent loading and
   Java-side native library linking will require explicit operator approval.
2. Project Valhalla introduces value objects whose identity, tagging,
   allocation-event, monitor, and constructor-frame behavior differs from
   traditional objects.
3. OpenJDK continues to evolve JNI and JVM TI additively. Current JDK 28 work
   appends one JNI function and consumes another reserved JVM TI capability bit
   without changing the established function-table order.
4. FFM is a supported alternative to JNI for Java-to-native calls, but it does
   not replace JVM TI or give foreign code access to Java objects.
5. JNA remains a JNI/libffi-based consumer. Its release evolution does not
   belong in the core Rust JVM TI API.

Version 3.0.0 should therefore be designed as the last *foreseeable* major
release, not promised as the last major under every circumstance. A future
major remains justified if OpenJDK itself makes an incompatible ABI change or
if another soundness defect requires one.

## Evidence boundary

This research distinguishes delivered behavior, active platform work, and
inference:

- JDK 26 is released and JDK 27 is in public review.
- Java SE 28 is an active JSR with a planned March 2027 release.
- Value classes are preview work and may still change before Java SE 28 is
  finalized.
- OpenJDK has not yet published a JDK 29 branch, tag, project repository, or
  Java SE 29 JSR. As of 2026-08-18, OpenJDK `master` identifies itself
  as JDK 28. This document therefore treats JDK 29 as an unknown-next-release
  compatibility boundary, not as an ABI that can already be copied.
- OpenJDK `master` is implementation evidence, not a promise that every current
  line will ship unchanged.
- No public source can reliably enumerate all changes two years in advance.

The source comparison used these pinned OpenJDK revisions:

| Line | Revision |
| --- | --- |
| JDK 26 | `4408cd2a07a14243a58cd9d30813302bfbe81133` |
| JDK 27 | `8106099da073416b2b1e1b9ff18afdfe2bafe989` |
| OpenJDK JDK 28 build 11 (`jdk-28+11`) | `d426d66b5e27f54f676f04e3263eb72be67b1f5f` |

The ABI gate fetches the immutable revision above into a commit-stamped cache.
The fetcher records the feature, tag, and commit and refreshes every input when
that marker changes, preventing a new manifest pin from silently reusing stale
headers. No source directory is labeled JDK 29 because no corresponding
upstream source exists.

The comparison covered `jni.h` and the source-of-truth `jvmti.xml`, not only
generated documentation. JDK 28 build 11 and OpenJDK mainline revision
`4d812a64865ef250bd81705ae0c0a18675e4b378` have byte-identical JNI/JVM TI ABI
inputs as of 2026-08-18.

## JDK 26 and JDK 27

The JDK 26 and JDK 27 `jni.h` files are byte-for-byte identical. Their JVM TI
specification sources have no API difference; the only observed XML change is
a punctuation correction in the ClassLoad description.

This means 3.0.0 does not need a JDK-27-specific public callback or raw ABI.
The existing JDK 21 virtual-thread additions remain the most recent delivered
JNI/JVM TI table extensions through JDK 27.

One operational change is still relevant: OpenJDK intends to expire the
temporary `-XX:AllowRedefinitionToAddDeleteMethods` compatibility option in JDK
27. The crate must never represent adding or deleting methods during
redefinition as a portable capability.

## JDK 28 and value objects

Current OpenJDK main-line sources contain the first concrete JNI and JVM TI
changes for value objects.

### JNI additions

The JNI function table appends:

```c
jboolean (JNICALL *HasIdentity)(JNIEnv *env, jobject obj);
```

The header also adds `JNI_VERSION_28`. Because `HasIdentity` is appended, older
field offsets remain stable. A wrapper must nevertheless negotiate the runtime
JNI version before reading or calling the new tail slot.

### JVM TI additions and semantic changes

The JVM TI function table does not gain a function in the audited main-line
snapshot. Its capability set gains `can_support_value_objects`, currently
marked preview and `since="28"`.

Value objects change existing semantics:

- `VMObjectAlloc` and `SampledObjectAlloc` may report value-object allocations
  only when `can_support_value_objects` is possessed.
- For those events, the `jobject` parameter is null for a value object even
  though the class parameter is present.
- `ObjectFree` is not sent for tagged value objects.
- Tags on value objects use value-equality semantics rather than stable object
  identity semantics.
- `GetClassModifiers` uses `ACC_IDENTITY` when preview features are enabled;
  the bit historically named `ACC_SUPER` is repurposed.
- `GetObjectMonitorUsage` returns empty monitor information for value objects.
- `GetLocalObject` and `GetLocalInstance` can return a snapshot of a value
  object under construction.
- `ForceEarlyReturnVoid` cannot force an early return from a value-class
  constructor.

The 3.0 callback API must not encode a non-null allocation object invariant.
The class/modifier API must interpret the identity bit in the context of the
runtime version and preview state.

## JDK 29 compatibility boundary

JDK 29 cannot yet be audited as a concrete source release. Version 3.0 must
instead make the predictable classes of JDK 29 evolution non-breaking:

- unknown JNI and JVM TI version values remain representable;
- new functions appended to native tables are never read before runtime
  version negotiation confirms their presence;
- unknown errors, event IDs, extension IDs, and capability bits round-trip;
- event payloads and callback context are crate-constructed and
  `#[non_exhaustive]`;
- preview JDK 28 value-object behavior is isolated behind feature/version
  checks, so removal or revision does not invalidate the core callback shape;
  and
- a release candidate cannot claim JDK 29 compatibility until its exact
  `jni.h` and `jvmti.xml` have passed the conformance suite.

When OpenJDK creates the JDK 29 line, the first maintenance task is to download
an immutable source snapshot under `/opt/jvmsrc`, record its revision and
archive digest, regenerate the private ABI oracle, and run the complete matrix.
That refresh should normally be a 3.x minor release; an unexpected incompatible
upstream ABI remains the explicit exception to the last-major-version goal.

## Integrity by default

### Dynamic agents

JEP 451 warns that a future JDK will disallow dynamic agent loading by default.
Startup loading with `-agentlib` or `-agentpath` remains explicitly supported
and does not produce that warning. Dynamic attach remains available when the
operator starts the JVM with `-XX:+EnableDynamicAgentLoading`.

Consequences for this crate:

- Treat startup-loaded agents as the primary deployment path.
- Keep `Agent_OnAttach`, but document the required operator opt-in.
- Never imply that an agent library can silently grant itself attach rights.
- Test both startup and denied/allowed dynamic-attach behavior.

### Native access

JEP 472 makes Java-side loading/linking through JNI and FFM subject to native
access approval. JDK 24 warns; a future release will deny by default unless the
relevant module or `ALL-UNNAMED` is enabled.

This does not deprecate JNI, and it does not forbid an already-loaded native
agent from calling JNI functions. It matters when a Java or JNA shim loads a
native library or declares native methods. Such integrations must document
`--enable-native-access` separately from core `-agentpath` use.

### Final fields

JEP 500 states that mutating final fields through JNI is undefined behavior.
JDK 26 can diagnose it with `-Xcheck:jni` or JNI debug logging, and a future JDK
may make JNI field setters return without performing the mutation.

The safe wrapper must not advertise mutation of final fields as supported.
Raw JNI setters remain available because they are part of JNI, but their safety
documentation must state the invariant and future risk.

## AOT and agent compatibility

JEP 483's AOT cache rejects or ignores configurations involving JVM TI agents
that can arbitrarily rewrite class files through `ClassFileLoadHook` or that
alter bootstrap/system class-loader search paths.

Version 3.0.0 should request capabilities narrowly and expose the active
capability/feature set to the developer. A monitoring-only agent should not
accidentally opt into transformation behavior. Documentation and diagnostics
should explain when an agent invalidates an AOT cache.

## FFM and JNA

The Foreign Function & Memory API has been final since JDK 22. Its stated
non-goals include reimplementing or changing JNI. FFM can provide a Java-side
binding to exported native functions, but it does not replace JVM TI and does
not let foreign libraries manipulate Java objects as JNI does.

JNA 5.19.1 remains implemented using a small JNI dispatch library and libffi;
5.20.0 is the next documented release line. JNA's maintainers require JDK 24+
users to grant native access to `com.sun.jna` or `ALL-UNNAMED` as appropriate.

Consequences for this crate:

- Keep JNA and FFM adapters out of the zero-dependency core.
- If Java-facing helpers are added, expose a stable C ABI from a separate
  feature or companion crate so either JNA or FFM can consume it.
- Do not expose JNA classes, FFM `MemorySegment`, or a particular Java loading
  mechanism in Rust callback signatures.

## 3.0.0 API decisions

### Callback context

`CallbackContext<'callback>` must have private fields and accessor methods. It
must be `#[non_exhaustive]` so future runtime metadata can be added without
changing every callback signature. It carries:

- the originating JVM TI environment;
- an optional JNI environment, present only where supplied and valid;
- runtime JNI/JVM TI version information; and
- callback phase/restriction metadata where it can be established reliably.

### Event payloads

Event payload structures must be `#[non_exhaustive]` and constructed only by
the crate. Borrowed views must carry the callback lifetime. Future fields can
then be added in minor releases.

Handles whose upstream contract permits null must use `Option` or an explicitly
nullable raw form. In particular, value-object allocation events cannot expose
an always-non-null object wrapper.

### Open upstream domains

- C integer domains use transparent numeric newtypes, not closed Rust enums.
- Public high-level enums that may grow are `#[non_exhaustive]`.
- JVM TI capabilities use an opaque/open bitset with named accessors rather
  than a public fixed list of Rust fields.
- Unknown capability bits, event IDs, errors, extension records, and versions
  must round-trip.

### Version and feature negotiation

New table-tail functions and preview behavior must be runtime-gated. The safe
API must return `NOT_AVAILABLE` or a typed unsupported-feature result before it
touches a slot absent from an older JVM.

JDK 28 value-object support should be included as an explicitly preview,
runtime-gated surface. It must not be represented as finalized until the Java
SE 28 specification is final.

### Future trait additions

Every standard event method present in 3.0 is canonical and unsuffixed. If a
future JDK adds an event, the crate may add a default no-op trait method in a
minor release. Existing methods are not duplicated with suffixes.

## Validation matrix

Before publishing 3.0.0:

1. Compile C/Rust layout and function-signature probes against every pinned
   JDK feature release from 8 through 28, not only representative LTS lines.
2. Diff every adjacent `jni.h`, generated `jvmti.h`, and source `jvmti.xml`;
   classify ABI, callback, capability, event, error, semantic, source-only, and
   runtime-policy changes in `version::RELEASE_PROFILES`.
3. Run callback agents on every installed JVM generation and retain exact
   callback-prefix sentinel coverage for every release.
4. Run JDK 24+ Java-shim tests with `--illegal-native-access=deny` and the
   appropriate explicit grant.
5. Run dynamic attach both denied and explicitly enabled.
6. Run transformation and non-transformation agents with AOT cache diagnostics.
7. Run JDK 28 EA value-object tests only when preview features are enabled and
   record the exact EA build used.
8. Re-diff `jni.h` and `jvmti.xml` immediately before release.
9. Add JDK 29 to this matrix as soon as an official source line exists; do not
   infer ABI support from JDK 28 `master` or a feature-project branch.

## Primary sources

- [JDK 27 project and schedule](https://openjdk.org/projects/jdk/27/)
- [Java SE 27 platform JSR 402](https://openjdk.org/projects/jdk/27/spec/)
- [Java SE 28 JSR 403](https://www.jcp.org/en/jsr/detail?id=403)
- [JEP 401: Value Classes and Objects](https://openjdk.org/jeps/401)
- [JDK 28 draft value-object JVM specification](https://cr.openjdk.org/~dlsmith/jep401/jep401-20260717/specs/value-objects-jvms.html)
- [JEP 451: Prepare to Disallow Dynamic Agent Loading](https://openjdk.org/jeps/451)
- [JEP 472: Prepare to Restrict the Use of JNI](https://openjdk.org/jeps/472)
- [JEP 500: Prepare to Make Final Mean Final](https://openjdk.org/jeps/500)
- [JEP 483: Ahead-of-Time Class Loading and Linking](https://openjdk.org/jeps/483)
- [JEP 454: Foreign Function and Memory API](https://openjdk.org/jeps/454)
- [OpenJDK JNI header](https://github.com/openjdk/jdk/blob/master/src/java.base/share/native/include/jni.h)
- [OpenJDK JVM TI specification source](https://github.com/openjdk/jdk/blob/master/src/hotspot/share/prims/jvmti.xml)
- [JNA project](https://github.com/java-native-access/jna)
- [JNA JDK 24 native-access issue](https://github.com/java-native-access/jna/issues/1665)
