use std::ffi::c_void;
use std::ptr::{self, NonNull};

use jvmti_bindings::agent::AgentLoadContext;
use jvmti_bindings::sys::{jni, jvmti};
use jvmti_bindings::{Agent, get_default_callbacks, set_global_agent};

struct DefaultNoOpAgent;

impl Agent for DefaultNoOpAgent {
    fn on_load(&self, _context: AgentLoadContext<'_>) -> jni::jint {
        jni::JNI_OK
    }
}

#[test]
fn absent_and_default_agents_preserve_the_vm_selected_native_address() {
    let callback = get_default_callbacks()
        .NativeMethodBind
        .expect("NativeMethodBind callback must be installed");
    let env = NonNull::<jvmti::jvmtiEnv>::dangling().as_ptr();
    let original = 0x1234usize as *mut c_void;

    let mut without_agent = original;
    unsafe {
        callback(
            env,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            original,
            &mut without_agent,
        )
    };
    assert_eq!(without_agent, original);

    set_global_agent(Box::new(DefaultNoOpAgent)).expect("global agent should be unset");
    let mut with_default_handler = original;
    unsafe {
        callback(
            env,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            original,
            &mut with_default_handler,
        )
    };
    assert_eq!(with_default_handler, original);
}
