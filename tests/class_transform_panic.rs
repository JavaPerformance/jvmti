use std::alloc::{Layout, alloc, dealloc};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use jvmti_bindings::agent::AgentLoadContext;
use jvmti_bindings::callbacks::{CallbackContext, ClassFileLoadHookEvent};
use jvmti_bindings::sys::{jni, jvmti};
use jvmti_bindings::{Agent, get_default_callbacks, set_global_agent};

static ALLOCATION_SIZE: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static CONTAINED_PANICS: AtomicUsize = AtomicUsize::new(0);
static PANIC_AFTER_TRANSFORM: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn allocate(
    _env: *mut jvmti::jvmtiEnv,
    size: jni::jlong,
    result: *mut *mut u8,
) -> jvmti::jvmtiError {
    if size <= 0 || result.is_null() {
        return jvmti::jvmtiError::ILLEGAL_ARGUMENT;
    }
    let size = size as usize;
    let memory = unsafe { alloc(Layout::from_size_align(size, 1).unwrap()) };
    if memory.is_null() {
        return jvmti::jvmtiError::OUT_OF_MEMORY;
    }
    ALLOCATION_SIZE.store(size, Ordering::SeqCst);
    ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
    unsafe { *result = memory };
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn deallocate_memory(
    _env: *mut jvmti::jvmtiEnv,
    memory: *mut u8,
) -> jvmti::jvmtiError {
    if memory.is_null() {
        return jvmti::jvmtiError::NULL_POINTER;
    }
    let size = ALLOCATION_SIZE.swap(0, Ordering::SeqCst);
    assert_ne!(size, 0, "deallocation must match the pending transform");
    unsafe { dealloc(memory, Layout::from_size_align(size, 1).unwrap()) };
    DEALLOCATIONS.fetch_add(1, Ordering::SeqCst);
    jvmti::jvmtiError::NONE
}

struct PanickingTransformAgent;

impl Agent for PanickingTransformAgent {
    fn on_load(&self, _context: AgentLoadContext<'_>) -> jni::jint {
        jni::JNI_OK
    }

    fn class_file_load_hook<'callback>(
        &self,
        context: CallbackContext<'callback>,
        mut event: ClassFileLoadHookEvent<'callback>,
    ) {
        event
            .set_transformed_class(&context, &[0xca, 0xfe, 0xba, 0xbe])
            .unwrap();
        if PANIC_AFTER_TRANSFORM.load(Ordering::SeqCst) {
            panic!("class transform panic sentinel");
        }
    }

    fn callback_panicked(&self, event: &'static str) {
        assert_eq!(event, "ClassFileLoadHook");
        CONTAINED_PANICS.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn class_transform_output_commits_only_after_normal_return() {
    ALLOCATION_SIZE.store(0, Ordering::SeqCst);
    ALLOCATIONS.store(0, Ordering::SeqCst);
    DEALLOCATIONS.store(0, Ordering::SeqCst);
    CONTAINED_PANICS.store(0, Ordering::SeqCst);
    PANIC_AFTER_TRANSFORM.store(false, Ordering::SeqCst);

    let mut table = jvmti::jvmtiInterface_1_::default();
    table.Allocate = Some(allocate);
    table.Deallocate = Some(deallocate_memory);
    let mut env = jvmti::jvmtiEnv { functions: &table };

    set_global_agent(Box::new(PanickingTransformAgent)).expect("global agent should be unset");
    let callback = get_default_callbacks()
        .ClassFileLoadHook
        .expect("ClassFileLoadHook callback must be installed");

    let class_data = [0xca, 0xfe, 0xba, 0xbe];
    let mut transformed_length = 99;
    let mut transformed = ptr::dangling_mut::<u8>();
    unsafe {
        callback(
            &mut env,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
            class_data.len() as jni::jint,
            class_data.as_ptr(),
            &mut transformed_length,
            &mut transformed,
        )
    };

    assert!(!transformed.is_null());
    assert_eq!(transformed_length, 4);
    assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 1);
    assert_eq!(DEALLOCATIONS.load(Ordering::SeqCst), 0);
    assert_eq!(CONTAINED_PANICS.load(Ordering::SeqCst), 0);
    // The VM owns successful ClassFileLoadHook output and would release it.
    unsafe { deallocate_memory(&mut env, transformed) };

    PANIC_AFTER_TRANSFORM.store(true, Ordering::SeqCst);
    transformed_length = 99;
    transformed = ptr::dangling_mut::<u8>();
    unsafe {
        callback(
            &mut env,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
            class_data.len() as jni::jint,
            class_data.as_ptr(),
            &mut transformed_length,
            &mut transformed,
        )
    };

    assert!(transformed.is_null());
    assert_eq!(transformed_length, 0);
    assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 2);
    assert_eq!(DEALLOCATIONS.load(Ordering::SeqCst), 2);
    assert_eq!(CONTAINED_PANICS.load(Ordering::SeqCst), 1);
}
