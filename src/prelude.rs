//! Common imports for building JVMTI agents.
//!
//! This prelude is intentionally small. It covers the types and helpers most
//! agents use while avoiding over-broad re-exports.

pub use crate::Agent;
pub use crate::agent::{AgentLoadContext, AgentUnloadContext, JavaVmRef};
pub use crate::callbacks::*;
pub use crate::describe_jni_result;
#[cfg(feature = "embed")]
pub use crate::embed::{AttachedThread, JavaVm, JavaVmBuilder, find_libjvm, find_libjvm_verbose};
pub use crate::env::{
    GlobalRef, JavaMonitorGuard, JniEnv, JniFunctionTable, JniVersionError, Jvmti, JvmtiAllocation,
    LocalFrame, LocalRef, PrimitiveArrayCritical, PrimitiveArrayElements, RawMonitor,
    RawMonitorGuard, StringCritical, WeakGlobalRef,
};
pub use crate::export_agent;
pub use crate::get_default_callbacks;
pub use crate::mutf8::{Mutf8Error, Mutf8ErrorKind};
pub use crate::sys::{jni, jvmti};
pub use crate::version::{
    FeatureMaturity, JniFeature, JvmtiErrorAddition, JvmtiFeature, JvmtiSemanticChange,
    MAX_VERIFIED_JDK, MIN_SUPPORTED_JDK, NativePolicyChange, NativeSourceChange, ReleaseDelta,
    ReleaseProfile, RuntimeChange, RuntimeSupport, jvmti_interface_feature, release_delta,
    release_profile, runtime_support,
};
