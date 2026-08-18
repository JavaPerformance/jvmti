use std::mem::MaybeUninit;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

use jvmti_bindings::env::{GlobalRef, JniEnv, WeakGlobalRef};
use jvmti_bindings::sys::jni;

static GLOBAL_CREATIONS: AtomicUsize = AtomicUsize::new(0);
static WEAK_CREATIONS: AtomicUsize = AtomicUsize::new(0);
static GLOBAL_DELETIONS: AtomicUsize = AtomicUsize::new(0);
static WEAK_DELETIONS: AtomicUsize = AtomicUsize::new(0);
static ATTACHMENTS: AtomicUsize = AtomicUsize::new(0);
static DETACHMENTS: AtomicUsize = AtomicUsize::new(0);
static RETURN_DETACHED: AtomicBool = AtomicBool::new(false);
static VM: AtomicPtr<jni::JavaVM> = AtomicPtr::new(ptr::null_mut());
static ENV: AtomicPtr<jni::JNIEnv> = AtomicPtr::new(ptr::null_mut());
static ARRAY_ELEMENTS: AtomicPtr<jni::jint> = AtomicPtr::new(ptr::null_mut());
static STRING_CHARACTERS: AtomicPtr<jni::jchar> = AtomicPtr::new(ptr::null_mut());
static ARRAY_RELEASES: AtomicUsize = AtomicUsize::new(0);
static ARRAY_RELEASE_MODE: AtomicUsize = AtomicUsize::new(usize::MAX);
static PRIMITIVE_CRITICAL_RELEASES: AtomicUsize = AtomicUsize::new(0);
static STRING_CRITICAL_RELEASES: AtomicUsize = AtomicUsize::new(0);
static LOCAL_FRAMES_PUSHED: AtomicUsize = AtomicUsize::new(0);
static LOCAL_FRAMES_POPPED: AtomicUsize = AtomicUsize::new(0);
static LAST_POP_RESULT: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(ptr::null_mut());
static MONITOR_ENTERS: AtomicUsize = AtomicUsize::new(0);
static MONITOR_EXITS: AtomicUsize = AtomicUsize::new(0);
static FAIL_MONITOR_EXIT_ONCE: AtomicBool = AtomicBool::new(false);
static TEST_LOCK: Mutex<()> = Mutex::new(());

unsafe extern "system" fn failing_get_java_vm(
    _env: *mut jni::JNIEnv,
    vm: *mut *mut jni::JavaVM,
) -> jni::jint {
    unsafe { *vm = ptr::null_mut() };
    jni::JNI_ERR
}

unsafe extern "system" fn successful_get_java_vm(
    _env: *mut jni::JNIEnv,
    vm: *mut *mut jni::JavaVM,
) -> jni::jint {
    unsafe { *vm = VM.load(Ordering::SeqCst) };
    jni::JNI_OK
}

unsafe extern "system" fn tracked_new_global_ref(
    _env: *mut jni::JNIEnv,
    obj: jni::jobject,
) -> jni::jobject {
    GLOBAL_CREATIONS.fetch_add(1, Ordering::SeqCst);
    obj
}

unsafe extern "system" fn tracked_new_weak_global_ref(
    _env: *mut jni::JNIEnv,
    obj: jni::jobject,
) -> jni::jweak {
    WEAK_CREATIONS.fetch_add(1, Ordering::SeqCst);
    obj
}

