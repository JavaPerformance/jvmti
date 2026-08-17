//! Opt-in ABI comparison against an actual OpenJDK header set.
//!
//! Run through `scripts/check-jdk-abi.sh`. A normal `cargo test` skips this
//! external-header proof so crates.io consumers do not need a local JDK.

use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::mem::{align_of, size_of, MaybeUninit};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::addr_of;

use jvmti_bindings::sys::{jni, jvmti};
use jvmti_bindings::version::release_profile;

macro_rules! offset_of {
    ($ty:ty, $field:ident) => {{
        let value = MaybeUninit::<$ty>::uninit();
        let base = value.as_ptr();
        let field = unsafe { addr_of!((*base).$field) };
        field as usize - base as usize
    }};
}

fn rust_layout(feature: u16) -> HashMap<&'static str, usize> {
    let mut values = HashMap::new();
    let profile = release_profile(feature).expect("audited release profile");
    values.insert("version.JNI", profile.jni_interface_version as usize);
    values.insert("version.JVMTI", profile.jvmti_interface_version as usize);
    macro_rules! size {
        ($ty:ty, $name:literal) => {
            values.insert(concat!("size.", $name), size_of::<$ty>());
            values.insert(concat!("align.", $name), align_of::<$ty>());
        };
    }
    macro_rules! offset {
        ($ty:ty, $name:literal, $field:ident) => {
            values.insert(
                concat!("offset.", $name, ".", stringify!($field)),
                offset_of!($ty, $field),
            );
        };
    }

    size!(jvmti::jvmtiError, "jvmtiError");
    size!(jvmti::jvmtiTimerInfo, "jvmtiTimerInfo");
    offset!(jvmti::jvmtiTimerInfo, "jvmtiTimerInfo", kind);
    offset!(jvmti::jvmtiTimerInfo, "jvmtiTimerInfo", reserved1);
    offset!(jvmti::jvmtiTimerInfo, "jvmtiTimerInfo", reserved2);

    size!(jvmti::jvmtiStackInfo, "jvmtiStackInfo");
    offset!(jvmti::jvmtiStackInfo, "jvmtiStackInfo", thread);
    offset!(jvmti::jvmtiStackInfo, "jvmtiStackInfo", state);
    offset!(jvmti::jvmtiStackInfo, "jvmtiStackInfo", frame_buffer);
    offset!(jvmti::jvmtiStackInfo, "jvmtiStackInfo", frame_count);

    values.insert(
        "size.jvmtiHeapReferenceInfoField",
        size_of::<jvmti::jvmtiHeapReferenceInfoField>(),
    );
    values.insert(
        "size.jvmtiHeapReferenceInfoArray",
        size_of::<jvmti::jvmtiHeapReferenceInfoArray>(),
    );
    values.insert(
        "size.jvmtiHeapReferenceInfoConstantPool",
        size_of::<jvmti::jvmtiHeapReferenceInfoConstantPool>(),
    );
    values.insert(
        "size.jvmtiHeapReferenceInfoStackLocal",
        size_of::<jvmti::jvmtiHeapReferenceInfoStackLocal>(),
    );
    values.insert(
        "size.jvmtiHeapReferenceInfoJniLocal",
        size_of::<jvmti::jvmtiHeapReferenceInfoJniLocal>(),
    );
    values.insert(
        "size.jvmtiHeapReferenceInfoReserved",
        size_of::<jvmti::jvmtiHeapReferenceInfoReserved>(),
    );
    size!(jvmti::jvmtiHeapReferenceInfo, "jvmtiHeapReferenceInfo");

    size!(jvmti::jvmtiHeapCallbacks, "jvmtiHeapCallbacks");
    offset!(
        jvmti::jvmtiHeapCallbacks,
        "jvmtiHeapCallbacks",
        heap_iteration_callback
    );
    offset!(
        jvmti::jvmtiHeapCallbacks,
        "jvmtiHeapCallbacks",
        heap_reference_callback
    );
    offset!(
        jvmti::jvmtiHeapCallbacks,
        "jvmtiHeapCallbacks",
        primitive_field_callback
    );
    offset!(
        jvmti::jvmtiHeapCallbacks,
        "jvmtiHeapCallbacks",
        array_primitive_value_callback
    );
    offset!(
        jvmti::jvmtiHeapCallbacks,
        "jvmtiHeapCallbacks",
        string_primitive_value_callback
    );
    offset!(jvmti::jvmtiHeapCallbacks, "jvmtiHeapCallbacks", reserved15);

    size!(jvmti::jvmtiParamInfo, "jvmtiParamInfo");
    size!(
        jvmti::jvmtiExtensionFunctionInfo,
        "jvmtiExtensionFunctionInfo"
    );
    offset!(
        jvmti::jvmtiExtensionFunctionInfo,
        "jvmtiExtensionFunctionInfo",
        func
    );
    offset!(
        jvmti::jvmtiExtensionFunctionInfo,
        "jvmtiExtensionFunctionInfo",
        id
    );
    offset!(
        jvmti::jvmtiExtensionFunctionInfo,
        "jvmtiExtensionFunctionInfo",
        params
    );
    size!(jvmti::jvmtiExtensionEventInfo, "jvmtiExtensionEventInfo");
    offset!(
        jvmti::jvmtiExtensionEventInfo,
        "jvmtiExtensionEventInfo",
        extension_event_index
    );
    offset!(
        jvmti::jvmtiExtensionEventInfo,
        "jvmtiExtensionEventInfo",
        id
    );

    size!(jvmti::jvmtiCapabilities, "jvmtiCapabilities");
    size!(jvmti::jvmtiEventCallbacks, "jvmtiEventCallbacks");
    offset!(jvmti::jvmtiEventCallbacks, "jvmtiEventCallbacks", VMInit);
    offset!(
        jvmti::jvmtiEventCallbacks,
        "jvmtiEventCallbacks",
        MethodEntry
    );
    offset!(
        jvmti::jvmtiEventCallbacks,
        "jvmtiEventCallbacks",
        VMObjectAlloc
    );
    offset!(
        jvmti::jvmtiEventCallbacks,
        "jvmtiEventCallbacks",
        SampledObjectAlloc
    );
    offset!(
        jvmti::jvmtiEventCallbacks,
        "jvmtiEventCallbacks",
        VirtualThreadStart
    );
    offset!(
        jvmti::jvmtiEventCallbacks,
        "jvmtiEventCallbacks",
        VirtualThreadEnd
    );

    size!(jni::JNINativeInterface_, "struct JNINativeInterface_");
    offset!(
        jni::JNINativeInterface_,
        "struct JNINativeInterface_",
        GetVersion
    );
    offset!(
        jni::JNINativeInterface_,
        "struct JNINativeInterface_",
        GetObjectRefType
    );
    offset!(
        jni::JNINativeInterface_,
        "struct JNINativeInterface_",
        GetModule
    );
    offset!(
        jni::JNINativeInterface_,
        "struct JNINativeInterface_",
        IsVirtualThread
    );
    offset!(
        jni::JNINativeInterface_,
        "struct JNINativeInterface_",
        GetStringUTFLengthAsLong
    );
    offset!(
        jni::JNINativeInterface_,
        "struct JNINativeInterface_",
        HasIdentity
    );

    size!(jvmti::jvmtiInterface_1_, "struct jvmtiInterface_1_");
    offset!(
        jvmti::jvmtiInterface_1_,
        "struct jvmtiInterface_1_",
        SetEventNotificationMode
    );
    offset!(
        jvmti::jvmtiInterface_1_,
        "struct jvmtiInterface_1_",
        GetVersionNumber
    );
    offset!(
        jvmti::jvmtiInterface_1_,
        "struct jvmtiInterface_1_",
        SetEventCallbacks
    );
    offset!(
        jvmti::jvmtiInterface_1_,
        "struct jvmtiInterface_1_",
        GetAllModules
    );
    offset!(
        jvmti::jvmtiInterface_1_,
        "struct jvmtiInterface_1_",
        SuspendAllVirtualThreads
    );
    offset!(
        jvmti::jvmtiInterface_1_,
        "struct jvmtiInterface_1_",
        ResumeAllVirtualThreads
    );
    offset!(
        jvmti::jvmtiInterface_1_,
        "struct jvmtiInterface_1_",
        ClearAllFramePops
    );
    offset!(
        jvmti::jvmtiInterface_1_,
        "struct jvmtiInterface_1_",
        SetHeapSamplingInterval
    );

    values
}

