//! Release-aware compatibility data for JNI and JVM TI evolution.
//!
//! JNI and JVM TI evolve in several different ways. A new operation may be
//! appended to a function table, occupy a formerly reserved slot, consume a
//! reserved capability bit, or only change the semantics of an existing
//! operation. Runtime policy can also change without changing either native
//! interface. This module records every native-interface-affecting delta in the
//! audited JDK 8-28 range. Safe wrappers use the structural feature gates;
//! diagnostics and callers can inspect semantic, source, and policy changes.

use crate::sys::{jni, jvmti};

/// Oldest Java feature release covered by the 3.x compatibility contract.
pub const MIN_SUPPORTED_JDK: u16 = 8;

/// Newest Java feature release verified against an immutable OpenJDK source.
pub const MAX_VERIFIED_JDK: u16 = 28;

/// Confidence boundary for a runtime feature release.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum RuntimeSupport {
    /// The release predates this crate's support contract.
    Unsupported,
    /// The release has passed the complete header and callback matrix.
    Verified,
    /// The release is newer than the newest source available to this build.
    UnverifiedFuture,
}

/// Maturity of an additive API on a particular runtime.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum FeatureMaturity {
    Unavailable,
    Preview,
    Permanent,
    UnverifiedFuture,
}

/// Additive JNI function-table features exposed by this crate.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum JniFeature {
    /// `JNINativeInterface_::GetModule`, added in JDK 9.
    Modules,
    /// `JNINativeInterface_::IsVirtualThread`, previewed in JDK 19.
    VirtualThreads,
    /// `JNINativeInterface_::GetStringUTFLengthAsLong`, added in JDK 24.
    ModifiedUtf8LongLength,
    /// `JNINativeInterface_::HasIdentity`, previewed in JDK 28.
    ValueObjectIdentity,
}

impl JniFeature {
    /// JNI version that must be reported before the corresponding tail slot is
    /// read.
    pub const fn required_version(self) -> jni::jint {
        match self {
            Self::Modules => jni::JNI_VERSION_9,
            Self::VirtualThreads => jni::JNI_VERSION_19,
            Self::ModifiedUtf8LongLength => jni::JNI_VERSION_24,
            Self::ValueObjectIdentity => jni::JNI_VERSION_28,
        }
    }

    pub const fn operation(self) -> &'static str {
        match self {
            Self::Modules => "JNI GetModule",
            Self::VirtualThreads => "JNI IsVirtualThread",
            Self::ModifiedUtf8LongLength => "JNI GetStringUTFLengthAsLong",
            Self::ValueObjectIdentity => "JNI HasIdentity",
        }
    }

    /// Maturity of this feature on a Java feature release.
    pub const fn maturity_on(self, release: u16) -> FeatureMaturity {
        if release > MAX_VERIFIED_JDK {
            return FeatureMaturity::UnverifiedFuture;
        }
        if release < self.required_release() {
            return FeatureMaturity::Unavailable;
        }
        match self {
            Self::VirtualThreads if release < 21 => FeatureMaturity::Preview,
            Self::ValueObjectIdentity => FeatureMaturity::Preview,
            _ => FeatureMaturity::Permanent,
        }
    }

    /// Java feature release that introduced the table slot.
    pub const fn required_release(self) -> u16 {
        match self {
            Self::Modules => 9,
            Self::VirtualThreads => 19,
            Self::ModifiedUtf8LongLength => 24,
            Self::ValueObjectIdentity => 28,
        }
    }
}

/// Additive JVM TI features exposed by this crate.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum JvmtiFeature {
    /// Module functions and early-start capability bits, added in JDK 9.
    Modules,
    /// Sampled allocation event, capability, and function, added in JDK 11.
    HeapSampling,
    /// Virtual-thread capability, functions, and events, previewed in JDK 19.
    VirtualThreads,
    /// `ClearAllFramePops`, installed into reserved function-table slot 67 in
    /// JDK 25.
    ClearAllFramePops,
    /// Preview value-object capability and semantics in JDK 28.
    ValueObjects,
}

impl JvmtiFeature {
    /// Java feature release required by this JVM TI surface.
    ///
    /// `GetVersionNumber` reports the current platform feature in the major
    /// field even when the generated header does not define a named
    /// `JVMTI_VERSION_<feature>` constant.
    pub const fn required_feature(self) -> u16 {
        match self {
            Self::Modules => 9,
            Self::HeapSampling => 11,
            Self::VirtualThreads => 19,
            Self::ClearAllFramePops => 25,
            Self::ValueObjects => 28,
        }
    }