unsafe extern "system" fn tracked_delete_global_ref(_env: *mut jni::JNIEnv, _obj: jni::jobject) {
    GLOBAL_DELETIONS.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "system" fn tracked_delete_weak_global_ref(_env: *mut jni::JNIEnv, _obj: jni::jweak) {
    WEAK_DELETIONS.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "system" fn get_env(
    _vm: *mut jni::JavaVM,
    env: *mut *mut std::ffi::c_void,
    _version: jni::jint,
) -> jni::jint {
    if RETURN_DETACHED.load(Ordering::SeqCst) {
        return jni::JNI_EDETACHED;
    }
    unsafe { *env = ENV.load(Ordering::SeqCst).cast() };
    jni::JNI_OK
}

unsafe extern "system" fn attach_current_thread(
    _vm: *mut jni::JavaVM,
    env: *mut *mut std::ffi::c_void,
    _args: *mut std::ffi::c_void,
) -> jni::jint {
    ATTACHMENTS.fetch_add(1, Ordering::SeqCst);
    unsafe { *env = ENV.load(Ordering::SeqCst).cast() };
    jni::JNI_OK
}

unsafe extern "system" fn detach_current_thread(_vm: *mut jni::JavaVM) -> jni::jint {
    DETACHMENTS.fetch_add(1, Ordering::SeqCst);
    jni::JNI_OK
}

unsafe extern "system" fn array_length(_env: *mut jni::JNIEnv, _array: jni::jarray) -> jni::jsize {
    3
}

unsafe extern "system" fn string_length(
    _env: *mut jni::JNIEnv,
    _string: jni::jstring,
) -> jni::jsize {
    2
}

unsafe extern "system" fn get_int_array_elements(
    _env: *mut jni::JNIEnv,
    _array: jni::jintArray,
    is_copy: *mut jni::jboolean,
) -> *mut jni::jint {
    unsafe { *is_copy = jni::JNI_TRUE };
    ARRAY_ELEMENTS.load(Ordering::SeqCst)
}

unsafe extern "system" fn release_int_array_elements(
    _env: *mut jni::JNIEnv,
    _array: jni::jintArray,
    _elements: *mut jni::jint,
    mode: jni::jint,
) {
    ARRAY_RELEASES.fetch_add(1, Ordering::SeqCst);
    ARRAY_RELEASE_MODE.store(mode as usize, Ordering::SeqCst);
}

unsafe extern "system" fn get_primitive_array_critical(
    _env: *mut jni::JNIEnv,
    _array: jni::jarray,
    is_copy: *mut jni::jboolean,
) -> *mut std::ffi::c_void {
    unsafe { *is_copy = jni::JNI_FALSE };
    ARRAY_ELEMENTS.load(Ordering::SeqCst).cast()
}

unsafe extern "system" fn release_primitive_array_critical(
    _env: *mut jni::JNIEnv,
    _array: jni::jarray,
    _elements: *mut std::ffi::c_void,
    _mode: jni::jint,
) {
    PRIMITIVE_CRITICAL_RELEASES.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "system" fn get_string_critical(
    _env: *mut jni::JNIEnv,
    _string: jni::jstring,
    is_copy: *mut jni::jboolean,
) -> *const jni::jchar {
    unsafe { *is_copy = jni::JNI_FALSE };
    STRING_CHARACTERS.load(Ordering::SeqCst)
}

unsafe extern "system" fn release_string_critical(
    _env: *mut jni::JNIEnv,
    _string: jni::jstring,
    _characters: *const jni::jchar,
) {
    STRING_CRITICAL_RELEASES.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "system" fn push_local_frame(
    _env: *mut jni::JNIEnv,
    _capacity: jni::jint,
) -> jni::jint {
    LOCAL_FRAMES_PUSHED.fetch_add(1, Ordering::SeqCst);
    jni::JNI_OK
}

unsafe extern "system" fn pop_local_frame(
    _env: *mut jni::JNIEnv,
    result: jni::jobject,
) -> jni::jobject {
    LOCAL_FRAMES_POPPED.fetch_add(1, Ordering::SeqCst);
    LAST_POP_RESULT.store(result, Ordering::SeqCst);
    result
}

unsafe extern "system" fn monitor_enter(
    _env: *mut jni::JNIEnv,
    _object: jni::jobject,
) -> jni::jint {
    MONITOR_ENTERS.fetch_add(1, Ordering::SeqCst);
    jni::JNI_OK
}

unsafe extern "system" fn monitor_exit(_env: *mut jni::JNIEnv, _object: jni::jobject) -> jni::jint {
    MONITOR_EXITS.fetch_add(1, Ordering::SeqCst);
    if FAIL_MONITOR_EXIT_ONCE.swap(false, Ordering::SeqCst) {
        jni::JNI_ERR
    } else {
        jni::JNI_OK
    }
}

fn reset_counters() {
    GLOBAL_CREATIONS.store(0, Ordering::SeqCst);
    WEAK_CREATIONS.store(0, Ordering::SeqCst);
    GLOBAL_DELETIONS.store(0, Ordering::SeqCst);
    WEAK_DELETIONS.store(0, Ordering::SeqCst);
    ATTACHMENTS.store(0, Ordering::SeqCst);
    DETACHMENTS.store(0, Ordering::SeqCst);
    ARRAY_RELEASES.store(0, Ordering::SeqCst);
    ARRAY_RELEASE_MODE.store(usize::MAX, Ordering::SeqCst);
    PRIMITIVE_CRITICAL_RELEASES.store(0, Ordering::SeqCst);
    STRING_CRITICAL_RELEASES.store(0, Ordering::SeqCst);
    LOCAL_FRAMES_PUSHED.store(0, Ordering::SeqCst);
    LOCAL_FRAMES_POPPED.store(0, Ordering::SeqCst);
    LAST_POP_RESULT.store(ptr::null_mut(), Ordering::SeqCst);
    MONITOR_ENTERS.store(0, Ordering::SeqCst);
    MONITOR_EXITS.store(0, Ordering::SeqCst);
    FAIL_MONITOR_EXIT_ONCE.store(false, Ordering::SeqCst);
}

fn with_array_environment(test: impl FnOnce(&JniEnv, jni::jarray, jni::jstring)) {
    let mut native_table = MaybeUninit::<jni::JNINativeInterface_>::uninit();
    unsafe {
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).GetArrayLength).write(array_length);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).GetIntArrayElements)
            .write(get_int_array_elements);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).ReleaseIntArrayElements)
            .write(release_int_array_elements);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).GetPrimitiveArrayCritical)
            .write(get_primitive_array_critical);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).ReleasePrimitiveArrayCritical)
            .write(release_primitive_array_critical);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).GetStringLength).write(string_length);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).GetStringCritical)
            .write(get_string_critical);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).ReleaseStringCritical)
            .write(release_string_critical);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).PushLocalFrame).write(push_local_frame);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).PopLocalFrame).write(pop_local_frame);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).MonitorEnter).write(monitor_enter);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).MonitorExit).write(monitor_exit);
    }
    let mut raw_env = native_table.as_ptr();
    let env = unsafe { JniEnv::from_raw(&mut raw_env) };
    let handle = ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr();
    test(&env, handle, handle);
}

