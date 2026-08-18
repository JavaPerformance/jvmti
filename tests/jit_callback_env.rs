use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use jvmti_bindings::agent::AgentLoadContext;
use jvmti_bindings::callbacks::{
    CallbackContext, CompiledMethodLoadEvent, CompiledMethodUnloadEvent, DynamicCodeGeneratedEvent,
    MethodExitEvent,
};
use jvmti_bindings::sys::jvmti;
use jvmti_bindings::{Agent, get_default_callbacks, jni, set_global_agent};

static LOAD_ENV: AtomicUsize = AtomicUsize::new(0);
static UNLOAD_ENV: AtomicUsize = AtomicUsize::new(0);
static DYNAMIC_CODE_ENV: AtomicUsize = AtomicUsize::new(0);
static METHOD_EXIT_ENV: AtomicUsize = AtomicUsize::new(0);
static METHOD_EXIT_VALUE: AtomicUsize = AtomicUsize::new(0);

struct RecordingAgent;

impl Agent for RecordingAgent {
    fn on_load(&self, _context: AgentLoadContext<'_>) -> jni::jint {
        jni::JNI_OK
    }

    fn compiled_method_load(
        &self,
        context: CallbackContext<'_>,
        _event: CompiledMethodLoadEvent<'_>,
    ) {
        LOAD_ENV.store(context.jvmti().raw() as usize, Ordering::Relaxed);
        assert!(context.jni().is_none());
    }

    fn compiled_method_unload(
        &self,
        context: CallbackContext<'_>,
        _event: CompiledMethodUnloadEvent,
    ) {
        UNLOAD_ENV.store(context.jvmti().raw() as usize, Ordering::Relaxed);
        assert!(context.jni().is_none());
    }

    fn dynamic_code_generated(
        &self,
        context: CallbackContext<'_>,
        _event: DynamicCodeGeneratedEvent<'_>,
    ) {
        DYNAMIC_CODE_ENV.store(context.jvmti().raw() as usize, Ordering::Relaxed);
        assert!(context.jni().is_none());
    }

    fn method_exit(&self, context: CallbackContext<'_>, event: MethodExitEvent) {
        METHOD_EXIT_ENV.store(context.jvmti().raw() as usize, Ordering::Relaxed);
        let value = event.return_value().expect("normal return has a value");
        METHOD_EXIT_VALUE.store(unsafe { value.j } as usize, Ordering::Relaxed);
    }
}

#[test]
fn trampolines_forward_context_and_complete_payloads() {
    set_global_agent(Box::new(RecordingAgent)).expect("global agent should be unset");

    let callback_env = ptr::NonNull::<jvmti::jvmtiEnv>::dangling().as_ptr();
    let jni_env = ptr::NonNull::<jni::JNIEnv>::dangling().as_ptr();
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
        callbacks.MethodExit.unwrap()(
            callback_env,
            jni_env,
            ptr::null_mut(),
            ptr::null_mut(),
            jni::JNI_FALSE,
            jni::jvalue { j: 0x1234 },
        );
    }

    let expected = callback_env as usize;
    assert_eq!(LOAD_ENV.load(Ordering::Relaxed), expected);
    assert_eq!(UNLOAD_ENV.load(Ordering::Relaxed), expected);
    assert_eq!(DYNAMIC_CODE_ENV.load(Ordering::Relaxed), expected);
    assert_eq!(METHOD_EXIT_ENV.load(Ordering::Relaxed), expected);
    assert_eq!(METHOD_EXIT_VALUE.load(Ordering::Relaxed), 0x1234);
}