    /// Encoded JVM TI version suitable for diagnostics and comparisons.
    pub const fn required_version(self) -> jni::jint {
        jvmti::version_for_feature(self.required_feature())
    }

    pub const fn operation(self) -> &'static str {
        match self {
            Self::Modules => "JVM TI module support",
            Self::HeapSampling => "JVM TI heap sampling",
            Self::VirtualThreads => "JVM TI virtual-thread support",
            Self::ClearAllFramePops => "JVM TI ClearAllFramePops",
            Self::ValueObjects => "JVM TI value-object support",
        }
    }

    /// Maturity of this feature on a Java feature release.
    pub const fn maturity_on(self, release: u16) -> FeatureMaturity {
        if release > MAX_VERIFIED_JDK {
            return FeatureMaturity::UnverifiedFuture;
        }
        if release < self.required_feature() {
            return FeatureMaturity::Unavailable;
        }
        match self {
            Self::VirtualThreads if release < 21 => FeatureMaturity::Preview,
            Self::ValueObjects => FeatureMaturity::Preview,
            _ => FeatureMaturity::Permanent,
        }
    }
}

/// JVM TI behavior changes that do not necessarily alter a C structure.
///
/// These are explicit because a layout-only compatibility check cannot detect
/// when an unchanged function gains a new legal input, output, or restriction.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum JvmtiSemanticChange {
    PrimordialClassFileLoadHooksRestricted,
    CompiledMethodLoadAllowedDuringStart,
    CurrentThreadMayBeNullDuringEarlyStart,
    ImplementationDefinedClassesMayBeUnmodifiable,
    NestmateAttributesImmutableDuringRedefinition,
    RedefineAnyClassMeansAnyModifiableClass,
    PopFrameAllowsCurrentThread,
    RecordAttributeImmutableDuringRedefinition,
    PermittedSubclassesImmutableDuringRedefinition,
    AttachFailureMaySkipAgentUnload,
    LegacyHeapFunctionsDeprecated,
    VirtualThreadsFinal,
    LivePhaseAgentStartupWarns,
    ValueAllocationObjectMayBeNull,
    ValueObjectFreeIsNotReported,
    ValueObjectTagsUseValueEquality,
    ClassModifierBitRepresentsIdentity,
    ValueObjectMonitorUsageIsEmpty,
    ValueObjectLocalsMayBeSnapshots,
    ValueConstructorRejectsForceEarlyReturnVoid,
}

/// Standard JVM TI error constants added after the JDK 8 baseline.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum JvmtiErrorAddition {
    /// JDK 11 error 72.
    UnsupportedRedefinitionClassAttributeChanged,
    /// JDK 19 error 73.
    UnsupportedOperation,
}

impl JvmtiSemanticChange {
    pub const fn description(self) -> &'static str {
        match self {
            Self::PrimordialClassFileLoadHooksRestricted => {
                "ClassFileLoadHook is no longer delivered in the primordial phase"
            }
            Self::CompiledMethodLoadAllowedDuringStart => {
                "CompiledMethodLoad may be delivered during the start phase"
            }
            Self::CurrentThreadMayBeNullDuringEarlyStart => {
                "GetCurrentThread may return null during early VM start"
            }
            Self::ImplementationDefinedClassesMayBeUnmodifiable => {
                "implementation-defined classes may be unmodifiable even with any-class capabilities"
            }
            Self::NestmateAttributesImmutableDuringRedefinition => {
                "NestHost and NestMembers cannot change during redefinition"
            }
            Self::RedefineAnyClassMeansAnyModifiableClass => {
                "can_redefine_any_class applies to every modifiable class"
            }
            Self::PopFrameAllowsCurrentThread => {
                "PopFrame accepts a suspended thread or the current thread"
            }
            Self::RecordAttributeImmutableDuringRedefinition => {
                "Record cannot change during redefinition"
            }
            Self::PermittedSubclassesImmutableDuringRedefinition => {
                "PermittedSubclasses cannot change during redefinition"
            }
            Self::AttachFailureMaySkipAgentUnload => "a failed attach may suppress Agent_OnUnload",
            Self::LegacyHeapFunctionsDeprecated => "JVM TI 1.0 heap functions are deprecated",
            Self::VirtualThreadsFinal => "virtual-thread JVM TI behavior is permanent",
            Self::LivePhaseAgentStartupWarns => {
                "starting an agent in the live phase produces a warning"
            }
            Self::ValueAllocationObjectMayBeNull => {
                "allocation events may carry a null object for value objects"
            }
            Self::ValueObjectFreeIsNotReported => {
                "ObjectFree is not reported for tagged value objects"
            }
            Self::ValueObjectTagsUseValueEquality => {
                "value-object tags follow value equality rather than stable identity"
            }
            Self::ClassModifierBitRepresentsIdentity => {
                "the historical ACC_SUPER bit represents ACC_IDENTITY"
            }
            Self::ValueObjectMonitorUsageIsEmpty => {
                "GetObjectMonitorUsage reports empty monitor state for value objects"
            }
            Self::ValueObjectLocalsMayBeSnapshots => {
                "local-object queries may return value-object construction snapshots"
            }
            Self::ValueConstructorRejectsForceEarlyReturnVoid => {
                "ForceEarlyReturnVoid cannot exit a value-class constructor"
            }
        }
    }
}

