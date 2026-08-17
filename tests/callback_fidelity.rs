use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use jvmti_bindings::agent::AgentLoadContext;
use jvmti_bindings::callbacks::*;
use jvmti_bindings::sys::jvmti;
use jvmti_bindings::{Agent, get_default_callbacks, jni, set_global_agent};

static SEEN: AtomicU64 = AtomicU64::new(0);
static PANIC_NEXT_METHOD_ENTRY: AtomicBool = AtomicBool::new(false);
static CONTAINED_PANICS: AtomicUsize = AtomicUsize::new(0);

const THREAD: usize = 0x1010;
const CLASS: usize = 0x2020;
const METHOD: usize = 0x3030;
const OBJECT: usize = 0x4040;
const FIELD: usize = 0x5050;
const ADDRESS: usize = 0x6060;
const COMPILE_INFO: usize = 0x7070;
const RESERVED: usize = 0x8080;

fn mut_ptr<T>(value: usize) -> *mut T {
    value as *mut T
}

fn const_ptr<T>(value: usize) -> *const T {
    value as *const T
}

fn mark(context: &CallbackContext<'_>, bit: u32, has_jni: bool) {
    assert_eq!(context.jvmti().raw() as usize, 0x9090);
    assert_eq!(context.jni().is_some(), has_jni);
    if let Some(jni) = context.jni() {
        assert_eq!(jni.raw() as usize, 0xa0a0);
    }
    SEEN.fetch_or(1_u64 << bit, Ordering::Relaxed);
}

struct SentinelAgent;

impl Agent for SentinelAgent {
    fn on_load(&self, _context: AgentLoadContext<'_>) -> jni::jint {
        jni::JNI_OK
    }

