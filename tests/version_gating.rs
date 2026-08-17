use std::ffi::c_char;
use std::ptr;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::Mutex;

use jvmti_bindings::env::{JniEnv, Jvmti};
use jvmti_bindings::sys::{jni, jvmti};
use jvmti_bindings::version::{
    jni_version_feature, jvmti_interface_feature, release_delta, release_profile, runtime_support,
    FeatureMaturity, JniFeature, JvmtiErrorAddition, JvmtiFeature, JvmtiSemanticChange,
    NativePolicyChange, NativeSourceChange, RuntimeChange, RuntimeSupport, MAX_VERIFIED_JDK,
    MIN_SUPPORTED_JDK, RELEASE_PROFILES,
};

static JNI_CALLS: AtomicUsize = AtomicUsize::new(0);
static JVMTI_CALLS: AtomicUsize = AtomicUsize::new(0);
static JVMTI_VERSION: AtomicI32 = AtomicI32::new(jvmti::JVMTI_VERSION_1_2);
static CALLBACK_TABLE_SIZE: AtomicI32 = AtomicI32::new(0);
static JVMTI_TEST_LOCK: Mutex<()> = Mutex::new(());

unsafe extern "system" fn jni_version_8(_env: *mut jni::JNIEnv) -> jni::jint {
    jni::JNI_VERSION_1_8
}
unsafe extern "system" fn jni_version_9(_env: *mut jni::JNIEnv) -> jni::jint {
    jni::JNI_VERSION_9
}
unsafe extern "system" fn jni_version_19(_env: *mut jni::JNIEnv) -> jni::jint {
    jni::JNI_VERSION_19
}
unsafe extern "system" fn jni_version_24(_env: *mut jni::JNIEnv) -> jni::jint {
    jni::JNI_VERSION_24
}
unsafe extern "system" fn jni_version_28(_env: *mut jni::JNIEnv) -> jni::jint {
    jni::JNI_VERSION_28
}

unsafe extern "system" fn get_module(_env: *mut jni::JNIEnv, _class: jni::jclass) -> jni::jobject {
    JNI_CALLS.fetch_add(1, Ordering::Relaxed);
    0x233usize as jni::jobject
}

unsafe extern "system" fn is_virtual_thread(
    _env: *mut jni::JNIEnv,
    _object: jni::jobject,
) -> jni::jboolean {
    JNI_CALLS.fetch_add(1, Ordering::Relaxed);
    jni::JNI_TRUE
}

unsafe extern "system" fn get_string_utf_length_as_long(
    _env: *mut jni::JNIEnv,
    _string: jni::jstring,
) -> jni::jlong {
    JNI_CALLS.fetch_add(1, Ordering::Relaxed);
    0x235
}

unsafe extern "system" fn has_identity(
    _env: *mut jni::JNIEnv,
    _object: jni::jobject,
) -> jni::jboolean {
    JNI_CALLS.fetch_add(1, Ordering::Relaxed);
    jni::JNI_TRUE
}

fn mock_jni_env(
    version: unsafe extern "system" fn(*mut jni::JNIEnv) -> jni::jint,
) -> (Vec<usize>, jni::JNIEnv) {
    let mut slots = vec![0usize; 237];
    slots[4] = version as usize;
    slots[233] = get_module as *const () as usize;
    slots[234] = is_virtual_thread as *const () as usize;
    slots[235] = get_string_utf_length_as_long as *const () as usize;
    slots[236] = has_identity as *const () as usize;
    let table = slots.as_ptr() as *const jni::JNINativeInterface_;
    (slots, table)
}

#[test]
fn every_jni_tail_operation_is_gated_before_its_slot_is_used() {
    JNI_CALLS.store(0, Ordering::Relaxed);
    let (slots, mut raw_env) = mock_jni_env(jni_version_8);
    let env = unsafe { JniEnv::from_raw(&mut raw_env) };

    // Null sentinel handles are consumed only by the local mock functions.
    let cases = unsafe {
        [
            (
                env.get_module(ptr::null_mut()).unwrap_err(),
                JniFeature::Modules,
            ),
            (
                env.is_virtual_thread(ptr::null_mut()).unwrap_err(),
                JniFeature::VirtualThreads,
            ),
            (
                env.get_string_utf_length_as_long(ptr::null_mut())
                    .unwrap_err(),
                JniFeature::ModifiedUtf8LongLength,
            ),
            (
                env.has_identity(ptr::null_mut()).unwrap_err(),
                JniFeature::ValueObjectIdentity,
            ),
        ]
    };
    for (error, feature) in cases {
        assert_eq!(error.actual, jni::JNI_VERSION_1_8);
        assert_eq!(error.required, feature.required_version());
    }
    assert_eq!(JNI_CALLS.load(Ordering::Relaxed), 0);
    std::hint::black_box(slots);
}