/// Source-level native changes that preserve the binary ABI.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum NativeSourceChange {
    /// JDK 16 changed anonymous C structure tags such as
    /// `_jvmtiThreadInfo` to the public typedef name `jvmtiThreadInfo`.
    NamedJvmtiStructureTags,
}

impl NativeSourceChange {
    pub const fn description(self) -> &'static str {
        match self {
            Self::NamedJvmtiStructureTags => {
                "JVM TI C structure tags use their public typedef names; layout is unchanged"
            }
        }
    }
}

/// Operational policy changes relevant to native agents and embedded VMs.
///
/// These do not change a JNI or JVM TI table. They still affect whether an
/// otherwise ABI-compatible agent can load, attach, transform classes, or use
/// an optimization supplied by a newer runtime.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum NativePolicyChange {
    /// JEP 451, delivered in JDK 21.
    DynamicAgentLoadingWarns,
    /// JEP 472, delivered in JDK 24.
    NativeLibraryLoadingRequiresEnabledAccess,
    /// JEP 483, delivered in JDK 24.
    TransformingAgentsCanInvalidateAotCache,
    /// JEP 500 diagnostics, delivered in JDK 26.
    JniFinalFieldMutationDiagnostics,
}

impl NativePolicyChange {
    pub const fn description(self) -> &'static str {
        match self {
            Self::DynamicAgentLoadingWarns => {
                "dynamic agent loading warns unless explicitly enabled"
            }
            Self::NativeLibraryLoadingRequiresEnabledAccess => {
                "native library loading and linking warn unless native access is enabled"
            }
            Self::TransformingAgentsCanInvalidateAotCache => {
                "class-transforming or class-path-extending JVM TI agents can invalidate the AOT cache"
            }
            Self::JniFinalFieldMutationDiagnostics => {
                "JNI final-field mutation is undefined and JDK diagnostics can report it"
            }
        }
    }
}

/// A change first introduced by one Java feature release.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum RuntimeChange {
    Jni(JniFeature),
    Jvmti(JvmtiFeature),
    JvmtiError(JvmtiErrorAddition),
    JvmtiSemantic(JvmtiSemanticChange),
    NativeSource(NativeSourceChange),
    NativePolicy(NativePolicyChange),
}

/// Exact native-table prefixes and first-introduced behavior for one release.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ReleaseProfile {
    pub feature: u16,
    /// Highest JNI interface version advertised by this release.
    pub jni_interface_version: jni::jint,
    /// JVM TI interface version reported by `GetVersionNumber`.
    pub jvmti_interface_version: jni::jint,
    pub jni_function_slots: u16,
    pub jvmti_function_slots: u16,
    pub event_callback_slots: u16,
    pub changes: &'static [RuntimeChange],
}

/// Difference between one audited release and its immediate predecessor.
///
/// Interface revisions and native table prefixes are compared directly;
/// [`RuntimeChange`] records changes that cannot be derived from byte sizes.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ReleaseDelta {
    pub from_feature: u16,
    pub to_feature: u16,
    pub jni_interface_changed: bool,
    pub jvmti_interface_changed: bool,
    pub jni_function_prefix_changed: bool,
    pub jvmti_function_prefix_changed: bool,
    pub event_callback_prefix_changed: bool,
    pub changes: &'static [RuntimeChange],
}

impl ReleaseProfile {
    pub const fn jni_table_bytes(self) -> usize {
        self.jni_function_slots as usize * std::mem::size_of::<*const ()>()
    }

    pub const fn jvmti_table_bytes(self) -> usize {
        self.jvmti_function_slots as usize * std::mem::size_of::<*const ()>()
    }

