use std::ptr;

use jvmti_bindings::env::Jvmti;
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