fn capability_bytes(configure: impl FnOnce(&mut jvmti::jvmtiCapabilities)) -> String {
    let mut caps = jvmti::jvmtiCapabilities::default();
    configure(&mut caps);
    let bytes = unsafe {
        std::slice::from_raw_parts(
            &caps as *const _ as *const u8,
            size_of::<jvmti::jvmtiCapabilities>(),
        )
    };
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn compile_probe(include_dir: &Path, platform_include_dir: &Path, feature: u32) -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = manifest
        .join("target/abi-conformance")
        .join(format!("jdk-{feature}-{}", std::process::id()));
    fs::create_dir_all(&output_dir).expect("create ABI probe output directory");
    let executable = output_dir.join("jvmti-abi-probe");
    let source = manifest.join("tests/abi/jvmti_abi_probe.c");
    let compiler = env::var_os("CC").unwrap_or_else(|| OsString::from("cc"));
    let output = Command::new(compiler)
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg(format!("-DPROBE_JDK_FEATURE={feature}"))
        .arg("-I")
        .arg(include_dir)
        .arg("-I")
        .arg(platform_include_dir)
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("run C compiler for ABI probe");
    assert!(
        output.status.success(),
        "C ABI probe compilation failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

fn parse_probe(stdout: &[u8]) -> HashMap<String, String> {
    String::from_utf8(stdout.to_vec())
        .expect("ABI probe output must be UTF-8")
        .lines()
        .map(|line| {
            let (key, value) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("malformed ABI probe line: {line}"));
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

#[test]
fn raw_bindings_match_selected_openjdk_headers() {
    let Some(include_dir) = env::var_os("JVMTI_ABI_INCLUDE_DIR") else {
        eprintln!("skipped: run scripts/check-jdk-abi.sh for external-header proof");
        return;
    };
    let platform_include_dir = env::var_os("JVMTI_ABI_PLATFORM_INCLUDE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&include_dir).join("linux"));
    let feature: u32 = env::var("JVMTI_ABI_FEATURE")
        .expect("JVMTI_ABI_FEATURE accompanies JVMTI_ABI_INCLUDE_DIR")
        .parse()
        .expect("JVMTI_ABI_FEATURE must be an integer");

    let executable = compile_probe(Path::new(&include_dir), &platform_include_dir, feature);
    let output = Command::new(executable)
        .output()
        .expect("run compiled C ABI probe");
    assert!(output.status.success(), "C ABI probe execution failed");
    let c = parse_probe(&output.stdout);
    let rust = rust_layout(feature as u16);

    let profile = release_profile(feature as u16)
        .unwrap_or_else(|| panic!("JDK {feature} has no audited release profile"));
    let expected_prefixes = [
        ("size.struct JNINativeInterface_", profile.jni_table_bytes()),
        ("size.struct jvmtiInterface_1_", profile.jvmti_table_bytes()),
        ("size.jvmtiEventCallbacks", profile.event_callbacks_bytes()),
    ];
    for (key, expected) in expected_prefixes {
        let actual: usize = c[key]
            .parse()
            .unwrap_or_else(|_| panic!("{key} must be numeric"));
        assert_eq!(
            actual, expected,
            "{key}: audited JDK {feature} prefix is stale"
        );
    }

    for (key, c_value) in &c {
        if key.starts_with("capability.") {
            continue;
        }
        let c_value: usize = c_value.parse().expect("C layout metric must be numeric");
        let rust_value = rust
            .get(key.as_str())
            .unwrap_or_else(|| panic!("Rust probe does not define {key}"));

        // JNI, JVMTI, and event tables grow by appending or reclaiming reserved
        // slots. The latest Rust table may be longer than an older runtime's
        // table, but every field present in that runtime must retain its offset.
        let versioned_table_size = matches!(
            key.as_str(),
            "size.jvmtiEventCallbacks"
                | "size.struct JNINativeInterface_"
                | "size.struct jvmtiInterface_1_"
        );
        if versioned_table_size {
            assert!(
                *rust_value >= c_value,
                "{key}: latest Rust table {rust_value} is shorter than JDK {feature} table {c_value}"
            );
        } else {
            assert_eq!(
                *rust_value, c_value,
                "{key}: Rust/OpenJDK {feature} ABI mismatch"
            );
        }
    }

    let expected_caps = [
        (
            "can_tag_objects",
            capability_bytes(|caps| caps.set_can_tag_objects(true)),
        ),
        (
            "can_generate_method_entry_events",
            capability_bytes(|caps| caps.set_can_generate_method_entry_events(true)),
        ),
        (
            "can_generate_garbage_collection_events",
            capability_bytes(|caps| caps.set_can_generate_garbage_collection_events(true)),
        ),
        (
            "can_generate_early_vmstart",
            capability_bytes(|caps| caps.set_can_generate_early_vmstart(true)),
        ),
        (
            "can_generate_sampled_object_alloc_events",
            capability_bytes(|caps| caps.set_can_generate_sampled_object_alloc_events(true)),
        ),
        (
            "can_support_virtual_threads",
            capability_bytes(|caps| caps.set_can_support_virtual_threads(true)),
        ),
        (
            "can_support_value_objects",
            capability_bytes(|caps| caps.set_can_support_value_objects(true)),
        ),
    ];
    for (name, rust_bytes) in expected_caps {
        let key = format!("capability.{name}");
        if let Some(c_bytes) = c.get(&key) {
            assert_eq!(rust_bytes, *c_bytes, "{key}: raw bit encoding mismatch");
        }
    }
}