    pub const fn event_callbacks_bytes(self) -> usize {
        self.event_callback_slots as usize * std::mem::size_of::<*const ()>()
    }
}

const JDK9_CHANGES: &[RuntimeChange] = &[
    RuntimeChange::Jni(JniFeature::Modules),
    RuntimeChange::Jvmti(JvmtiFeature::Modules),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::PrimordialClassFileLoadHooksRestricted),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::CompiledMethodLoadAllowedDuringStart),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::CurrentThreadMayBeNullDuringEarlyStart),
    RuntimeChange::JvmtiSemantic(
        JvmtiSemanticChange::ImplementationDefinedClassesMayBeUnmodifiable,
    ),
];
const JDK11_CHANGES: &[RuntimeChange] = &[
    RuntimeChange::Jvmti(JvmtiFeature::HeapSampling),
    RuntimeChange::JvmtiError(JvmtiErrorAddition::UnsupportedRedefinitionClassAttributeChanged),
    RuntimeChange::JvmtiSemantic(
        JvmtiSemanticChange::NestmateAttributesImmutableDuringRedefinition,
    ),
];
const JDK13_CHANGES: &[RuntimeChange] = &[
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::RedefineAnyClassMeansAnyModifiableClass),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::PopFrameAllowsCurrentThread),
];
const JDK14_CHANGES: &[RuntimeChange] = &[RuntimeChange::JvmtiSemantic(
    JvmtiSemanticChange::RecordAttributeImmutableDuringRedefinition,
)];
const JDK15_CHANGES: &[RuntimeChange] = &[RuntimeChange::JvmtiSemantic(
    JvmtiSemanticChange::PermittedSubclassesImmutableDuringRedefinition,
)];
const JDK16_CHANGES: &[RuntimeChange] = &[RuntimeChange::NativeSource(
    NativeSourceChange::NamedJvmtiStructureTags,
)];
const JDK17_CHANGES: &[RuntimeChange] = &[
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::AttachFailureMaySkipAgentUnload),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::LegacyHeapFunctionsDeprecated),
];
const JDK19_CHANGES: &[RuntimeChange] = &[
    RuntimeChange::Jni(JniFeature::VirtualThreads),
    RuntimeChange::Jvmti(JvmtiFeature::VirtualThreads),
    RuntimeChange::JvmtiError(JvmtiErrorAddition::UnsupportedOperation),
];
const JDK21_CHANGES: &[RuntimeChange] = &[
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::VirtualThreadsFinal),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::LivePhaseAgentStartupWarns),
    RuntimeChange::NativePolicy(NativePolicyChange::DynamicAgentLoadingWarns),
];
const JDK24_CHANGES: &[RuntimeChange] = &[
    RuntimeChange::Jni(JniFeature::ModifiedUtf8LongLength),
    RuntimeChange::NativePolicy(NativePolicyChange::NativeLibraryLoadingRequiresEnabledAccess),
    RuntimeChange::NativePolicy(NativePolicyChange::TransformingAgentsCanInvalidateAotCache),
];
const JDK25_CHANGES: &[RuntimeChange] = &[RuntimeChange::Jvmti(JvmtiFeature::ClearAllFramePops)];
const JDK26_CHANGES: &[RuntimeChange] = &[RuntimeChange::NativePolicy(
    NativePolicyChange::JniFinalFieldMutationDiagnostics,
)];
const JDK28_CHANGES: &[RuntimeChange] = &[
    RuntimeChange::Jni(JniFeature::ValueObjectIdentity),
    RuntimeChange::Jvmti(JvmtiFeature::ValueObjects),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::ValueAllocationObjectMayBeNull),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::ValueObjectFreeIsNotReported),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::ValueObjectTagsUseValueEquality),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::ClassModifierBitRepresentsIdentity),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::ValueObjectMonitorUsageIsEmpty),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::ValueObjectLocalsMayBeSnapshots),
    RuntimeChange::JvmtiSemantic(JvmtiSemanticChange::ValueConstructorRejectsForceEarlyReturnVoid),
];

const NO_CHANGES: &[RuntimeChange] = &[];