fn with_mock_environment(test_detached_cleanup: bool, test: impl FnOnce(&JniEnv, jni::jobject)) {
    let mut native_table = MaybeUninit::<jni::JNINativeInterface_>::uninit();
    unsafe {
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).GetJavaVM).write(successful_get_java_vm);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).NewGlobalRef).write(tracked_new_global_ref);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).NewWeakGlobalRef)
            .write(tracked_new_weak_global_ref);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).DeleteGlobalRef)
            .write(tracked_delete_global_ref);
        ptr::addr_of_mut!((*native_table.as_mut_ptr()).DeleteWeakGlobalRef)
            .write(tracked_delete_weak_global_ref);
    }

    let mut invoke_table = MaybeUninit::<jni::JNIInvokeInterface_>::uninit();
    unsafe {
        ptr::addr_of_mut!((*invoke_table.as_mut_ptr()).AttachCurrentThread)
            .write(attach_current_thread);
        ptr::addr_of_mut!((*invoke_table.as_mut_ptr()).DetachCurrentThread)
            .write(detach_current_thread);
        ptr::addr_of_mut!((*invoke_table.as_mut_ptr()).GetEnv).write(get_env);
    }

    let mut raw_vm = invoke_table.as_ptr();
    let mut raw_env = native_table.as_ptr();
    VM.store(&mut raw_vm, Ordering::SeqCst);
    ENV.store(ptr::addr_of_mut!(raw_env), Ordering::SeqCst);
    RETURN_DETACHED.store(test_detached_cleanup, Ordering::SeqCst);

    let env = unsafe { JniEnv::from_raw(ptr::addr_of_mut!(raw_env)) };
    let object = ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr();
    test(&env, object);

    VM.store(ptr::null_mut(), Ordering::SeqCst);
    ENV.store(ptr::null_mut(), Ordering::SeqCst);
    RETURN_DETACHED.store(false, Ordering::SeqCst);
}

