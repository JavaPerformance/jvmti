use std::sync::atomic::{AtomicUsize, Ordering};

use jvmti_bindings::agent::AgentLoadContext;
use jvmti_bindings::sys::jni;
use jvmti_bindings::{__agent_on_load, Agent};

static PANICS: AtomicUsize = AtomicUsize::new(0);

#[derive(Default)]
struct PanickingAgent;

impl Agent for PanickingAgent {
    fn on_load(&self, _context: AgentLoadContext<'_>) -> jni::jint {
        panic!("lifecycle panic sentinel");
    }

    fn callback_panicked(&self, event: &'static str) {
        assert_eq!(event, "Agent_OnLoad");
        PANICS.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn lifecycle_panic_is_contained_at_the_ffi_boundary() {
    let result = unsafe {
        __agent_on_load::<PanickingAgent>(
            0x1111usize as *mut jni::JavaVM,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(result, jni::JNI_ERR);
    assert_eq!(PANICS.load(Ordering::Relaxed), 1);
}
