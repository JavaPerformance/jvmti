use std::ffi::CStr;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use jvmti_bindings::env::Jvmti;
use jvmti_bindings::mutf8;
use jvmti_bindings::sys::{jni, jvmti};

static TEST_LOCK: Mutex<()> = Mutex::new(());
static CREATED: AtomicUsize = AtomicUsize::new(0);
static DESTROYED: AtomicUsize = AtomicUsize::new(0);
static ENTERED: AtomicUsize = AtomicUsize::new(0);
static EXITED: AtomicUsize = AtomicUsize::new(0);
static WAITED: AtomicUsize = AtomicUsize::new(0);
static NOTIFIED: AtomicUsize = AtomicUsize::new(0);
static NOTIFIED_ALL: AtomicUsize = AtomicUsize::new(0);
static FAIL_DESTROY_ONCE: AtomicBool = AtomicBool::new(false);
static FAIL_EXIT_ONCE: AtomicBool = AtomicBool::new(false);
static NAME: Mutex<String> = Mutex::new(String::new());

const MONITOR: jvmti::jrawMonitorID = 0x1234usize as jvmti::jrawMonitorID;

unsafe extern "system" fn create(
    _env: *mut jvmti::jvmtiEnv,
    name: *const std::os::raw::c_char,
    monitor: *mut jvmti::jrawMonitorID,
) -> jvmti::jvmtiError {
    if name.is_null() || monitor.is_null() {
        return jvmti::jvmtiError::NULL_POINTER;
    }
    let decoded = mutf8::decode_cstr(unsafe { CStr::from_ptr(name) })
        .expect("wrapper must pass Modified UTF-8");
    *NAME.lock().unwrap() = decoded;
    unsafe { *monitor = MONITOR };
    CREATED.fetch_add(1, Ordering::SeqCst);
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn destroy(
    _env: *mut jvmti::jvmtiEnv,
    monitor: jvmti::jrawMonitorID,
) -> jvmti::jvmtiError {
    if monitor != MONITOR {
        return jvmti::jvmtiError::INVALID_MONITOR;
    }
    DESTROYED.fetch_add(1, Ordering::SeqCst);
    if FAIL_DESTROY_ONCE.swap(false, Ordering::SeqCst) {
        return jvmti::jvmtiError::INTERNAL;
    }
    jvmti::jvmtiError::NONE
}

macro_rules! monitor_operation {
    ($name:ident, $counter:ident) => {
        unsafe extern "system" fn $name(
            _env: *mut jvmti::jvmtiEnv,
            monitor: jvmti::jrawMonitorID,
        ) -> jvmti::jvmtiError {
            if monitor != MONITOR {
                return jvmti::jvmtiError::INVALID_MONITOR;
            }
            $counter.fetch_add(1, Ordering::SeqCst);
            jvmti::jvmtiError::NONE
        }
    };
}

monitor_operation!(enter, ENTERED);
monitor_operation!(notify, NOTIFIED);
monitor_operation!(notify_all, NOTIFIED_ALL);

unsafe extern "system" fn exit(
    _env: *mut jvmti::jvmtiEnv,
    monitor: jvmti::jrawMonitorID,
) -> jvmti::jvmtiError {
    if monitor != MONITOR {
        return jvmti::jvmtiError::INVALID_MONITOR;
    }
    EXITED.fetch_add(1, Ordering::SeqCst);
    if FAIL_EXIT_ONCE.swap(false, Ordering::SeqCst) {
        return jvmti::jvmtiError::INTERNAL;
    }
    jvmti::jvmtiError::NONE
}

unsafe extern "system" fn wait(
    _env: *mut jvmti::jvmtiEnv,
    monitor: jvmti::jrawMonitorID,
    _millis: jni::jlong,
) -> jvmti::jvmtiError {
    if monitor != MONITOR {
        return jvmti::jvmtiError::INVALID_MONITOR;
    }
    WAITED.fetch_add(1, Ordering::SeqCst);
    jvmti::jvmtiError::NONE
}

fn environment() -> (jvmti::jvmtiInterface_1_, jvmti::jvmtiEnv) {
    let mut table = jvmti::jvmtiInterface_1_::default();
    table.CreateRawMonitor = Some(create);
    table.DestroyRawMonitor = Some(destroy);
    table.RawMonitorEnter = Some(enter);
    table.RawMonitorExit = Some(exit);
    table.RawMonitorWait = Some(wait);
    table.RawMonitorNotify = Some(notify);
    table.RawMonitorNotifyAll = Some(notify_all);
    let env = jvmti::jvmtiEnv {
        functions: ptr::null(),
    };
    (table, env)
}

fn reset() {
    for counter in [
        &CREATED,
        &DESTROYED,
        &ENTERED,
        &EXITED,
        &WAITED,
        &NOTIFIED,
        &NOTIFIED_ALL,
    ] {
        counter.store(0, Ordering::SeqCst);
    }
    NAME.lock().unwrap().clear();
    FAIL_DESTROY_ONCE.store(false, Ordering::SeqCst);
    FAIL_EXIT_ONCE.store(false, Ordering::SeqCst);
}

#[test]
fn monitor_and_enter_guard_release_exactly_once() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset();
    let (table, mut raw_env) = environment();
    raw_env.functions = &table;
    let env = unsafe { Jvmti::from_raw(&mut raw_env) };

    {
        let monitor = env.create_raw_monitor("lock\0\u{1f680}").unwrap();
        assert_eq!(&*NAME.lock().unwrap(), "lock\0\u{1f680}");
        {
            let entered = monitor.enter().unwrap();
            entered.wait(5).unwrap();
            entered.notify().unwrap();
            entered.notify_all().unwrap();
        }
        assert_eq!(EXITED.load(Ordering::SeqCst), 1);
        drop(monitor.enter().unwrap());
        assert_eq!(EXITED.load(Ordering::SeqCst), 2);
    }

    assert_eq!(CREATED.load(Ordering::SeqCst), 1);
    assert_eq!(DESTROYED.load(Ordering::SeqCst), 1);
    assert_eq!(ENTERED.load(Ordering::SeqCst), 2);
    assert_eq!(WAITED.load(Ordering::SeqCst), 1);
    assert_eq!(NOTIFIED.load(Ordering::SeqCst), 1);
    assert_eq!(NOTIFIED_ALL.load(Ordering::SeqCst), 1);
}