#[test]
fn failed_vm_lookup_cannot_strand_global_references() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_counters();

    let mut table = MaybeUninit::<jni::JNINativeInterface_>::uninit();
    unsafe {
        ptr::addr_of_mut!((*table.as_mut_ptr()).GetJavaVM).write(failing_get_java_vm);
        ptr::addr_of_mut!((*table.as_mut_ptr()).NewGlobalRef).write(tracked_new_global_ref);
        ptr::addr_of_mut!((*table.as_mut_ptr()).NewWeakGlobalRef)
            .write(tracked_new_weak_global_ref);
    }
    let mut raw_env = table.as_ptr();
    let env = unsafe { JniEnv::from_raw(&mut raw_env) };
    let object = ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr();

    let global_result = unsafe { GlobalRef::new(&env, object) };
    assert!(matches!(global_result, Err(jni::JNI_ERR)));
    let weak_result = unsafe { WeakGlobalRef::new(&env, object) };
    assert!(matches!(weak_result, Err(jni::JNI_ERR)));

    assert_eq!(GLOBAL_CREATIONS.load(Ordering::SeqCst), 0);
    assert_eq!(WEAK_CREATIONS.load(Ordering::SeqCst), 0);
}

#[test]
fn owning_references_close_exactly_once_on_attached_thread() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_counters();

    with_mock_environment(false, |env, object| {
        unsafe { GlobalRef::new(env, object) }
            .unwrap()
            .close()
            .unwrap();
        unsafe { WeakGlobalRef::new(env, object) }
            .unwrap()
            .close()
            .unwrap();
    });

    assert_eq!(GLOBAL_CREATIONS.load(Ordering::SeqCst), 1);
    assert_eq!(WEAK_CREATIONS.load(Ordering::SeqCst), 1);
    assert_eq!(GLOBAL_DELETIONS.load(Ordering::SeqCst), 1);
    assert_eq!(WEAK_DELETIONS.load(Ordering::SeqCst), 1);
    assert_eq!(ATTACHMENTS.load(Ordering::SeqCst), 0);
    assert_eq!(DETACHMENTS.load(Ordering::SeqCst), 0);
}

#[test]
fn owning_references_attach_for_drop_and_delete_exactly_once() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_counters();

    with_mock_environment(true, |env, object| {
        drop(unsafe { GlobalRef::new(env, object) }.unwrap());
        drop(unsafe { WeakGlobalRef::new(env, object) }.unwrap());
    });

    assert_eq!(GLOBAL_DELETIONS.load(Ordering::SeqCst), 1);
    assert_eq!(WEAK_DELETIONS.load(Ordering::SeqCst), 1);
    assert_eq!(ATTACHMENTS.load(Ordering::SeqCst), 2);
    assert_eq!(DETACHMENTS.load(Ordering::SeqCst), 2);
}