#[test]
fn each_jni_tail_operation_works_at_its_introducing_release() {
    JNI_CALLS.store(0, Ordering::Relaxed);

    let (slots9, mut raw9) = mock_jni_env(jni_version_9);
    let env9 = unsafe { JniEnv::from_raw(&mut raw9) };
    assert_eq!(
        unsafe { env9.get_module(ptr::null_mut()) }.unwrap() as usize,
        0x233
    );

    let (slots19, mut raw19) = mock_jni_env(jni_version_19);
    let env19 = unsafe { JniEnv::from_raw(&mut raw19) };
    assert!(unsafe { env19.is_virtual_thread(ptr::null_mut()) }.unwrap());

    let (slots24, mut raw24) = mock_jni_env(jni_version_24);
    let env24 = unsafe { JniEnv::from_raw(&mut raw24) };
    assert_eq!(
        unsafe { env24.get_string_utf_length_as_long(ptr::null_mut()) }.unwrap(),
        0x235
    );

    let (slots28, mut raw28) = mock_jni_env(jni_version_28);
    let env28 = unsafe { JniEnv::from_raw(&mut raw28) };
    assert!(unsafe { env28.has_identity(ptr::null_mut()) }.unwrap());

    assert_eq!(JNI_CALLS.load(Ordering::Relaxed), 4);
    std::hint::black_box((slots9, slots19, slots24, slots28));
}

