//! Common imports for building JVMTI agents.
//!
//! This prelude is intentionally small. It covers the types and helpers most
//! agents use while avoiding over-broad re-exports.

pub use crate::env::{GlobalRef, JniEnv, Jvmti, LocalRef};
pub use crate::export_agent;
pub use crate::get_default_callbacks;
pub use crate::sys::{jni, jvmti};
pub use crate::Agent;
#[cfg(feature = "embed")]
pub use crate::embed::{JavaVm, JavaVmBuilder};