#[test]
fn primitive_array_element_guard_releases_exactly_once() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_counters();
    let mut values = [1, 2, 3];
    ARRAY_ELEMENTS.store(values.as_mut_ptr(), Ordering::SeqCst);

    with_array_environment(|env, array, _string| {
        let mut elements = unsafe { env.get_int_array_elements(array) }.unwrap();
        assert!(elements.is_copy());
        assert_eq!(&*elements, &[1, 2, 3]);
        elements[1] = 7;
        elements.commit();
        assert_eq!(ARRAY_RELEASES.load(Ordering::SeqCst), 1);
        assert_eq!(
            ARRAY_RELEASE_MODE.load(Ordering::SeqCst),
            jni::JNI_COMMIT as usize
        );
        drop(elements);
    });

    assert_eq!(values, [1, 7, 3]);
    assert_eq!(ARRAY_RELEASES.load(Ordering::SeqCst), 2);
    assert_eq!(ARRAY_RELEASE_MODE.load(Ordering::SeqCst), 0);
    ARRAY_ELEMENTS.store(ptr::null_mut(), Ordering::SeqCst);
}

#[test]
fn primitive_array_element_guard_abort_does_not_double_release() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_counters();
    let mut values = [1, 2, 3];
    ARRAY_ELEMENTS.store(values.as_mut_ptr(), Ordering::SeqCst);

    with_array_environment(|env, array, _string| {
        unsafe { env.get_int_array_elements(array) }
            .unwrap()
            .abort();
    });

    assert_eq!(ARRAY_RELEASES.load(Ordering::SeqCst), 1);
    assert_eq!(
        ARRAY_RELEASE_MODE.load(Ordering::SeqCst),
        jni::JNI_ABORT as usize
    );
    ARRAY_ELEMENTS.store(ptr::null_mut(), Ordering::SeqCst);
}

#[test]
fn critical_region_guards_release_exactly_once() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_counters();
    let mut values = [1, 2, 3];
    let mut characters = [b'O' as jni::jchar, b'K' as jni::jchar];
    ARRAY_ELEMENTS.store(values.as_mut_ptr(), Ordering::SeqCst);
    STRING_CHARACTERS.store(characters.as_mut_ptr(), Ordering::SeqCst);

    with_array_environment(|env, array, string| {
        let primitive = unsafe { env.get_primitive_array_critical(array) }.unwrap();
        assert_eq!(primitive.element_count(), 3);
        assert!(!primitive.is_copy());
        primitive.close();

        let text = unsafe { env.get_string_critical(string) }.unwrap();
        assert_eq!(&*text, &[b'O' as jni::jchar, b'K' as jni::jchar]);
        assert!(!text.is_copy());
        drop(text);
    });

    assert_eq!(PRIMITIVE_CRITICAL_RELEASES.load(Ordering::SeqCst), 1);
    assert_eq!(STRING_CRITICAL_RELEASES.load(Ordering::SeqCst), 1);
    ARRAY_ELEMENTS.store(ptr::null_mut(), Ordering::SeqCst);
    STRING_CHARACTERS.store(ptr::null_mut(), Ordering::SeqCst);
}

#[test]
fn local_frame_guard_pops_exactly_once_and_can_promote_one_reference() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_counters();

    with_array_environment(|env, object, _string| {
        drop(env.push_local_frame(8).unwrap());
        assert!(LAST_POP_RESULT.load(Ordering::SeqCst).is_null());

        let frame = env.push_local_frame(8).unwrap();
        let promoted = unsafe { frame.pop(object) };
        assert_eq!(promoted, object);
    });

    assert_eq!(LOCAL_FRAMES_PUSHED.load(Ordering::SeqCst), 2);
    assert_eq!(LOCAL_FRAMES_POPPED.load(Ordering::SeqCst), 2);
}

#[test]
fn java_monitor_guard_exits_and_retries_one_failed_explicit_exit() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_counters();

    with_array_environment(|env, object, _string| {
        drop(unsafe { env.monitor_enter(object) }.unwrap());

        FAIL_MONITOR_EXIT_ONCE.store(true, Ordering::SeqCst);
        let result = unsafe { env.monitor_enter(object) }.unwrap().exit();
        assert_eq!(result, Err(jni::JNI_ERR));
    });

    assert_eq!(MONITOR_ENTERS.load(Ordering::SeqCst), 2);
    assert_eq!(MONITOR_EXITS.load(Ordering::SeqCst), 3);
}