const fn profile(feature: u16, changes: &'static [RuntimeChange]) -> ReleaseProfile {
    ReleaseProfile {
        feature,
        jni_interface_version: if feature >= 28 {
            jni::JNI_VERSION_28
        } else if feature >= 24 {
            jni::JNI_VERSION_24
        } else if feature >= 21 {
            jni::JNI_VERSION_21
        } else if feature >= 20 {
            jni::JNI_VERSION_20
        } else if feature >= 19 {
            jni::JNI_VERSION_19
        } else if feature >= 10 {
            jni::JNI_VERSION_10
        } else if feature >= 9 {
            jni::JNI_VERSION_9
        } else {
            jni::JNI_VERSION_1_8
        },
        jvmti_interface_version: jvmti::version_for_feature(feature),
        jni_function_slots: if feature >= 28 {
            237
        } else if feature >= 24 {
            236
        } else if feature >= 19 {
            235
        } else if feature >= 9 {
            234
        } else {
            233
        },
        jvmti_function_slots: if feature >= 11 { 156 } else { 155 },
        event_callback_slots: if feature >= 19 {
            39
        } else if feature >= 11 {
            37
        } else {
            35
        },
        changes,
    }
}

/// Audited profile for every supported Java feature release.
pub const RELEASE_PROFILES: [ReleaseProfile; 21] = [
    profile(8, NO_CHANGES),
    profile(9, JDK9_CHANGES),
    profile(10, NO_CHANGES),
    profile(11, JDK11_CHANGES),
    profile(12, NO_CHANGES),
    profile(13, JDK13_CHANGES),
    profile(14, JDK14_CHANGES),
    profile(15, JDK15_CHANGES),
    profile(16, JDK16_CHANGES),
    profile(17, JDK17_CHANGES),
    profile(18, NO_CHANGES),
    profile(19, JDK19_CHANGES),
    profile(20, NO_CHANGES),
    profile(21, JDK21_CHANGES),
    profile(22, NO_CHANGES),
    profile(23, NO_CHANGES),
    profile(24, JDK24_CHANGES),
    profile(25, JDK25_CHANGES),
    profile(26, JDK26_CHANGES),
    profile(27, NO_CHANGES),
    profile(28, JDK28_CHANGES),
];

/// Return the exact audited profile for a Java feature release.
pub const fn release_profile(feature: u16) -> Option<&'static ReleaseProfile> {
    if feature < MIN_SUPPORTED_JDK || feature > MAX_VERIFIED_JDK {
        return None;
    }
    Some(&RELEASE_PROFILES[(feature - MIN_SUPPORTED_JDK) as usize])
}

/// Return the audited changes between a release and its immediate predecessor.
///
/// JDK 8 is the baseline and therefore has no adjacent delta. A false
/// structural flag means only that the corresponding prefix or interface
/// revision did not change; consult [`ReleaseDelta::changes`] for semantic,
/// source, and operational policy changes.
pub fn release_delta(feature: u16) -> Option<ReleaseDelta> {
    if feature <= MIN_SUPPORTED_JDK || feature > MAX_VERIFIED_JDK {
        return None;
    }
    let previous = release_profile(feature - 1)?;
    let current = release_profile(feature)?;
    Some(ReleaseDelta {
        from_feature: previous.feature,
        to_feature: current.feature,
        jni_interface_changed: previous.jni_interface_version != current.jni_interface_version,
        jvmti_interface_changed: previous.jvmti_interface_version
            != current.jvmti_interface_version,
        jni_function_prefix_changed: previous.jni_function_slots != current.jni_function_slots,
        jvmti_function_prefix_changed: previous.jvmti_function_slots
            != current.jvmti_function_slots,
        event_callback_prefix_changed: previous.event_callback_slots
            != current.event_callback_slots,
        changes: current.changes,
    })
}

/// Classify a Java feature release against this build's evidence boundary.
pub const fn runtime_support(feature: u16) -> RuntimeSupport {
    if feature < MIN_SUPPORTED_JDK {
        RuntimeSupport::Unsupported
    } else if feature <= MAX_VERIFIED_JDK {
        RuntimeSupport::Verified
    } else {
        RuntimeSupport::UnverifiedFuture
    }
}

/// Convert a JNI interface version to its Java feature release where defined.
pub const fn jni_version_feature(version: jni::jint) -> u16 {
    let major = ((version as u32) >> 16) as u16;
    let minor = (version as u32 & 0xffff) as u16;
    if major == 1 { minor } else { major }
}

/// Return the feature milestone encoded by a JVM TI interface version.
///
/// Java 8 reports the legacy JVM TI 1.2 line. JDK 10 still reports interface
/// milestone 9 and JDK 12 still reports milestone 11, so this is deliberately
/// not named `java_feature_release`.
pub const fn jvmti_interface_feature(version: jni::jint) -> u16 {
    if version & !0xff == jvmti::JVMTI_VERSION_1_2 {
        8
    } else {
        jvmti::version_feature(version)
    }
}
