use std::ffi::{c_char, c_void};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use jvmti_bindings::sys::jvmti;
use jvmti_bindings::{get_default_callbacks, jni, set_global_agent, Agent};

static LOAD_ENV: AtomicUsize = AtomicUsize::new(0);
static UNLOAD_ENV: AtomicUsize = AtomicUsize::new(0);
static DYNAMIC_CODE_ENV: AtomicUsize = AtomicUsize::new(0);

struct RecordingAgent;

impl Agent for RecordingAgent {
    fn on_load(&self, _vm: *mut jni::JavaVM, _options: &str) -> jni::jint {
        jni::JNI_OK
    }

    fn compiled_method_load_with_jvmti(
        &self,
        jvmti: *mut jvmti::jvmtiEnv,
        _method: jni::jmethodID,
        _code_size: jni::jint,
        _code_addr: *const c_void,
        _map_length: jni::jint,
        _map: *const c_void,
        _compile_info: *const c_void,
    ) {
        LOAD_ENV.store(jvmti as usize, Ordering::Relaxed);
    }

    fn compiled_method_unload_with_jvmti(
        &self,
        jvmti: *mut jvmti::jvmtiEnv,
        _method: jni::jmethodID,
        _code_addr: *const c_void,
    ) {
        UNLOAD_ENV.store(jvmti as usize, Ordering::Relaxed);
    }

    fn dynamic_code_generated_with_jvmti(
        &self,
        jvmti: *mut jvmti::jvmtiEnv,
        _name: *const c_char,
        _address: *const c_void,
        _length: jni::jint,
    ) {
        DYNAMIC_CODE_ENV.store(jvmti as usize, Ordering::Relaxed);
    }
}

#[test]
fn jit_trampolines_forward_the_callback_jvmti_environment() {
    set_global_agent(Box::new(RecordingAgent)).expect("global agent should be unset");

    let callback_env = ptr::NonNull::<jvmti::jvmtiEnv>::dangling().as_ptr();
    let callbacks = get_default_callbacks();

    unsafe {
        callbacks.CompiledMethodLoad.unwrap()(
            callback_env,
            ptr::null_mut(),
            0,
            ptr::null(),
            0,
            ptr::null(),
            ptr::null(),
        );
        callbacks.CompiledMethodUnload.unwrap()(callback_env, ptr::null_mut(), ptr::null());
        callbacks.DynamicCodeGenerated.unwrap()(callback_env, ptr::null(), ptr::null(), 0);
    }

    let expected = callback_env as usize;
    assert_eq!(LOAD_ENV.load(Ordering::Relaxed), expected);
    assert_eq!(UNLOAD_ENV.load(Ordering::Relaxed), expected);
    assert_eq!(DYNAMIC_CODE_ENV.load(Ordering::Relaxed), expected);
}

#[test]
fn jvmti_variants_default_to_the_existing_jit_callbacks() {
    struct LegacyAgent {
        calls: AtomicUsize,
    }

    impl Agent for LegacyAgent {
        fn on_load(&self, _vm: *mut jni::JavaVM, _options: &str) -> jni::jint {
            jni::JNI_OK
        }

        fn compiled_method_load(
            &self,
            _method: jni::jmethodID,
            _code_size: jni::jint,
            _code_addr: *const c_void,
            _map_length: jni::jint,
            _map: *const c_void,
            _compile_info: *const c_void,
        ) {
            self.calls.fetch_add(1, Ordering::Relaxed);
        }

        fn compiled_method_unload(&self, _method: jni::jmethodID, _code_addr: *const c_void) {
            self.calls.fetch_add(1, Ordering::Relaxed);
        }

        fn dynamic_code_generated(
            &self,
            _name: *const c_char,
            _address: *const c_void,
            _length: jni::jint,
        ) {
            self.calls.fetch_add(1, Ordering::Relaxed);
        }
    }

    let agent = LegacyAgent {
        calls: AtomicUsize::new(0),
    };
    Agent::compiled_method_load_with_jvmti(
        &agent,
        ptr::null_mut(),
        ptr::null_mut(),
        0,
        ptr::null(),
        0,
        ptr::null(),
        ptr::null(),
    );
    Agent::compiled_method_unload_with_jvmti(&agent, ptr::null_mut(), ptr::null_mut(), ptr::null());
    Agent::dynamic_code_generated_with_jvmti(&agent, ptr::null_mut(), ptr::null(), ptr::null(), 0);

    assert_eq!(agent.calls.load(Ordering::Relaxed), 3);
}