#[test]
fn explicit_close_and_raw_transfer_do_not_double_destroy() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset();
    let (table, mut raw_env) = environment();
    raw_env.functions = &table;
    let env = unsafe { Jvmti::from_raw(&mut raw_env) };

    env.create_raw_monitor("closed").unwrap().close().unwrap();
    assert_eq!(DESTROYED.load(Ordering::SeqCst), 1);

    let monitor = env.create_raw_monitor("transferred").unwrap();
    let raw = unsafe { monitor.into_raw() };
    assert_eq!(DESTROYED.load(Ordering::SeqCst), 1);
    unsafe { env.destroy_raw_monitor(raw) }.unwrap();
    assert_eq!(DESTROYED.load(Ordering::SeqCst), 2);
}

#[test]
fn failed_explicit_release_retains_ownership_for_drop_retry() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset();
    let (table, mut raw_env) = environment();
    raw_env.functions = &table;
    let env = unsafe { Jvmti::from_raw(&mut raw_env) };

    FAIL_DESTROY_ONCE.store(true, Ordering::SeqCst);
    assert_eq!(
        env.create_raw_monitor("retry-destroy").unwrap().close(),
        Err(jvmti::jvmtiError::INTERNAL)
    );
    assert_eq!(DESTROYED.load(Ordering::SeqCst), 2);

    let monitor = env.create_raw_monitor("retry-exit").unwrap();
    FAIL_EXIT_ONCE.store(true, Ordering::SeqCst);
    assert_eq!(
        monitor.enter().unwrap().exit(),
        Err(jvmti::jvmtiError::INTERNAL)
    );
    assert_eq!(EXITED.load(Ordering::SeqCst), 2);
}
