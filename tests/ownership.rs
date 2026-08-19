use std::alloc::{Layout, alloc, dealloc};
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};

use jvmti_bindings::env::Jvmti;
use jvmti_bindings::sys::{jni, jvmti};
use jvmti_bindings::version::{jvmti_interface_feature, release_profile};

static LAST_ALLOCATION_SIZE: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static DISPOSALS: AtomicUsize = AtomicUsize::new(0);
static RUNTIME_VERSION: AtomicI32 = AtomicI32::new(jvmti::JVMTI_VERSION_1_2);
static OWNERSHIP_TEST_LOCK: Mutex<()> = Mutex::new(());

unsafe extern "system" fn allocate(
    _env: *mut jvmti::jvmtiEnv,
    size: jni::jlong,
    mem_ptr: *mut *mut u8,
) -> jvmti::jvmtiError {
    if size < 0 || mem_ptr.is_null() {
        return jvmti::jvmtiError::ILLEGAL_ARGUMENT;
    }
    if size == 0 {
        unsafe { *mem_ptr = ptr::null_mut() };
        return jvmti::jvmtiError::NONE;
    }
    let size = size as usize;
    let ptr = unsafe { alloc(Layout::from_size_align(size, 1).unwrap()) };
    if ptr.is_null() {
        return jvmti::jvmtiError::OUT_OF_MEMORY;
    }
    LAST_ALLOCATION_SIZE.store(size, Ordering::SeqCst);
    ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
    unsafe { *mem_ptr = ptr };
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn deallocate_memory(
    _env: *mut jvmti::jvmtiEnv,
    mem: *mut u8,
) -> jvmti::jvmtiError {
    if !mem.is_null() {
        let size = LAST_ALLOCATION_SIZE.swap(0, Ordering::SeqCst);
        assert_ne!(size, 0, "deallocation must correspond to an allocation");
        unsafe { dealloc(mem, Layout::from_size_align(size, 1).unwrap()) };
        DEALLOCATIONS.fetch_add(1, Ordering::SeqCst);
    }
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn dispose_environment(_env: *mut jvmti::jvmtiEnv) -> jvmti::jvmtiError {
    DISPOSALS.fetch_add(1, Ordering::SeqCst);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn get_version_number(
    _env: *mut jvmti::jvmtiEnv,
    version: *mut jni::jint,
) -> jvmti::jvmtiError {
    unsafe { *version = RUNTIME_VERSION.load(Ordering::SeqCst) };
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn get_jni_function_table(
    env: *mut jvmti::jvmtiEnv,
    table: *mut *mut jni::JNINativeInterface_,
) -> jvmti::jvmtiError {
    let feature = jvmti_interface_feature(RUNTIME_VERSION.load(Ordering::SeqCst));
    let bytes = release_profile(feature)
        .map(|profile| profile.jni_table_bytes())
        .unwrap_or(std::mem::size_of::<jni::JNINativeInterface_>());
    unsafe { allocate(env, bytes as jni::jlong, table.cast()) }
}

unsafe extern "system" fn get_loaded_classes_with_invalid_count(
    env: *mut jvmti::jvmtiEnv,
    count: *mut jni::jint,
    classes: *mut *mut jni::jclass,
) -> jvmti::jvmtiError {
    let error = unsafe {
        allocate(
            env,
            std::mem::size_of::<jni::jclass>() as jni::jlong,
            classes.cast(),
        )
    };
    if error == jvmti::jvmtiError::NONE {
        unsafe { *count = -1 };
    }
    error
}

fn mock_environment() -> (jvmti::jvmtiInterface_1_, jvmti::jvmtiEnv) {
    let mut table = jvmti::jvmtiInterface_1_::default();
    table.Allocate = Some(allocate);
    table.Deallocate = Some(deallocate_memory);
    table.DisposeEnvironment = Some(dispose_environment);
    table.GetVersionNumber = Some(get_version_number);
    table.GetJNIFunctionTable = Some(get_jni_function_table);
    table.GetLoadedClasses = Some(get_loaded_classes_with_invalid_count);
    let env = jvmti::jvmtiEnv {
        functions: ptr::null(),
    };
    (table, env)
}

#[test]
fn jni_table_copy_preserves_the_runtime_prefix_and_ownership() {
    let _guard = OWNERSHIP_TEST_LOCK.lock().unwrap();
    ALLOCATIONS.store(0, Ordering::SeqCst);
    DEALLOCATIONS.store(0, Ordering::SeqCst);
    LAST_ALLOCATION_SIZE.store(0, Ordering::SeqCst);
    RUNTIME_VERSION.store(jvmti::version_for_feature(8), Ordering::SeqCst);

    let (table, mut raw_env) = mock_environment();
    raw_env.functions = &table;
    let env = unsafe { Jvmti::from_raw(&mut raw_env) };

    {
        let table = env.get_jni_function_table().unwrap();
        assert_eq!(table.jvmti_interface_feature(), 8);
        assert_eq!(
            table.known_byte_len(),
            Some(release_profile(8).unwrap().jni_table_bytes())
        );
        assert!(!table.as_ptr().is_null());
    }
    assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 1);
    assert_eq!(DEALLOCATIONS.load(Ordering::SeqCst), 1);

    RUNTIME_VERSION.store(jvmti::version_for_feature(29), Ordering::SeqCst);
    {
        let table = env.get_jni_function_table().unwrap();
        assert_eq!(table.jvmti_interface_feature(), 29);
        assert_eq!(table.known_byte_len(), None);
    }
    assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 2);
    assert_eq!(DEALLOCATIONS.load(Ordering::SeqCst), 2);
}

#[test]
fn allocation_is_environment_bound_and_deallocated_once() {
    let _guard = OWNERSHIP_TEST_LOCK.lock().unwrap();
    ALLOCATIONS.store(0, Ordering::SeqCst);
    DEALLOCATIONS.store(0, Ordering::SeqCst);
    DISPOSALS.store(0, Ordering::SeqCst);
    LAST_ALLOCATION_SIZE.store(0, Ordering::SeqCst);

    let (table, mut raw_env) = mock_environment();
    raw_env.functions = &table;
    let env = unsafe { Jvmti::from_raw(&mut raw_env) };

    {
        let mut allocation = env.allocate(4).unwrap();
        allocation.as_mut_slice().copy_from_slice(&[1, 2, 3, 4]);
        assert_eq!(allocation.as_slice(), &[1, 2, 3, 4]);
        assert_eq!(allocation.byte_len(), 4);
    }
    assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 1);
    assert_eq!(DEALLOCATIONS.load(Ordering::SeqCst), 1);

    let allocation = env.allocate(8).unwrap();
    let ptr = unsafe { allocation.into_raw() };
    assert_eq!(DEALLOCATIONS.load(Ordering::SeqCst), 1);
    unsafe { env.deallocate_raw(ptr) }.unwrap();
    assert_eq!(DEALLOCATIONS.load(Ordering::SeqCst), 2);

    let error = unsafe { env.allocate_raw(-1) }.unwrap_err();
    assert_eq!(error, jvmti::jvmtiError::ILLEGAL_ARGUMENT);
    assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 2);

    // SAFETY: this mock environment has no callbacks or outstanding resources.
    unsafe { env.dispose_environment() }.unwrap();
    assert_eq!(DISPOSALS.load(Ordering::SeqCst), 1);
}

#[test]
fn malformed_native_array_is_deallocated_before_error_returns() {
    let _guard = OWNERSHIP_TEST_LOCK.lock().unwrap();
    ALLOCATIONS.store(0, Ordering::SeqCst);
    DEALLOCATIONS.store(0, Ordering::SeqCst);
    LAST_ALLOCATION_SIZE.store(0, Ordering::SeqCst);

    let (table, mut raw_env) = mock_environment();
    raw_env.functions = &table;
    let env = unsafe { Jvmti::from_raw(&mut raw_env) };

    assert_eq!(
        env.get_loaded_classes().unwrap_err(),
        jvmti::jvmtiError::ILLEGAL_ARGUMENT
    );
    assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 1);
    assert_eq!(DEALLOCATIONS.load(Ordering::SeqCst), 1);
}