    fn callback_panicked(&self, event: &'static str) {
        assert_eq!(event, "MethodEntry");
        CONTAINED_PANICS.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_init(&self, context: CallbackContext<'_>, event: ThreadEvent) {
        mark(&context, 0, true);
        assert_eq!(event.thread() as usize, THREAD);
    }

    fn vm_death(&self, context: CallbackContext<'_>) {
        mark(&context, 1, true);
    }

    fn thread_start(&self, context: CallbackContext<'_>, event: ThreadEvent) {
        mark(&context, 2, true);
        assert_eq!(event.thread() as usize, THREAD);
    }

    fn thread_end(&self, context: CallbackContext<'_>, event: ThreadEvent) {
        mark(&context, 3, true);
        assert_eq!(event.thread() as usize, THREAD);
    }

    fn class_file_load_hook(
        &self,
        context: CallbackContext<'_>,
        event: ClassFileLoadHookEvent<'_>,
    ) {
        mark(&context, 4, true);
        assert_eq!(event.class_being_redefined() as usize, CLASS);
        assert_eq!(event.loader() as usize, OBJECT);
        assert_eq!(event.name().unwrap().to_bytes(), b"example/Agent");
        assert_eq!(event.protection_domain() as usize, OBJECT + 1);
        assert_eq!(event.class_data(), &[0xca, 0xfe, 0xba, 0xbe]);
    }

    fn class_load(&self, context: CallbackContext<'_>, event: ClassEvent) {
        mark(&context, 5, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.class() as usize, CLASS);
    }

    fn class_prepare(&self, context: CallbackContext<'_>, event: ClassEvent) {
        mark(&context, 6, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.class() as usize, CLASS);
    }

    fn vm_start(&self, context: CallbackContext<'_>) {
        mark(&context, 7, true);
    }

    fn exception(&self, context: CallbackContext<'_>, event: ExceptionEvent) {
        mark(&context, 8, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.method() as usize, METHOD);
        assert_eq!(event.location(), 101);
        assert_eq!(event.exception() as usize, OBJECT);
        assert_eq!(event.catch_method() as usize, METHOD + 1);
        assert_eq!(event.catch_location(), 102);
    }

    fn exception_catch(&self, context: CallbackContext<'_>, event: ExceptionCatchEvent) {
        mark(&context, 9, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.method() as usize, METHOD);
        assert_eq!(event.location(), 103);
        assert_eq!(event.exception() as usize, OBJECT);
    }

    fn single_step(&self, context: CallbackContext<'_>, event: LocationEvent) {
        mark(&context, 10, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.method() as usize, METHOD);
        assert_eq!(event.location(), 104);
    }

    fn frame_pop(&self, context: CallbackContext<'_>, event: FramePopEvent) {
        mark(&context, 11, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.method() as usize, METHOD);
        assert!(event.was_popped_by_exception());
    }

    fn breakpoint(&self, context: CallbackContext<'_>, event: LocationEvent) {
        mark(&context, 12, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.method() as usize, METHOD);
        assert_eq!(event.location(), 105);
    }

    fn field_access(&self, context: CallbackContext<'_>, event: FieldAccessEvent) {
        mark(&context, 13, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.method() as usize, METHOD);
        assert_eq!(event.location(), 106);
        assert_eq!(event.field_class() as usize, CLASS);
        assert_eq!(event.object() as usize, OBJECT);
        assert_eq!(event.field() as usize, FIELD);
    }

    fn field_modification(&self, context: CallbackContext<'_>, event: FieldModificationEvent) {
        mark(&context, 14, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.method() as usize, METHOD);
        assert_eq!(event.location(), 107);
        assert_eq!(event.field_class() as usize, CLASS);
        assert_eq!(event.object() as usize, OBJECT);
        assert_eq!(event.field() as usize, FIELD);
        assert_eq!(event.signature_type(), b'J' as _);
        assert_eq!(unsafe { event.new_value().j }, 0x1122_3344_5566_7788);
    }

    fn method_entry(&self, context: CallbackContext<'_>, event: MethodEvent) {
        mark(&context, 15, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.method() as usize, METHOD);
        if PANIC_NEXT_METHOD_ENTRY.swap(false, Ordering::Relaxed) {
            panic!("callback panic sentinel");
        }
    }

    fn method_exit(&self, context: CallbackContext<'_>, event: MethodExitEvent) {
        mark(&context, 16, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.method() as usize, METHOD);
        assert!(!event.was_popped_by_exception());
        assert_eq!(unsafe { event.return_value().unwrap().j }, 0x1234_5678);
    }

    fn native_method_bind(&self, context: CallbackContext<'_>, mut event: NativeMethodBindEvent) {
        mark(&context, 17, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.method() as usize, METHOD);
        assert_eq!(event.address() as usize, ADDRESS);
        unsafe { event.set_new_address(mut_ptr(ADDRESS + 1)) };
    }

    fn compiled_method_load(
        &self,
        context: CallbackContext<'_>,
        event: CompiledMethodLoadEvent<'_>,
    ) {
        mark(&context, 18, false);
        assert_eq!(event.method() as usize, METHOD);
        assert_eq!(event.code_size(), 4096);
        assert_eq!(event.code_address() as usize, ADDRESS);
        assert_eq!(event.map().len(), 1);
        assert_eq!(event.map()[0].start_address as usize, ADDRESS + 2);
        assert_eq!(event.map()[0].location, 108);
        assert_eq!(event.compile_info() as usize, COMPILE_INFO);
    }

    fn compiled_method_unload(
        &self,
        context: CallbackContext<'_>,
        event: CompiledMethodUnloadEvent,
    ) {
        mark(&context, 19, false);
        assert_eq!(event.method() as usize, METHOD);
        assert_eq!(event.code_address() as usize, ADDRESS);
    }

    fn dynamic_code_generated(
        &self,
        context: CallbackContext<'_>,
        event: DynamicCodeGeneratedEvent<'_>,
    ) {
        mark(&context, 20, false);
        assert_eq!(event.name().unwrap().to_bytes(), b"jit-sentinel");
        assert_eq!(event.address() as usize, ADDRESS);
        assert_eq!(event.length(), 2048);
    }

    fn data_dump_request(&self, context: CallbackContext<'_>) {
        mark(&context, 21, false);
    }

    fn monitor_wait(&self, context: CallbackContext<'_>, event: MonitorWaitEvent) {
        mark(&context, 22, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.object() as usize, OBJECT);
        assert_eq!(event.timeout(), 109);
    }

    fn monitor_waited(&self, context: CallbackContext<'_>, event: MonitorWaitedEvent) {
        mark(&context, 23, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.object() as usize, OBJECT);
        assert!(event.timed_out());
    }

    fn monitor_contended_enter(&self, context: CallbackContext<'_>, event: MonitorEvent) {
        mark(&context, 24, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.object() as usize, OBJECT);
    }

    fn monitor_contended_entered(&self, context: CallbackContext<'_>, event: MonitorEvent) {
        mark(&context, 25, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.object() as usize, OBJECT);
    }

    fn resource_exhausted(&self, context: CallbackContext<'_>, event: ResourceExhaustedEvent<'_>) {
        mark(&context, 26, true);
        assert_eq!(event.flags(), 0x55aa);
        assert_eq!(event.reserved() as usize, RESERVED);
        assert_eq!(
            event.description().unwrap().to_bytes(),
            b"resource sentinel"
        );
    }

    fn garbage_collection_start(&self, context: CallbackContext<'_>) {
        mark(&context, 27, false);
    }

    fn garbage_collection_finish(&self, context: CallbackContext<'_>) {
        mark(&context, 28, false);
    }

    fn object_free(&self, context: CallbackContext<'_>, event: ObjectFreeEvent) {
        mark(&context, 29, false);
        assert_eq!(event.tag(), 0x2233_4455_6677_8899);
    }

    fn vm_object_alloc(&self, context: CallbackContext<'_>, event: ObjectAllocationEvent) {
        mark(&context, 30, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert_eq!(event.object() as usize, OBJECT);
        assert_eq!(event.class() as usize, CLASS);
        assert_eq!(event.size(), 8192);
    }

    fn sampled_object_alloc(&self, context: CallbackContext<'_>, event: ObjectAllocationEvent) {
        mark(&context, 31, true);
        assert_eq!(event.thread() as usize, THREAD);
        assert!(event.object().is_null());
        assert_eq!(event.class() as usize, CLASS);
        assert_eq!(event.size(), 16384);
    }

    fn virtual_thread_start(&self, context: CallbackContext<'_>, event: ThreadEvent) {
        mark(&context, 32, true);
        assert_eq!(event.thread() as usize, THREAD);
    }

    fn virtual_thread_end(&self, context: CallbackContext<'_>, event: ThreadEvent) {
        mark(&context, 33, true);
        assert_eq!(event.thread() as usize, THREAD);
    }
}

#[test]
fn every_callback_forwards_exact_context_and_payload() {
    set_global_agent(Box::new(SentinelAgent)).expect("global agent should be unset");

    let env = mut_ptr::<jvmti::jvmtiEnv>(0x9090);
    let jni_env = mut_ptr::<jni::JNIEnv>(0xa0a0);
    let thread = mut_ptr(THREAD);
    let class = mut_ptr(CLASS);
    let method = mut_ptr(METHOD);
    let object = mut_ptr(OBJECT);
    let field = mut_ptr(FIELD);
    let class_name = b"example/Agent\0";
    let class_data = [0xca, 0xfe, 0xba, 0xbe];
    let dynamic_name = b"jit-sentinel\0";
    let resource_description = b"resource sentinel\0";
    let map = [jvmti::jvmtiAddrLocationMap {
        start_address: const_ptr(ADDRESS + 2),
        location: 108,
    }];
    let mut new_class_data_length = 999;
    let mut new_class_data = mut_ptr(0xb0b0);
    let mut new_native_address = mut_ptr(0xc0c0);
    let callbacks = get_default_callbacks();

    unsafe {
        callbacks.VMInit.unwrap()(env, jni_env, thread);
        callbacks.VMDeath.unwrap()(env, jni_env);
        callbacks.ThreadStart.unwrap()(env, jni_env, thread);
        callbacks.ThreadEnd.unwrap()(env, jni_env, thread);
        callbacks.ClassFileLoadHook.unwrap()(
            env,
            jni_env,
            class,
            object,
            class_name.as_ptr().cast(),
            mut_ptr(OBJECT + 1),
            class_data.len() as jni::jint,
            class_data.as_ptr(),
            &mut new_class_data_length,
            &mut new_class_data,
        );
        callbacks.ClassLoad.unwrap()(env, jni_env, thread, class);
        callbacks.ClassPrepare.unwrap()(env, jni_env, thread, class);
        callbacks.VMStart.unwrap()(env, jni_env);
        callbacks.Exception.unwrap()(
            env,
            jni_env,
            thread,
            method,
            101,
            object,
            mut_ptr(METHOD + 1),
            102,
        );
        callbacks.ExceptionCatch.unwrap()(env, jni_env, thread, method, 103, object);
        callbacks.SingleStep.unwrap()(env, jni_env, thread, method, 104);
        callbacks.FramePop.unwrap()(env, jni_env, thread, method, jni::JNI_TRUE);
        callbacks.Breakpoint.unwrap()(env, jni_env, thread, method, 105);
        callbacks.FieldAccess.unwrap()(env, jni_env, thread, method, 106, class, object, field);
        callbacks.FieldModification.unwrap()(
            env,
            jni_env,
            thread,
            method,
            107,
            class,
            object,
            field,
            b'J' as _,
            jni::jvalue {
                j: 0x1122_3344_5566_7788,
            },
        );
        callbacks.MethodEntry.unwrap()(env, jni_env, thread, method);
        callbacks.MethodExit.unwrap()(
            env,
            jni_env,
            thread,
            method,
            jni::JNI_FALSE,
            jni::jvalue { j: 0x1234_5678 },
        );
        callbacks.NativeMethodBind.unwrap()(
            env,
            jni_env,
            thread,
            method,
            mut_ptr(ADDRESS),
            &mut new_native_address,
        );
        callbacks.CompiledMethodLoad.unwrap()(
            env,
            method,
            4096,
            const_ptr(ADDRESS),
            map.len() as jni::jint,
            map.as_ptr(),
            const_ptr(COMPILE_INFO),
        );
        callbacks.CompiledMethodUnload.unwrap()(env, method, const_ptr(ADDRESS));
        callbacks.DynamicCodeGenerated.unwrap()(
            env,
            dynamic_name.as_ptr().cast(),
            const_ptr(ADDRESS),
            2048,
        );
        callbacks.DataDumpRequest.unwrap()(env);
        callbacks.MonitorWait.unwrap()(env, jni_env, thread, object, 109);
        callbacks.MonitorWaited.unwrap()(env, jni_env, thread, object, jni::JNI_TRUE);
        callbacks.MonitorContendedEnter.unwrap()(env, jni_env, thread, object);
        callbacks.MonitorContendedEntered.unwrap()(env, jni_env, thread, object);
        callbacks.ResourceExhausted.unwrap()(
            env,
            jni_env,
            0x55aa,
            const_ptr(RESERVED),
            resource_description.as_ptr().cast(),
        );
        callbacks.GarbageCollectionStart.unwrap()(env);
        callbacks.GarbageCollectionFinish.unwrap()(env);
        callbacks.ObjectFree.unwrap()(env, 0x2233_4455_6677_8899);
        callbacks.VMObjectAlloc.unwrap()(env, jni_env, thread, object, class, 8192);
        callbacks.SampledObjectAlloc.unwrap()(env, jni_env, thread, ptr::null_mut(), class, 16384);
        callbacks.VirtualThreadStart.unwrap()(env, jni_env, thread);
        callbacks.VirtualThreadEnd.unwrap()(env, jni_env, thread);
    }

    assert_eq!(new_native_address as usize, ADDRESS + 1);
    assert!(new_class_data.is_null());
    assert_eq!(new_class_data_length, 0);
    assert_eq!(SEEN.load(Ordering::Relaxed), (1_u64 << 34) - 1);

    PANIC_NEXT_METHOD_ENTRY.store(true, Ordering::Relaxed);
    unsafe { callbacks.MethodEntry.unwrap()(env, jni_env, thread, method) };
    assert_eq!(CONTAINED_PANICS.load(Ordering::Relaxed), 1);
}