unsafe extern "system" fn get_jvmti_version(
    _env: *mut jvmti::jvmtiEnv,
    version: *mut jni::jint,
) -> jvmti::jvmtiError {
    unsafe { *version = JVMTI_VERSION.load(Ordering::Relaxed) };
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn get_all_modules(
    _env: *mut jvmti::jvmtiEnv,
    count: *mut jni::jint,
    modules: *mut *mut jni::jobject,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    unsafe {
        *count = 0;
        *modules = ptr::null_mut();
    }
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn get_named_module(
    _env: *mut jvmti::jvmtiEnv,
    _loader: jni::jobject,
    _package: *const c_char,
    module: *mut jni::jobject,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    unsafe { *module = 0x240usize as jni::jobject };
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn set_heap_sampling_interval(
    _env: *mut jvmti::jvmtiEnv,
    _interval: jni::jint,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn virtual_thread_control(
    _env: *mut jvmti::jvmtiEnv,
    _count: jni::jint,
    _exceptions: *const jni::jthread,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn clear_all_frame_pops(
    _env: *mut jvmti::jvmtiEnv,
    _thread: jni::jthread,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn add_capabilities(
    _env: *mut jvmti::jvmtiEnv,
    _capabilities: *const jvmti::jvmtiCapabilities,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn get_potential_capabilities(
    _env: *mut jvmti::jvmtiEnv,
    capabilities: *mut jvmti::jvmtiCapabilities,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    unsafe { (*capabilities).set_can_support_value_objects(true) };
    jvmti::jvmtiError::NONE
}

unsafe extern "C" fn set_event_notification_mode(
    _env: *mut jvmti::jvmtiEnv,
    _mode: jni::jint,
    _event_type: jni::jint,
    _thread: jni::jthread,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn add_module_reads(
    _env: *mut jvmti::jvmtiEnv,
    _module: jni::jobject,
    _source_module: jni::jobject,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn add_module_package(
    _env: *mut jvmti::jvmtiEnv,
    _module: jni::jobject,
    _package: *const c_char,
    _to_module: jni::jobject,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn add_module_uses(
    _env: *mut jvmti::jvmtiEnv,
    _module: jni::jobject,
    _service: jni::jclass,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn add_module_provides(
    _env: *mut jvmti::jvmtiEnv,
    _module: jni::jobject,
    _service: jni::jclass,
    _implementation: jni::jclass,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn is_modifiable_module(
    _env: *mut jvmti::jvmtiEnv,
    _module: jni::jobject,
    result: *mut jni::jboolean,
) -> jvmti::jvmtiError {
    JVMTI_CALLS.fetch_add(1, Ordering::Relaxed);
    unsafe { *result = jni::JNI_TRUE };
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn set_event_callbacks(
    _env: *mut jvmti::jvmtiEnv,
    _callbacks: *const jvmti::jvmtiEventCallbacks,
    size: jni::jint,
) -> jvmti::jvmtiError {
    CALLBACK_TABLE_SIZE.store(size, Ordering::Relaxed);
    jvmti::jvmtiError::NONE
}

fn mock_jvmti_slots() -> Vec<usize> {
    let mut slots = vec![0usize; 156];
    slots[1] = set_event_notification_mode as *const () as usize; // Function 2.
    slots[2] = get_all_modules as *const () as usize; // Function 3.
    slots[39] = get_named_module as *const () as usize; // Function 40.
    slots[66] = clear_all_frame_pops as *const () as usize; // Function 67.
    slots[87] = get_jvmti_version as *const () as usize; // Function 88.
    slots[93] = add_module_reads as *const () as usize; // Function 94.
    slots[94] = add_module_package as *const () as usize; // Function 95.
    slots[95] = add_module_package as *const () as usize; // Function 96.
    slots[96] = add_module_uses as *const () as usize; // Function 97.
    slots[97] = add_module_provides as *const () as usize; // Function 98.
    slots[98] = is_modifiable_module as *const () as usize; // Function 99.
    slots[117] = virtual_thread_control as *const () as usize; // Function 118.
    slots[118] = virtual_thread_control as *const () as usize; // Function 119.
    slots[121] = set_event_callbacks as *const () as usize; // Function 122.
    slots[139] = get_potential_capabilities as *const () as usize; // Function 140.
    slots[141] = add_capabilities as *const () as usize; // Function 142.
    slots[155] = set_heap_sampling_interval as *const () as usize; // Function 156.
    slots
}

fn with_mock_jvmti<T>(version: jni::jint, f: impl FnOnce(&Jvmti) -> T) -> T {
    JVMTI_VERSION.store(version, Ordering::Relaxed);
    let slots = mock_jvmti_slots();
    let mut raw_env = jvmti::jvmtiEnv {
        functions: slots.as_ptr() as *const jvmti::jvmtiInterface_1_,
    };
    let env = unsafe { Jvmti::from_raw(&mut raw_env) };
    let value = f(&env);
    std::hint::black_box(slots);
    value
}

#[test]
fn every_jvmti_addition_is_gated_on_an_older_runtime() {
    let _guard = JVMTI_TEST_LOCK.lock().unwrap();
    JVMTI_CALLS.store(0, Ordering::Relaxed);
    with_mock_jvmti(jvmti::JVMTI_VERSION_1_2, |env| unsafe {
        assert_eq!(
            env.get_all_modules().unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.get_named_module(ptr::null_mut(), "pkg").unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.is_modifiable_module(ptr::null_mut()).unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.add_module_reads(ptr::null_mut(), ptr::null_mut())
                .unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.add_module_exports(ptr::null_mut(), "pkg", ptr::null_mut())
                .unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.add_module_opens(ptr::null_mut(), "pkg", ptr::null_mut())
                .unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.add_module_uses(ptr::null_mut(), ptr::null_mut())
                .unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.add_module_provides(ptr::null_mut(), ptr::null_mut(), ptr::null_mut())
                .unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.set_heap_sampling_interval(1).unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.enable_heap_sampling_events().unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.suspend_all_virtual_threads(&[]).unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.resume_all_virtual_threads(&[]).unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.enable_virtual_thread_events().unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.clear_all_frame_pops(ptr::null_mut()).unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.add_value_object_capabilities().unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
    });
    assert_eq!(JVMTI_CALLS.load(Ordering::Relaxed), 0);
}

#[test]
fn each_jvmti_function_works_at_its_introducing_release() {
    let _guard = JVMTI_TEST_LOCK.lock().unwrap();
    JVMTI_CALLS.store(0, Ordering::Relaxed);

    with_mock_jvmti(jvmti::version_for_feature(9), |env| unsafe {
        assert!(env.get_all_modules().unwrap().is_empty());
        assert_eq!(
            env.get_named_module(ptr::null_mut(), "pkg").unwrap() as usize,
            0x240
        );
        assert!(env.is_modifiable_module(ptr::null_mut()).unwrap());
        env.add_module_reads(ptr::null_mut(), ptr::null_mut())
            .unwrap();
        env.add_module_exports(ptr::null_mut(), "pkg", ptr::null_mut())
            .unwrap();
        env.add_module_opens(ptr::null_mut(), "pkg", ptr::null_mut())
            .unwrap();
        env.add_module_uses(ptr::null_mut(), ptr::null_mut())
            .unwrap();
        env.add_module_provides(ptr::null_mut(), ptr::null_mut(), ptr::null_mut())
            .unwrap();
    });
    with_mock_jvmti(jvmti::version_for_feature(11), |env| {
        env.set_heap_sampling_interval(1).unwrap();
        env.enable_heap_sampling_events().unwrap();
    });
    with_mock_jvmti(jvmti::version_for_feature(19), |env| unsafe {
        env.suspend_all_virtual_threads(&[]).unwrap();
        env.resume_all_virtual_threads(&[]).unwrap();
        env.enable_virtual_thread_events().unwrap();
    });
    with_mock_jvmti(jvmti::version_for_feature(25), |env| unsafe {
        env.clear_all_frame_pops(ptr::null_mut()).unwrap();
    });
    with_mock_jvmti(jvmti::version_for_feature(28), |env| {
        env.add_value_object_capabilities().unwrap();
    });

    assert_eq!(JVMTI_CALLS.load(Ordering::Relaxed), 17);
}

#[test]
fn versioned_capability_bits_are_rejected_before_add_capabilities() {
    let _guard = JVMTI_TEST_LOCK.lock().unwrap();
    let cases = [
        (
            JvmtiFeature::Modules,
            jvmti::JVMTI_VERSION_1_2,
            (|caps: &mut jvmti::jvmtiCapabilities| caps.set_can_generate_early_vmstart(true))
                as fn(&mut jvmti::jvmtiCapabilities),
        ),
        (
            JvmtiFeature::Modules,
            jvmti::JVMTI_VERSION_1_2,
            (|caps: &mut jvmti::jvmtiCapabilities| {
                caps.set_can_generate_early_class_hook_events(true)
            }) as fn(&mut jvmti::jvmtiCapabilities),
        ),
        (
            JvmtiFeature::HeapSampling,
            jvmti::version_for_feature(9),
            (|caps: &mut jvmti::jvmtiCapabilities| {
                caps.set_can_generate_sampled_object_alloc_events(true)
            }) as fn(&mut jvmti::jvmtiCapabilities),
        ),
        (
            JvmtiFeature::VirtualThreads,
            jvmti::version_for_feature(11),
            (|caps: &mut jvmti::jvmtiCapabilities| caps.set_can_support_virtual_threads(true))
                as fn(&mut jvmti::jvmtiCapabilities),
        ),
        (
            JvmtiFeature::ValueObjects,
            jvmti::version_for_feature(27),
            (|caps: &mut jvmti::jvmtiCapabilities| caps.set_can_support_value_objects(true))
                as fn(&mut jvmti::jvmtiCapabilities),
        ),
    ];

    for (feature, old_version, set) in cases {
        JVMTI_CALLS.store(0, Ordering::Relaxed);
        with_mock_jvmti(old_version, |env| {
            let mut caps = jvmti::jvmtiCapabilities::default();
            set(&mut caps);
            assert_eq!(
                env.add_capabilities(&caps).unwrap_err(),
                jvmti::jvmtiError::NOT_AVAILABLE,
                "{} must be gated",
                feature.operation()
            );
            assert_eq!(
                env.relinquish_capabilities(&caps).unwrap_err(),
                jvmti::jvmtiError::NOT_AVAILABLE,
                "{} relinquish must be gated",
                feature.operation()
            );
        });
        assert_eq!(JVMTI_CALLS.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn jvmti_interface_version_encoding_preserves_every_audited_milestone() {
    assert_eq!(jvmti::version_for_feature(8), jvmti::JVMTI_VERSION_1_2 | 1);
    assert_eq!(jvmti_interface_feature(jvmti::JVMTI_VERSION_1_2 | 1), 8);
    assert_eq!(jvmti::version_for_feature(10), jvmti::JVMTI_VERSION_9);
    assert_eq!(jvmti::version_for_feature(12), jvmti::JVMTI_VERSION_11);
    for feature in 13..=29 {
        let version = jvmti::version_for_feature(feature) | 0x0000_1234;
        assert_eq!(jvmti_interface_feature(version), feature);
    }
}

#[test]
#[should_panic(expected = "JDK feature predates the supported version model")]
fn jvmti_interface_version_rejects_pre_contract_releases() {
    let _ = jvmti::version_for_feature(7);
}

#[test]
fn callback_registration_uses_the_runtime_specific_table_prefix() {
    let _guard = JVMTI_TEST_LOCK.lock().unwrap();
    for (feature, expected_slots) in [(8, 35), (11, 37), (18, 37), (19, 39), (28, 39)] {
        CALLBACK_TABLE_SIZE.store(0, Ordering::Relaxed);
        with_mock_jvmti(jvmti::version_for_feature(feature), |env| {
            env.set_event_callbacks(jvmti::jvmtiEventCallbacks::default())
                .unwrap();
        });
        assert_eq!(
            CALLBACK_TABLE_SIZE.load(Ordering::Relaxed) as usize,
            expected_slots * std::mem::size_of::<Option<jvmti::JvmtiEventReservedFn>>(),
            "wrong callback prefix for JDK {feature}"
        );
    }
}

#[test]
fn versioned_events_are_rejected_before_the_runtime_call() {
    let _guard = JVMTI_TEST_LOCK.lock().unwrap();
    JVMTI_CALLS.store(0, Ordering::Relaxed);
    with_mock_jvmti(jvmti::version_for_feature(8), |env| unsafe {
        assert_eq!(
            env.enable_event(jvmti::JVMTI_EVENT_SAMPLED_OBJECT_ALLOC, ptr::null_mut())
                .unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
        assert_eq!(
            env.enable_event(jvmti::JVMTI_EVENT_VIRTUAL_THREAD_START, ptr::null_mut())
                .unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
    });
    assert_eq!(JVMTI_CALLS.load(Ordering::Relaxed), 0);

    with_mock_jvmti(jvmti::version_for_feature(11), |env| unsafe {
        env.enable_event(jvmti::JVMTI_EVENT_SAMPLED_OBJECT_ALLOC, ptr::null_mut())
            .unwrap();
        assert_eq!(
            env.enable_event(jvmti::JVMTI_EVENT_VIRTUAL_THREAD_END, ptr::null_mut())
                .unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
    });
    assert_eq!(JVMTI_CALLS.load(Ordering::Relaxed), 1);
}

#[test]
fn absent_function_slots_and_invalid_environments_fail_closed() {
    let _guard = JVMTI_TEST_LOCK.lock().unwrap();
    with_mock_jvmti(jvmti::JVMTI_VERSION_1_2, |env| {
        assert_eq!(
            env.get_current_thread().unwrap_err(),
            jvmti::jvmtiError::NOT_AVAILABLE
        );
    });

    let env = unsafe { Jvmti::from_raw(ptr::null_mut()) };
    assert_eq!(
        env.get_version_number().unwrap_err(),
        jvmti::jvmtiError::INVALID_ENVIRONMENT
    );
}

#[test]
fn every_supported_release_has_an_exact_contiguous_profile() {
    assert_eq!(RELEASE_PROFILES.len(), 21);
    for feature in MIN_SUPPORTED_JDK..=MAX_VERIFIED_JDK {
        let profile = release_profile(feature).expect("supported release profile");
        assert_eq!(profile.feature, feature);
        let expected_interface_feature = match feature {
            8 => 8,
            9 | 10 => 9,
            11 | 12 => 11,
            _ => feature,
        };
        assert_eq!(
            jvmti_interface_feature(profile.jvmti_interface_version),
            expected_interface_feature
        );
        assert!(profile.jni_function_slots >= 233);
        assert!(profile.jvmti_function_slots >= 155);
        assert!(matches!(profile.event_callback_slots, 35 | 37 | 39));
    }
    assert_eq!(release_profile(MIN_SUPPORTED_JDK - 1), None);
    assert_eq!(release_profile(MAX_VERIFIED_JDK + 1), None);
    assert_eq!(runtime_support(7), RuntimeSupport::Unsupported);
    assert_eq!(runtime_support(28), RuntimeSupport::Verified);
    assert_eq!(runtime_support(29), RuntimeSupport::UnverifiedFuture);
}

#[test]
fn release_profiles_capture_each_native_table_growth_boundary() {
    let expected = [
        (8, 233, 155, 35),
        (9, 234, 155, 35),
        (10, 234, 155, 35),
        (11, 234, 156, 37),
        (18, 234, 156, 37),
        (19, 235, 156, 39),
        (23, 235, 156, 39),
        (24, 236, 156, 39),
        (27, 236, 156, 39),
        (28, 237, 156, 39),
    ];
    for (feature, jni_slots, jvmti_slots, callback_slots) in expected {
        let profile = release_profile(feature).unwrap();
        assert_eq!(profile.jni_function_slots, jni_slots, "JDK {feature}");
        assert_eq!(profile.jvmti_function_slots, jvmti_slots, "JDK {feature}");
        assert_eq!(
            profile.event_callback_slots, callback_slots,
            "JDK {feature}"
        );
    }
}

#[test]
fn release_profiles_capture_jni_interface_revision_boundaries() {
    let expected = [
        (8, jni::JNI_VERSION_1_8),
        (9, jni::JNI_VERSION_9),
        (10, jni::JNI_VERSION_10),
        (18, jni::JNI_VERSION_10),
        (19, jni::JNI_VERSION_19),
        (20, jni::JNI_VERSION_20),
        (21, jni::JNI_VERSION_21),
        (23, jni::JNI_VERSION_21),
        (24, jni::JNI_VERSION_24),
        (27, jni::JNI_VERSION_24),
        (28, jni::JNI_VERSION_28),
    ];
    for (feature, interface_version) in expected {
        assert_eq!(
            release_profile(feature).unwrap().jni_interface_version,
            interface_version,
            "JDK {feature}"
        );
    }
}

#[test]
fn release_profiles_capture_exact_jvmti_interface_revisions() {
    for feature in MIN_SUPPORTED_JDK..=MAX_VERIFIED_JDK {
        let expected = match feature {
            8 => jvmti::JVMTI_VERSION_1_2 | 1,
            9 | 10 => jvmti::JVMTI_VERSION_9,
            11 | 12 => jvmti::JVMTI_VERSION_11,
            _ => {
                jvmti::JVMTI_VERSION_INTERFACE_JVMTI
                    | ((feature as jni::jint) << jvmti::JVMTI_VERSION_SHIFT_MAJOR)
            }
        };
        assert_eq!(
            release_profile(feature).unwrap().jvmti_interface_version,
            expected,
            "JDK {feature}"
        );
    }
}

#[test]
fn adjacent_release_deltas_cover_every_structural_axis() {
    let jni_interface = [9, 10, 19, 20, 21, 24, 28];
    let jni_prefix = [9, 19, 24, 28];
    let jvmti_prefix = [11];
    let callback_prefix = [11, 19];

    assert_eq!(release_delta(8), None);
    assert_eq!(release_delta(29), None);
    for feature in 9..=28 {
        let delta = release_delta(feature).expect("adjacent audited release delta");
        assert_eq!(delta.from_feature, feature - 1);
        assert_eq!(delta.to_feature, feature);
        assert_eq!(
            delta.jni_interface_changed,
            jni_interface.contains(&feature),
            "JNI interface delta at JDK {feature}"
        );
        assert_eq!(
            delta.jvmti_interface_changed,
            feature == 9 || feature == 11 || feature >= 13,
            "JVM TI interface delta at JDK {feature}"
        );
        assert_eq!(
            delta.jni_function_prefix_changed,
            jni_prefix.contains(&feature),
            "JNI table delta at JDK {feature}"
        );
        assert_eq!(
            delta.jvmti_function_prefix_changed,
            jvmti_prefix.contains(&feature),
            "JVM TI table delta at JDK {feature}"
        );
        assert_eq!(
            delta.event_callback_prefix_changed,
            callback_prefix.contains(&feature),
            "callback table delta at JDK {feature}"
        );
        assert_eq!(delta.changes, release_profile(feature).unwrap().changes);
    }
}

#[test]
fn preview_and_final_transitions_are_not_inferred_past_the_evidence() {
    assert_eq!(
        JniFeature::VirtualThreads.maturity_on(18),
        FeatureMaturity::Unavailable
    );
    assert_eq!(
        JniFeature::VirtualThreads.maturity_on(19),
        FeatureMaturity::Preview
    );
    assert_eq!(
        JvmtiFeature::VirtualThreads.maturity_on(21),
        FeatureMaturity::Permanent
    );
    assert_eq!(
        JvmtiFeature::ValueObjects.maturity_on(28),
        FeatureMaturity::Preview
    );
    assert_eq!(
        JvmtiFeature::ValueObjects.maturity_on(29),
        FeatureMaturity::UnverifiedFuture
    );
}

#[test]
fn semantic_only_release_changes_are_explicit() {
    let jdk9 = release_profile(9).unwrap();
    assert!(jdk9.changes.contains(&RuntimeChange::JvmtiSemantic(
        JvmtiSemanticChange::ImplementationDefinedClassesMayBeUnmodifiable
    )));
    let jdk13 = release_profile(13).unwrap();
    assert!(jdk13.changes.contains(&RuntimeChange::JvmtiSemantic(
        JvmtiSemanticChange::PopFrameAllowsCurrentThread
    )));
    let jdk17 = release_profile(17).unwrap();
    assert!(jdk17.changes.contains(&RuntimeChange::JvmtiSemantic(
        JvmtiSemanticChange::LegacyHeapFunctionsDeprecated
    )));
    let jdk28 = release_profile(28).unwrap();
    assert!(jdk28.changes.contains(&RuntimeChange::JvmtiSemantic(
        JvmtiSemanticChange::ValueAllocationObjectMayBeNull
    )));
    assert!(release_profile(11)
        .unwrap()
        .changes
        .contains(&RuntimeChange::JvmtiError(
            JvmtiErrorAddition::UnsupportedRedefinitionClassAttributeChanged
        )));
    assert!(release_profile(19)
        .unwrap()
        .changes
        .contains(&RuntimeChange::JvmtiError(
            JvmtiErrorAddition::UnsupportedOperation
        )));

    assert!(release_profile(16)
        .unwrap()
        .changes
        .contains(&RuntimeChange::NativeSource(
            NativeSourceChange::NamedJvmtiStructureTags
        )));
    assert!(release_profile(21)
        .unwrap()
        .changes
        .contains(&RuntimeChange::NativePolicy(
            NativePolicyChange::DynamicAgentLoadingWarns
        )));
    assert!(release_profile(24)
        .unwrap()
        .changes
        .contains(&RuntimeChange::NativePolicy(
            NativePolicyChange::NativeLibraryLoadingRequiresEnabledAccess
        )));
    assert!(release_profile(24)
        .unwrap()
        .changes
        .contains(&RuntimeChange::NativePolicy(
            NativePolicyChange::TransformingAgentsCanInvalidateAotCache
        )));
    assert!(release_profile(26)
        .unwrap()
        .changes
        .contains(&RuntimeChange::NativePolicy(
            NativePolicyChange::JniFinalFieldMutationDiagnostics
        )));
}

#[test]
fn jni_versions_map_to_java_feature_releases() {
    assert_eq!(jni_version_feature(jni::JNI_VERSION_1_8), 8);
    assert_eq!(jni_version_feature(jni::JNI_VERSION_9), 9);
    assert_eq!(jni_version_feature(jni::JNI_VERSION_19), 19);
    assert_eq!(jni_version_feature(jni::JNI_VERSION_24), 24);
    assert_eq!(jni_version_feature(jni::JNI_VERSION_28), 28);
}
