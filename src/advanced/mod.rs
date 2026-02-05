//! Advanced helpers for JVMTI power users.
//!
//! These utilities are feature-gated because they may be expensive or VM-specific.

#[cfg(feature = "heap-graph")]
pub mod heap_graph;
