# Embedded JVM Consumer Canary

This project is compiled against the exact packaged `jvmti-bindings` crate by
`scripts/check-downstream-canaries.sh`. It exercises the `embed` feature,
builder options, scoped thread attachment, JNI lookup/call surface, and
explicit VM destruction from an external package rather than from this
repository's own crate graph.
