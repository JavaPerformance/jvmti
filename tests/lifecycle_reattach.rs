use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicUsize, Ordering};

use jvmti_bindings::agent::AgentLoadContext;
use jvmti_bindings::sys::jni;
use jvmti_bindings::{__agent_on_attach, __agent_on_load, Agent};

static CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);
static LOADS: AtomicUsize = AtomicUsize::new(0);
static ATTACHES: AtomicUsize = AtomicUsize::new(0);

struct ReattachAgent;

impl Default for ReattachAgent {
    fn default() -> Self {
        CONSTRUCTIONS.fetch_add(1, Ordering::Relaxed);
        Self
    }
}

impl Agent for ReattachAgent {
    fn on_load(&self, _context: AgentLoadContext<'_>) -> jni::jint {
        LOADS.fetch_add(1, Ordering::Relaxed);
        11
    }

    fn on_attach(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let index = ATTACHES.fetch_add(1, Ordering::Relaxed);
        let expected = ["attach=1", "nul=\0,rocket=\u{1f680}"][index];
        assert_eq!(context.options_str().unwrap().as_deref(), Some(expected));
        22
    }
}

fn invoke_attach(vm: *mut jni::JavaVM, options: &str) -> jni::jint {
    let mut options = jvmti_bindings::mutf8::encode_cstring(options).into_bytes_with_nul();
    unsafe {
        __agent_on_attach::<ReattachAgent>(
            vm,
            options.as_mut_ptr().cast::<c_char>(),
            std::ptr::null_mut::<c_void>(),
        )
    }
}

#[test]
fn startup_agent_accepts_repeated_dynamic_attach_without_reconstruction() {
    let vm = 0x1111usize as *mut jni::JavaVM;
    let result =
        unsafe { __agent_on_load::<ReattachAgent>(vm, std::ptr::null_mut(), std::ptr::null_mut()) };
    assert_eq!(result, 11);
    assert_eq!(invoke_attach(vm, "attach=1"), 22);
    assert_eq!(invoke_attach(vm, "nul=\0,rocket=\u{1f680}"), 22);
    assert_eq!(CONSTRUCTIONS.load(Ordering::Relaxed), 1);
    assert_eq!(LOADS.load(Ordering::Relaxed), 1);
    assert_eq!(ATTACHES.load(Ordering::Relaxed), 2);
}
