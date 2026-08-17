use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use jvmti_bindings::agent::{AgentLoadContext, AgentUnloadContext};
use jvmti_bindings::sys::jni;
use jvmti_bindings::{Agent, __agent_on_load, __agent_on_unload};

static VM: AtomicUsize = AtomicUsize::new(0);
static RESERVED: AtomicUsize = AtomicUsize::new(0);
static UNLOAD_VM: AtomicUsize = AtomicUsize::new(0);
static OPTIONS: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[derive(Default)]
struct LifecycleAgent;

impl Agent for LifecycleAgent {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        VM.store(context.vm().raw() as usize, Ordering::Relaxed);
        RESERVED.store(context.reserved() as usize, Ordering::Relaxed);
        *OPTIONS.lock().unwrap() = context.option_bytes().unwrap().to_vec();
        assert!(context.options_str().is_err());
        assert_eq!(context.options_lossy().unwrap(), "agent=�");
        27
    }

    fn on_unload(&self, context: AgentUnloadContext<'_>) {
        UNLOAD_VM.store(context.vm().raw() as usize, Ordering::Relaxed);
    }
}

#[test]
fn lifecycle_preserves_vm_reserved_pointer_and_non_utf8_options() {
    let vm = 0x1111usize as *mut jni::JavaVM;
    let reserved = 0x2222usize as *mut c_void;
    let mut options = b"agent=\xff\0".to_vec();

    let result = unsafe {
        __agent_on_load::<LifecycleAgent>(vm, options.as_mut_ptr().cast::<c_char>(), reserved)
    };
    assert_eq!(result, 27);
    assert_eq!(VM.load(Ordering::Relaxed), vm as usize);
    assert_eq!(RESERVED.load(Ordering::Relaxed), reserved as usize);
    assert_eq!(*OPTIONS.lock().unwrap(), b"agent=\xff");

    unsafe { __agent_on_unload(vm) };
    assert_eq!(UNLOAD_VM.load(Ordering::Relaxed), vm as usize);
}
