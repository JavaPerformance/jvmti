use std::ptr;

use jvmti_bindings::env::{JniEnv, Jvmti};
use jvmti_bindings::sys::jvmti;
use jvmti_bindings::{describe_jni_result, jni};

#[test]
fn jvmti_new_rejects_null_vm_pointer() {
    let err = match Jvmti::new(ptr::null_mut()) {
        Ok(_) => panic!("null JavaVM must be rejected"),
        Err(err) => err,
    };
    assert_eq!(err, jni::JNI_ERR);
}

#[test]
fn jni_results_have_display_helpers() {
    assert_eq!(
        jni::describe_result(jni::JNI_EDETACHED),
        "JNI_EDETACHED (-2)"
    );
    assert_eq!(describe_jni_result(jni::JNI_EVERSION), "JNI_EVERSION (-3)");
}

#[test]
fn jvmti_errors_have_static_names() {
    assert_eq!(
        jvmti::error_name(jvmti::jvmtiError::MUST_POSSESS_CAPABILITY),
        "JVMTI_ERROR_MUST_POSSESS_CAPABILITY"
    );
}

#[test]
fn capability_presets_set_expected_bits() {
    let class_hook = jvmti::jvmtiCapabilities::for_class_file_load_hook();
    assert!(class_hook.can_generate_all_class_hook_events());

    let method_trace = jvmti::jvmtiCapabilities::for_method_trace();
    assert!(method_trace.can_generate_method_entry_events());
    assert!(method_trace.can_generate_method_exit_events());

    let exceptions = jvmti::jvmtiCapabilities::for_exceptions();
    assert!(exceptions.can_generate_exception_events());

    let heap_sampling = jvmti::jvmtiCapabilities::for_heap_sampling();
    assert!(heap_sampling.can_generate_sampled_object_alloc_events());
}

#[test]
fn jvmti_workflow_helpers_are_public_api() {
    let _ = Jvmti::set_default_agent_callbacks as fn(&Jvmti) -> Result<(), jvmti::jvmtiError>;
    let _ = Jvmti::add_class_file_load_hook_capabilities
        as fn(&Jvmti) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError>;
    let _ = Jvmti::add_method_trace_capabilities
        as fn(&Jvmti) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError>;
    let _ = Jvmti::add_exception_capabilities
        as fn(&Jvmti) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError>;
    let _ = Jvmti::add_heap_sampling_capabilities
        as fn(&Jvmti) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError>;
    let _ =
        Jvmti::enable_class_file_load_hook_events as fn(&Jvmti) -> Result<(), jvmti::jvmtiError>;
    let _ = Jvmti::enable_method_entry_exit_events as fn(&Jvmti) -> Result<(), jvmti::jvmtiError>;
    let _ = Jvmti::enable_exception_events as fn(&Jvmti) -> Result<(), jvmti::jvmtiError>;
    let _ = Jvmti::enable_heap_sampling_events as fn(&Jvmti) -> Result<(), jvmti::jvmtiError>;
    let _ = Jvmti::enable_vm_lifecycle_events as fn(&Jvmti) -> Result<(), jvmti::jvmtiError>;
    let _ =
        Jvmti::configure_class_file_load_hook_agent as fn(&Jvmti) -> Result<(), jvmti::jvmtiError>;
    let _ = Jvmti::configure_method_trace_agent as fn(&Jvmti) -> Result<(), jvmti::jvmtiError>;
    let _ = Jvmti::configure_exception_agent as fn(&Jvmti) -> Result<(), jvmti::jvmtiError>;
    let _ = Jvmti::configure_heap_sampling_agent as fn(&Jvmti) -> Result<(), jvmti::jvmtiError>;
    let _ = Jvmti::get_error_name_string
        as fn(&Jvmti, jvmti::jvmtiError) -> Result<String, jvmti::jvmtiError>;
}

#[test]
fn jni_classloader_and_module_helpers_are_public_api() {
    let _ = JniEnv::define_class as fn(&JniEnv, &str, jni::jobject, &[u8]) -> Option<jni::jclass>;
    let _ = JniEnv::class_loader_parent as fn(&JniEnv, jni::jobject) -> Option<jni::jobject>;
    let _ = JniEnv::system_class_loader as fn(&JniEnv) -> Option<jni::jobject>;
    let _ = JniEnv::module_name as fn(&JniEnv, jni::jobject) -> Option<String>;
    let _ = JniEnv::module_packages as fn(&JniEnv, jni::jobject) -> Option<Vec<String>>;
    let _ = JniEnv::module_class_loader as fn(&JniEnv, jni::jobject) -> Option<jni::jobject>;
    let _ = JniEnv::module_can_read as fn(&JniEnv, jni::jobject, jni::jobject) -> bool;
    let _ = JniEnv::module_is_exported_to as fn(&JniEnv, jni::jobject, &str, jni::jobject) -> bool;
    let _ = JniEnv::module_is_open_to as fn(&JniEnv, jni::jobject, &str, jni::jobject) -> bool;
}

#[test]
fn agent_jvmti_callback_variants_are_public_api() {
    struct ApiAgent;
    impl jvmti_bindings::Agent for ApiAgent {
        fn on_load(&self, _vm: *mut jni::JavaVM, _options: &str) -> jni::jint {
            jni::JNI_OK
        }
    }

    let agent = ApiAgent;
    jvmti_bindings::Agent::vm_init_with_jvmti(
        &agent,
        ptr::null_mut(),
        ptr::null_mut(),
        ptr::null_mut(),
    );
    jvmti_bindings::Agent::vm_death_with_jvmti(&agent, ptr::null_mut(), ptr::null_mut());
    jvmti_bindings::Agent::vm_start_with_jvmti(&agent, ptr::null_mut(), ptr::null_mut());
    jvmti_bindings::Agent::data_dump_request(&agent);
    jvmti_bindings::Agent::virtual_thread_start(&agent, ptr::null_mut(), ptr::null_mut());
    jvmti_bindings::Agent::virtual_thread_end(&agent, ptr::null_mut(), ptr::null_mut());
}
