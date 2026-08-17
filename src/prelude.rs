//! Common imports for building JVMTI agents.
//!
//! This prelude is intentionally small. It covers the types and helpers most
//! agents use while avoiding over-broad re-exports.

pub use crate::agent::{AgentLoadContext, AgentUnloadContext, JavaVmRef};
pub use crate::callbacks::*;
pub use crate::describe_jni_result;
#[cfg(feature = "embed")]
pub use crate::embed::{find_libjvm, find_libjvm_verbose, AttachedThread, JavaVm, JavaVmBuilder};
pub use crate::env::{
    GlobalRef, JniEnv, JniFunctionTable, JniVersionError, Jvmti, JvmtiAllocation, LocalRef,
};
pub use crate::export_agent;
pub use crate::get_default_callbacks;
pub use crate::sys::{jni, jvmti};
pub use crate::version::{
    jvmti_interface_feature, release_delta, release_profile, runtime_support, FeatureMaturity,
    JniFeature, JvmtiErrorAddition, JvmtiFeature, JvmtiSemanticChange, NativePolicyChange,
    NativeSourceChange, ReleaseDelta, ReleaseProfile, RuntimeChange, RuntimeSupport,
    MAX_VERIFIED_JDK, MIN_SUPPORTED_JDK,
};
pub use crate::Agent;
