use jvmti_bindings::prelude::*;

#[derive(Default)]
struct MyAgent;

impl Agent for MyAgent {
    fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
        eprintln!("[agent] on_load: {}", options);

        let jvmti = match Jvmti::new(vm) {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[agent] Failed to get JVMTI: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        if let Err(e) = jvmti.add_capabilities_with(|caps| {
            caps.set_can_generate_all_class_hook_events(true);
        }) {
            eprintln!("[agent] Failed to add capabilities: {:?}", e);
            return jni::JNI_ERR;
        }

        let callbacks = get_default_callbacks();
        if let Err(e) = jvmti.set_event_callbacks(callbacks) {
            eprintln!("[agent] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti.enable_events_global(&[jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK]) {
            eprintln!("[agent] Failed to enable events: {:?}", e);
            return jni::JNI_ERR;
        }

        jni::JNI_OK
    }

    fn class_file_load_hook(
        &self,
        _jni: *mut jni::JNIEnv,
        _class_being_redefined: jni::jclass,
        _loader: jni::jobject,
        name: *const std::os::raw::c_char,
        _protection_domain: jni::jobject,
        class_data_len: jni::jint,
        _class_data: *const u8,
        _new_class_data_len: *mut jni::jint,
        _new_class_data: *mut *mut u8,
    ) {
        let class_name = if name.is_null() {
            "<unknown>".to_string()
        } else {
            unsafe { std::ffi::CStr::from_ptr(name) }
                .to_str()
                .unwrap_or("<invalid>")
                .to_string()
        };

        eprintln!("[agent] Loaded: {} ({} bytes)", class_name, class_data_len);
    }
}

export_agent!(MyAgent);
