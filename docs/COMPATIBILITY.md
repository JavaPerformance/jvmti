# Compatibility Matrix (JDK 8-27)

This crate is verified against OpenJDK headers through JDK 27.

| JDK | JNI Additions | JVMTI Additions | Notes |
|---|---|---|---|
| 8 | Baseline | Baseline | All core JNI/JVMTI functions |
| 9 | GetModule | Module-related functions | JPMS support |
| 11 | - | SetHeapSamplingInterval | Heap sampling support |
| 21 | IsVirtualThread | Virtual thread events | Project Loom |
| 24/25 | GetStringUTFLengthAsLong | - | UTF length as long |
| 27 | - | ClearAllFramePops | Reserved slot repurposed |

Class file parsing supports all standard attributes through Java 27.
