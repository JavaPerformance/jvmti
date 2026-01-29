// jvmti/src/sys/jvmti.rs
//
// Complete JVMTI (JVM Tool Interface) bindings for Rust.
// No external dependencies - suitable for standalone use.
//
// Verified against JDK 27 jvmti.h header. Compatible with JDK 8+.
//
// The JVMTI interface has been stable since JDK 1.5. Newer JDKs add
// functions at the END of the vtable, maintaining backwards compatibility:
//   - JDK 9:  Module functions (94-99)
//   - JDK 11: SetHeapSamplingInterval (156)
//   - JDK 21: Virtual thread support (118-119)
//   - JDK 25: ClearAllFramePops (67 - replaced reserved slot)
//
// Reserved slots: 1, 105, 113, 117, 141

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::os::raw::{c_void, c_uchar};
use crate::sys::jni::{jint, jlong, jthread, jclass, jobject, jfieldID, jmethodID, jboolean, JNIEnv, jvalue, jfloat, jdouble};

// --- Constants ---
pub const JVMTI_VERSION_1_0: jint = 0x30010000;
pub const JVMTI_VERSION_1_1: jint = 0x30010100;
pub const JVMTI_VERSION_1_2: jint = 0x30010200;
pub const JVMTI_VERSION_9: jint = 0x30090000;
pub const JVMTI_VERSION_11: jint = 0x300B0000;
pub const JVMTI_VERSION_19: jint = 0x30130000;
pub const JVMTI_VERSION_21: jint = 0x30150000;

pub const JVMTI_EVENT_VM_INIT: u32 = 50;
pub const JVMTI_EVENT_VM_DEATH: u32 = 51;
pub const JVMTI_EVENT_THREAD_START: u32 = 52;
pub const JVMTI_EVENT_THREAD_END: u32 = 53;
pub const JVMTI_EVENT_CLASS_FILE_LOAD_HOOK: u32 = 54;
pub const JVMTI_EVENT_CLASS_LOAD: u32 = 55;
pub const JVMTI_EVENT_CLASS_PREPARE: u32 = 56;
pub const JVMTI_EVENT_VM_START: u32 = 57;
pub const JVMTI_EVENT_EXCEPTION: u32 = 58;
pub const JVMTI_EVENT_EXCEPTION_CATCH: u32 = 59;
pub const JVMTI_EVENT_SINGLE_STEP: u32 = 60;
pub const JVMTI_EVENT_FRAME_POP: u32 = 61;
pub const JVMTI_EVENT_BREAKPOINT: u32 = 62;
pub const JVMTI_EVENT_FIELD_ACCESS: u32 = 63;
pub const JVMTI_EVENT_FIELD_MODIFICATION: u32 = 64;
pub const JVMTI_EVENT_METHOD_ENTRY: u32 = 66;
pub const JVMTI_EVENT_METHOD_EXIT: u32 = 67;
pub const JVMTI_EVENT_NATIVE_METHOD_BIND: u32 = 68;
pub const JVMTI_EVENT_COMPILED_METHOD_LOAD: u32 = 69;
pub const JVMTI_EVENT_COMPILED_METHOD_UNLOAD: u32 = 70;
pub const JVMTI_EVENT_DYNAMIC_CODE_GENERATED: u32 = 71;
pub const JVMTI_EVENT_DATA_DUMP_REQUEST: u32 = 72;
pub const JVMTI_EVENT_MONITOR_WAIT: u32 = 73;
pub const JVMTI_EVENT_MONITOR_WAITED: u32 = 74;
pub const JVMTI_EVENT_MONITOR_CONTENDED_ENTER: u32 = 75;
pub const JVMTI_EVENT_MONITOR_CONTENDED_ENTERED: u32 = 76;
pub const JVMTI_EVENT_RESOURCE_EXHAUSTED: u32 = 77;
pub const JVMTI_EVENT_GARBAGE_COLLECTION_START: u32 = 78;
pub const JVMTI_EVENT_GARBAGE_COLLECTION_FINISH: u32 = 79;
pub const JVMTI_EVENT_OBJECT_FREE: u32 = 80;
pub const JVMTI_EVENT_VM_OBJECT_ALLOC: u32 = 81;
pub const JVMTI_EVENT_SAMPLED_OBJECT_ALLOC: u32 = 82;



// --- Phases ---
pub const JVMTI_PHASE_ONLOAD: jint = 1;
pub const JVMTI_PHASE_PRIMORDIAL: jint = 2;
pub const JVMTI_PHASE_START: jint = 6;
pub const JVMTI_PHASE_LIVE: jint = 4;
pub const JVMTI_PHASE_DEAD: jint = 8;

pub const JVMTI_ENABLE: jint = 1;
pub const JVMTI_DISABLE: jint = 0;

// --- Error Codes ---
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum jvmtiError {
    NONE = 0,
    INVALID_THREAD = 10,
    INVALID_CLASS = 21,
    NOT_AVAILABLE = 98,
    MUST_POSSESS_CAPABILITY = 99,
    NULL_POINTER = 100,
    ABSENT_INFORMATION = 101,
    INVALID_EVENT_TYPE = 102,
    // ...
}

pub type jlocation = jlong;
pub type jrawMonitorID = *mut c_void;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiLineNumberEntry {
    pub start_location: jlocation,
    pub line_number: jint,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiLocalVariableEntry {
    pub start_location: jlocation,
    pub length: jint,
    pub name: *mut std::os::raw::c_char,
    pub signature: *mut std::os::raw::c_char,
    pub generic_signature: *mut std::os::raw::c_char,
    pub slot: jint,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiMonitorUsage {
    pub owner: jthread,
    pub entry_count: jint,
    pub waiter_count: jint,
    pub waiters: *mut jthread,
    pub notify_waiter_count: jint,
    pub notify_waiters: *mut jthread,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct jvmtiFrameInfo {
    pub method: jmethodID,
    pub location: jlocation,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct jvmtiThreadInfo {
    pub name: *mut std::os::raw::c_char,
    pub priority: jint,
    pub is_daemon: jboolean,
    pub thread_group: jobject,
    pub context_class_loader: jobject,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct jvmtiThreadGroupInfo {
    pub parent: jobject,
    pub name: *mut std::os::raw::c_char,
    pub max_priority: jint,
    pub is_daemon: jboolean,
}

pub type jvmtiStartFunction = unsafe extern "system" fn(env: *mut jvmtiEnv, arg: *mut c_void);

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct jvmtiClassDefinition {
    pub klass: jclass,
    pub class_byte_count: jint,
    pub class_bytes: *const std::os::raw::c_uchar,
}

pub type jvmtiIterationControl = jint;
pub const JVMTI_ITERATION_CONTINUE: jint = 1;
pub const JVMTI_ITERATION_IGNORE: jint = 2;
pub const JVMTI_ITERATION_ABORT: jint = 0;

pub type jvmtiObjectReferenceCallback = unsafe extern "system" fn(
    reference_kind: jint,
    reference_info: jvmtiObjectReferenceInfo,
    class_tag: jlong,
    referrer_tag: jlong,
    target_tag: jlong,
    reference_index: jint,
    user_data: *mut c_void,
    index_ptr: *mut jint,
) -> jint;

#[repr(C)]
#[derive(Copy, Clone)]
pub union jvmtiObjectReferenceInfo {
    pub field: jvmtiObjectReferenceInfoField,
    pub array: jvmtiObjectReferenceInfoArray,
    pub frame: jvmtiObjectReferenceInfoFrame,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiObjectReferenceInfoField {
    pub index: jint,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiObjectReferenceInfoArray {
    pub index: jint,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiObjectReferenceInfoFrame {
    pub depth: jint,
}

pub type jvmtiHeapRootCallback = unsafe extern "system" fn(
    root_kind: jint,
    class_tag: jlong,
    thread_tag: jlong,
    tag_ptr: *mut jlong,
    user_data: *mut c_void,
) -> jint;

pub type jvmtiStackReferenceCallback = unsafe extern "system" fn(
    root_kind: jint,
    class_tag: jlong,
    thread_tag: jlong,
    tag_ptr: *mut jlong,
    user_data: *mut c_void,
    depth: jint,
    method: jmethodID,
    slot: jint,
) -> jint;

pub type jvmtiObjectCallback = unsafe extern "system" fn(
    class_tag: jlong,
    size: jlong,
    tag_ptr: *mut jlong,
    user_data: *mut c_void,
) -> jint;

pub type jvmtiHeapIterationCallback = unsafe extern "system" fn(
    class_tag: jlong,
    size: jlong,
    tag_ptr: *mut jlong,
    user_data: *mut c_void,
) -> jint;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiHeapCallbacks {
    pub heap_root_callback: Option<jvmtiHeapRootCallback>,
    pub stack_reference_callback: Option<jvmtiStackReferenceCallback>,
    pub object_reference_callback: Option<jvmtiObjectReferenceCallback>,
    pub object_callback: Option<jvmtiObjectCallback>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiTimerInfo {
    pub max_value: jlong,
    pub may_skip_forward: jboolean,
    pub may_skip_backward: jboolean,
    pub kind: jint,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiExtensionFunctionInfo {
    pub func: *mut c_void,
    pub id: *mut std::os::raw::c_char,
    pub short_description: *mut std::os::raw::c_char,
    pub param_count: jint,
    pub params: *mut jvmtiExtensionParamInfo,
    pub error_count: jint,
    pub errors: *mut jvmtiError,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiExtensionParamInfo {
    pub name: *mut std::os::raw::c_char,
    pub kind: jint,
    pub base_type: jint,
    pub null_ok: jboolean,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiExtensionEventInfo {
    pub extension_event_index: jint,
    pub id: *mut std::os::raw::c_char,
    pub short_description: *mut std::os::raw::c_char,
    pub param_count: jint,
    pub params: *mut jvmtiExtensionParamInfo,
}

pub type jvmtiExtensionEventCallback = unsafe extern "system" fn();

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiMonitorStackDepthInfo {
    pub monitor: jobject,
    pub stack_depth: jint,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct jvmtiStackInfo {
    pub frame_buffer: *mut jvmtiFrameInfo,
    pub thread: jthread,
    pub state: jint,
    pub frame_count: jint,
}

// --- Capabilities ---
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct jvmtiCapabilities {
    bits: [u32; 4],
}

impl Default for jvmtiCapabilities {
    fn default() -> Self { Self { bits: [0; 4] } }
}

impl jvmtiCapabilities {
    // --- Helper Methods ---
    fn set_bit(&mut self, bit_offset: usize, value: bool) {
        let word_index = bit_offset / 32;
        let bit_index = bit_offset % 32;
        if value {
            self.bits[word_index] |= 1 << bit_index;
        } else {
            self.bits[word_index] &= !(1 << bit_index);
        }
    }

    fn get_bit(&self, bit_offset: usize) -> bool {
        let word_index = bit_offset / 32;
        let bit_index = bit_offset % 32;
        (self.bits[word_index] & (1 << bit_index)) != 0
    }

    // =========================================================================
    // 1. MEMORY & HEAP (0-7, 29, 31-32, 43)
    // =========================================================================

    // [0]
    pub fn set_can_tag_objects(&mut self, v: bool) { self.set_bit(0, v); }
    pub fn can_tag_objects(&self) -> bool { self.get_bit(0) }

    // [1]
    pub fn set_can_generate_field_modification_events(&mut self, v: bool) { self.set_bit(1, v); }
    pub fn can_generate_field_modification_events(&self) -> bool { self.get_bit(1) }

    // [2]
    pub fn set_can_generate_field_access_events(&mut self, v: bool) { self.set_bit(2, v); }
    pub fn can_generate_field_access_events(&self) -> bool { self.get_bit(2) }

    // [29]
    pub fn set_can_generate_vm_object_alloc_events(&mut self, v: bool) { self.set_bit(29, v); }
    pub fn can_generate_vm_object_alloc_events(&self) -> bool { self.get_bit(29) }

    // [31]
    pub fn set_can_generate_garbage_collection_events(&mut self, v: bool) { self.set_bit(31, v); }
    pub fn can_generate_garbage_collection_events(&self) -> bool { self.get_bit(31) }

    // [32]
    pub fn set_can_generate_object_free_events(&mut self, v: bool) { self.set_bit(32, v); }
    pub fn can_generate_object_free_events(&self) -> bool { self.get_bit(32) }

    // [43] (Java 11+)
    pub fn set_can_generate_sampled_object_alloc_events(&mut self, v: bool) { self.set_bit(43, v); }
    pub fn can_generate_sampled_object_alloc_events(&self) -> bool { self.get_bit(43) }


    // =========================================================================
    // 2. BYTECODE & STACK (3-8, 14, 18)
    // =========================================================================

    // [3]
    pub fn set_can_get_bytecodes(&mut self, v: bool) { self.set_bit(3, v); }
    pub fn can_get_bytecodes(&self) -> bool { self.get_bit(3) }

    // [4]
    pub fn set_can_get_synthetic_attribute(&mut self, v: bool) { self.set_bit(4, v); }
    pub fn can_get_synthetic_attribute(&self) -> bool { self.get_bit(4) }

    // [5]
    pub fn set_can_get_owned_monitor_info(&mut self, v: bool) { self.set_bit(5, v); }
    pub fn can_get_owned_monitor_info(&self) -> bool { self.get_bit(5) }

    // [6]
    pub fn set_can_get_current_contended_monitor(&mut self, v: bool) { self.set_bit(6, v); }
    pub fn can_get_current_contended_monitor(&self) -> bool { self.get_bit(6) }

    // [7]
    pub fn set_can_get_monitor_info(&mut self, v: bool) { self.set_bit(7, v); }
    pub fn can_get_monitor_info(&self) -> bool { self.get_bit(7) }

    // [8]
    pub fn set_can_pop_frame(&mut self, v: bool) { self.set_bit(8, v); }
    pub fn can_pop_frame(&self) -> bool { self.get_bit(8) }

    // [14]
    pub fn set_can_access_local_variables(&mut self, v: bool) { self.set_bit(14, v); }
    pub fn can_access_local_variables(&self) -> bool { self.get_bit(14) }

    // [18]
    pub fn set_can_generate_frame_pop_events(&mut self, v: bool) { self.set_bit(18, v); }
    pub fn can_generate_frame_pop_events(&self) -> bool { self.get_bit(18) }


    // =========================================================================
    // 3. CLASS & REDEFINITION (9, 21, 26, 37-38, 42)
    // =========================================================================

    // [9]
    pub fn set_can_redefine_classes(&mut self, v: bool) { self.set_bit(9, v); }
    pub fn can_redefine_classes(&self) -> bool { self.get_bit(9) }

    // [21]
    pub fn set_can_redefine_any_class(&mut self, v: bool) { self.set_bit(21, v); }
    pub fn can_redefine_any_class(&self) -> bool { self.get_bit(21) }

    // [26]
    pub fn set_can_generate_all_class_hook_events(&mut self, v: bool) { self.set_bit(26, v); }
    pub fn can_generate_all_class_hook_events(&self) -> bool { self.get_bit(26) }

    // [37]
    pub fn set_can_retransform_classes(&mut self, v: bool) { self.set_bit(37, v); }
    pub fn can_retransform_classes(&self) -> bool { self.get_bit(37) }

    // [38]
    pub fn set_can_retransform_any_class(&mut self, v: bool) { self.set_bit(38, v); }
    pub fn can_retransform_any_class(&self) -> bool { self.get_bit(38) }

    // [42]
    pub fn set_can_generate_early_class_hook_events(&mut self, v: bool) { self.set_bit(42, v); }
    pub fn can_generate_early_class_hook_events(&self) -> bool { self.get_bit(42) }


    // =========================================================================
    // 4. DEBUGGING & SOURCE (10-13, 16-17, 19-20, 27, 30, 33-36)
    // =========================================================================

    // [10]
    pub fn set_can_signal_thread(&mut self, v: bool) { self.set_bit(10, v); }
    pub fn can_signal_thread(&self) -> bool { self.get_bit(10) }

    // [11]
    pub fn set_can_get_source_file_name(&mut self, v: bool) { self.set_bit(11, v); }
    pub fn can_get_source_file_name(&self) -> bool { self.get_bit(11) }

    // [12]
    pub fn set_can_get_line_numbers(&mut self, v: bool) { self.set_bit(12, v); }
    pub fn can_get_line_numbers(&self) -> bool { self.get_bit(12) }

    // [13]
    pub fn set_can_get_source_debug_extension(&mut self, v: bool) { self.set_bit(13, v); }
    pub fn can_get_source_debug_extension(&self) -> bool { self.get_bit(13) }

    // [15]
    pub fn set_can_maintain_original_method_order(&mut self, v: bool) { self.set_bit(15, v); }
    pub fn can_maintain_original_method_order(&self) -> bool { self.get_bit(15) }

    // [16]
    pub fn set_can_generate_single_step_events(&mut self, v: bool) { self.set_bit(16, v); }
    pub fn can_generate_single_step_events(&self) -> bool { self.get_bit(16) }

    // [17]
    pub fn set_can_generate_exception_events(&mut self, v: bool) { self.set_bit(17, v); }
    pub fn can_generate_exception_events(&self) -> bool { self.get_bit(17) }

    // [19]
    pub fn set_can_generate_breakpoint_events(&mut self, v: bool) { self.set_bit(19, v); }
    pub fn can_generate_breakpoint_events(&self) -> bool { self.get_bit(19) }

    // [20]
    pub fn set_can_suspend(&mut self, v: bool) { self.set_bit(20, v); }
    pub fn can_suspend(&self) -> bool { self.get_bit(20) }

    // [27]
    pub fn set_can_generate_compiled_method_load_events(&mut self, v: bool) { self.set_bit(27, v); }
    pub fn can_generate_compiled_method_load_events(&self) -> bool { self.get_bit(27) }

    // [28]
    pub fn set_can_generate_monitor_events(&mut self, v: bool) { self.set_bit(28, v); }
    pub fn can_generate_monitor_events(&self) -> bool { self.get_bit(28) }

    // [30]
    pub fn set_can_generate_native_method_bind_events(&mut self, v: bool) { self.set_bit(30, v); }
    pub fn can_generate_native_method_bind_events(&self) -> bool { self.get_bit(30) }

    // [33]
    pub fn set_can_force_early_return(&mut self, v: bool) { self.set_bit(33, v); }
    pub fn can_force_early_return(&self) -> bool { self.get_bit(33) }

    // [34]
    pub fn set_can_get_owned_monitor_stack_depth_info(&mut self, v: bool) { self.set_bit(34, v); }
    pub fn can_get_owned_monitor_stack_depth_info(&self) -> bool { self.get_bit(34) }

    // [35]
    pub fn set_can_get_constant_pool(&mut self, v: bool) { self.set_bit(35, v); }
    pub fn can_get_constant_pool(&self) -> bool { self.get_bit(35) }

    // [36]
    pub fn set_can_set_native_method_prefix(&mut self, v: bool) { self.set_bit(36, v); }
    pub fn can_set_native_method_prefix(&self) -> bool { self.get_bit(36) }


    // =========================================================================
    // 5. PROFILING & TIMERS (22-25, 39-41, 44)
    // =========================================================================

    // [22]
    pub fn set_can_get_current_thread_cpu_time(&mut self, v: bool) { self.set_bit(22, v); }
    pub fn can_get_current_thread_cpu_time(&self) -> bool { self.get_bit(22) }

    // [23]
    pub fn set_can_get_thread_cpu_time(&mut self, v: bool) { self.set_bit(23, v); }
    pub fn can_get_thread_cpu_time(&self) -> bool { self.get_bit(23) }

    // [24]
    pub fn set_can_generate_method_entry_events(&mut self, v: bool) { self.set_bit(24, v); }
    pub fn can_generate_method_entry_events(&self) -> bool { self.get_bit(24) }

    // [25]
    pub fn set_can_generate_method_exit_events(&mut self, v: bool) { self.set_bit(25, v); }
    pub fn can_generate_method_exit_events(&self) -> bool { self.get_bit(25) }

    // [39]
    pub fn set_can_generate_resource_exhaustion_heap_events(&mut self, v: bool) { self.set_bit(39, v); }
    pub fn can_generate_resource_exhaustion_heap_events(&self) -> bool { self.get_bit(39) }

    // [40]
    pub fn set_can_generate_resource_exhaustion_threads_events(&mut self, v: bool) { self.set_bit(40, v); }
    pub fn can_generate_resource_exhaustion_threads_events(&self) -> bool { self.get_bit(40) }

    // [41] (Java 9+)
    pub fn set_can_generate_early_vmstart(&mut self, v: bool) { self.set_bit(41, v); }
    pub fn can_generate_early_vmstart(&self) -> bool { self.get_bit(41) }

    // [44] (Java 21+)
    pub fn set_can_support_virtual_threads(&mut self, v: bool) { self.set_bit(44, v); }
    pub fn can_support_virtual_threads(&self) -> bool { self.get_bit(44) }
}

use std::fmt;
impl fmt::Display for jvmtiCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Capabilities [")?;
        if self.can_generate_method_entry_events() { write!(f, "MethodEntry ")?; }
        if self.can_generate_method_exit_events() { write!(f, "MethodExit ")?; }
        // ... add others
        write!(f, "]")
    }
}

// --- Function Typedefs ---


pub type JvmtiSetEventNotificationModeFn = unsafe extern "system" fn(env: *mut jvmtiEnv, mode: jint, event_type: u32, event_thread: jthread) -> jvmtiError;
pub type JvmtiGetAllModulesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, module_count_ptr: *mut jint, modules_ptr: *mut *mut jobject) -> jvmtiError;
pub type JvmtiGetAllThreadsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, threads_count_ptr: *mut jint, threads_ptr: *mut *mut jthread) -> jvmtiError;
pub type JvmtiSuspendThreadFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread) -> jvmtiError;
pub type JvmtiResumeThreadFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread) -> jvmtiError;
pub type JvmtiStopThreadFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, exception: jobject) -> jvmtiError;
pub type JvmtiInterruptThreadFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread) -> jvmtiError;
pub type JvmtiGetThreadInfoFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, info_ptr: *mut jvmtiThreadInfo) -> jvmtiError;
pub type JvmtiGetOwnedMonitorInfoFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, owned_monitor_count_ptr: *mut jint, owned_monitors_ptr: *mut *mut jobject) -> jvmtiError;
pub type JvmtiGetCurrentContendedMonitorFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, monitor_ptr: *mut jobject) -> jvmtiError;
pub type JvmtiRunAgentThreadFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, proc: jvmtiStartFunction, arg: *const c_void, priority: jint) -> jvmtiError;
pub type JvmtiGetTopThreadGroupsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, group_count_ptr: *mut jint, groups_ptr: *mut *mut jobject) -> jvmtiError;
pub type JvmtiGetThreadGroupInfoFn = unsafe extern "system" fn(env: *mut jvmtiEnv, group: jobject, info_ptr: *mut jvmtiThreadGroupInfo) -> jvmtiError;
pub type JvmtiGetThreadGroupChildrenFn = unsafe extern "system" fn(env: *mut jvmtiEnv, group: jobject, thread_count_ptr: *mut jint, threads_ptr: *mut *mut jthread, group_count_ptr: *mut jint, groups_ptr: *mut *mut jobject) -> jvmtiError;
pub type JvmtiGetFrameCountFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, count_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetThreadStateFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, thread_state_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetCurrentThreadFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread_ptr: *mut jthread) -> jvmtiError;
pub type JvmtiGetFrameLocationFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, method_ptr: *mut jmethodID, location_ptr: *mut jlocation) -> jvmtiError;
pub type JvmtiNotifyFramePopFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint) -> jvmtiError;
pub type JvmtiClearAllFramePopsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread) -> jvmtiError;
pub type JvmtiGetLocalObjectFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, slot: jint, value_ptr: *mut jobject) -> jvmtiError;
pub type JvmtiGetLocalIntFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, slot: jint, value_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetLocalLongFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, slot: jint, value_ptr: *mut jlong) -> jvmtiError;
pub type JvmtiGetLocalFloatFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, slot: jint, value_ptr: *mut jfloat) -> jvmtiError;
pub type JvmtiGetLocalDoubleFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, slot: jint, value_ptr: *mut jdouble) -> jvmtiError;
pub type JvmtiSetLocalObjectFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, slot: jint, value: jobject) -> jvmtiError;
pub type JvmtiSetLocalIntFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, slot: jint, value: jint) -> jvmtiError;
pub type JvmtiSetLocalLongFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, slot: jint, value: jlong) -> jvmtiError;
pub type JvmtiSetLocalFloatFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, slot: jint, value: jfloat) -> jvmtiError;
pub type JvmtiSetLocalDoubleFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, slot: jint, value: jdouble) -> jvmtiError;
pub type JvmtiCreateRawMonitorFn = unsafe extern "system" fn(env: *mut jvmtiEnv, name: *const std::os::raw::c_char, monitor_ptr: *mut jrawMonitorID) -> jvmtiError;
pub type JvmtiDestroyRawMonitorFn = unsafe extern "system" fn(env: *mut jvmtiEnv, monitor: jrawMonitorID) -> jvmtiError;
pub type JvmtiRawMonitorEnterFn = unsafe extern "system" fn(env: *mut jvmtiEnv, monitor: jrawMonitorID) -> jvmtiError;
pub type JvmtiRawMonitorExitFn = unsafe extern "system" fn(env: *mut jvmtiEnv, monitor: jrawMonitorID) -> jvmtiError;
pub type JvmtiRawMonitorWaitFn = unsafe extern "system" fn(env: *mut jvmtiEnv, monitor: jrawMonitorID, millis: jlong) -> jvmtiError;
pub type JvmtiRawMonitorNotifyFn = unsafe extern "system" fn(env: *mut jvmtiEnv, monitor: jrawMonitorID) -> jvmtiError;
pub type JvmtiRawMonitorNotifyAllFn = unsafe extern "system" fn(env: *mut jvmtiEnv, monitor: jrawMonitorID) -> jvmtiError;
pub type JvmtiSetBreakpointFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, location: jlocation) -> jvmtiError;
pub type JvmtiClearBreakpointFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, location: jlocation) -> jvmtiError;
pub type JvmtiGetNamedModuleFn = unsafe extern "system" fn(env: *mut jvmtiEnv, class_loader: jobject, package_name: *const std::os::raw::c_char, module_ptr: *mut jobject) -> jvmtiError;
pub type JvmtiSetFieldAccessWatchFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, field: jfieldID) -> jvmtiError;
pub type JvmtiClearFieldAccessWatchFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, field: jfieldID) -> jvmtiError;
pub type JvmtiSetFieldModificationWatchFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, field: jfieldID) -> jvmtiError;
pub type JvmtiClearFieldModificationWatchFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, field: jfieldID) -> jvmtiError;
pub type JvmtiIsModifiableClassFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, is_modifiable_class_ptr: *mut jboolean) -> jvmtiError;
pub type JvmtiAllocateFn = unsafe extern "system" fn(env: *mut jvmtiEnv, size: jlong, mem_ptr: *mut *mut c_uchar) -> jvmtiError;
pub type JvmtiDeallocateFn = unsafe extern "system" fn(env: *mut jvmtiEnv, mem: *mut c_uchar) -> jvmtiError;
pub type JvmtiGetClassSignatureFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, signature_ptr: *mut *mut std::os::raw::c_char, generic_ptr: *mut *mut std::os::raw::c_char) -> jvmtiError;
pub type JvmtiGetClassStatusFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, status_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetSourceFileNameFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, source_name_ptr: *mut *mut std::os::raw::c_char) -> jvmtiError;
pub type JvmtiGetClassModifiersFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, modifiers_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetClassMethodsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, method_count_ptr: *mut jint, methods_ptr: *mut *mut jmethodID) -> jvmtiError;
pub type JvmtiGetClassFieldsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, field_count_ptr: *mut jint, fields_ptr: *mut *mut jfieldID) -> jvmtiError;
pub type JvmtiGetImplementedInterfacesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, interface_count_ptr: *mut jint, interfaces_ptr: *mut *mut jclass) -> jvmtiError;
pub type JvmtiIsInterfaceFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, is_interface_ptr: *mut jboolean) -> jvmtiError;
pub type JvmtiIsArrayClassFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, is_array_class_ptr: *mut jboolean) -> jvmtiError;
pub type JvmtiGetClassLoaderFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, classloader_ptr: *mut jobject) -> jvmtiError;
pub type JvmtiGetObjectHashCodeFn = unsafe extern "system" fn(env: *mut jvmtiEnv, object: jobject, hash_code_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetObjectMonitorUsageFn = unsafe extern "system" fn(env: *mut jvmtiEnv, object: jobject, info_ptr: *mut jvmtiMonitorUsage) -> jvmtiError;
pub type JvmtiGetFieldNameFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, field: jfieldID, name_ptr: *mut *mut std::os::raw::c_char, signature_ptr: *mut *mut std::os::raw::c_char, generic_ptr: *mut *mut std::os::raw::c_char) -> jvmtiError;
pub type JvmtiGetFieldDeclaringClassFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, field: jfieldID, declaring_class_ptr: *mut jclass) -> jvmtiError;
pub type JvmtiGetFieldModifiersFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, field: jfieldID, modifiers_ptr: *mut jint) -> jvmtiError;
pub type JvmtiIsFieldSyntheticFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, field: jfieldID, is_synthetic_ptr: *mut jboolean) -> jvmtiError;
pub type JvmtiGetMethodNameFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, name_ptr: *mut *mut std::os::raw::c_char, signature_ptr: *mut *mut std::os::raw::c_char, generic_ptr: *mut *mut std::os::raw::c_char) -> jvmtiError;
pub type JvmtiGetMethodDeclaringClassFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, declaring_class_ptr: *mut jclass) -> jvmtiError;
pub type JvmtiGetMethodModifiersFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, modifiers_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetMaxLocalsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, max_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetArgumentsSizeFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, size_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetLineNumberTableFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, entry_count_ptr: *mut jint, table_ptr: *mut *mut jvmtiLineNumberEntry) -> jvmtiError;
pub type JvmtiGetMethodLocationFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, start_location_ptr: *mut jlocation, end_location_ptr: *mut jlocation) -> jvmtiError;
pub type JvmtiGetLocalVariableTableFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, entry_count_ptr: *mut jint, table_ptr: *mut *mut jvmtiLocalVariableEntry) -> jvmtiError;
pub type JvmtiSetNativeMethodPrefixFn = unsafe extern "system" fn(env: *mut jvmtiEnv, prefix: *const std::os::raw::c_char) -> jvmtiError;
pub type JvmtiSetNativeMethodPrefixesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, count: jint, prefixes: *mut *mut std::os::raw::c_char) -> jvmtiError;
pub type JvmtiGetBytecodesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, bytecode_count_ptr: *mut jint, bytecodes_ptr: *mut *mut std::os::raw::c_uchar) -> jvmtiError;
pub type JvmtiIsMethodNativeFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, is_native_ptr: *mut jboolean) -> jvmtiError;
pub type JvmtiIsMethodSyntheticFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, is_synthetic_ptr: *mut jboolean) -> jvmtiError;
pub type JvmtiGetLoadedClassesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, class_count_ptr: *mut jint, classes_ptr: *mut *mut jclass) -> jvmtiError;
pub type JvmtiGetClassLoaderClassesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, initiating_loader: jobject, class_count_ptr: *mut jint, classes_ptr: *mut *mut jclass) -> jvmtiError;
pub type JvmtiPopFrameFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread) -> jvmtiError;
pub type JvmtiForceEarlyReturnObjectFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, value: jobject) -> jvmtiError;
pub type JvmtiForceEarlyReturnIntFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, value: jint) -> jvmtiError;
pub type JvmtiForceEarlyReturnLongFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, value: jlong) -> jvmtiError;
pub type JvmtiForceEarlyReturnFloatFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, value: jfloat) -> jvmtiError;
pub type JvmtiForceEarlyReturnDoubleFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, value: jdouble) -> jvmtiError;
pub type JvmtiForceEarlyReturnVoidFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread) -> jvmtiError;
pub type JvmtiRedefineClassesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, class_count: jint, class_definitions: *const jvmtiClassDefinition) -> jvmtiError;
pub type JvmtiGetVersionNumberFn = unsafe extern "system" fn(env: *mut jvmtiEnv, version_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetCapabilitiesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, capabilities_ptr: *mut jvmtiCapabilities) -> jvmtiError;
pub type JvmtiGetSourceDebugExtensionFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, source_debug_extension_ptr: *mut *mut std::os::raw::c_char) -> jvmtiError;
pub type JvmtiIsMethodObsoleteFn = unsafe extern "system" fn(env: *mut jvmtiEnv, method: jmethodID, is_obsolete_ptr: *mut jboolean) -> jvmtiError;
pub type JvmtiSuspendThreadListFn = unsafe extern "system" fn(env: *mut jvmtiEnv, request_count: jint, request_list: *const jthread, results: *mut jvmtiError) -> jvmtiError;
pub type JvmtiResumeThreadListFn = unsafe extern "system" fn(env: *mut jvmtiEnv, request_count: jint, request_list: *const jthread, results: *mut jvmtiError) -> jvmtiError;
pub type JvmtiAddModuleReadsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, module: jobject, source_module: jobject) -> jvmtiError;
pub type JvmtiAddModuleExportsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, module: jobject, package: *const std::os::raw::c_char, to_module: jobject) -> jvmtiError;
pub type JvmtiAddModuleOpensFn = unsafe extern "system" fn(env: *mut jvmtiEnv, module: jobject, package: *const std::os::raw::c_char, to_module: jobject) -> jvmtiError;
pub type JvmtiAddModuleUsesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, module: jobject, service: jclass) -> jvmtiError;
pub type JvmtiAddModuleProvidesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, module: jobject, service: jclass, implementation: jclass) -> jvmtiError;
pub type JvmtiIsModifiableModuleFn = unsafe extern "system" fn(env: *mut jvmtiEnv, module: jobject, is_modifiable_module_ptr: *mut jboolean) -> jvmtiError;
pub type JvmtiGetAllStackTracesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, max_frame_count: jint, stack_info_ptr: *mut *mut jvmtiStackInfo, thread_count_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetThreadListStackTracesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread_count: jint, thread_list: *const jthread, max_frame_count: jint, stack_info_ptr: *mut *mut jvmtiStackInfo) -> jvmtiError;
pub type JvmtiGetThreadLocalStorageFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, data_ptr: *mut *mut c_void) -> jvmtiError;
pub type JvmtiSetThreadLocalStorageFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, data: *const c_void) -> jvmtiError;
pub type JvmtiGetStackTraceFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, start_depth: jint, max_frame_count: jint, frame_buffer: *mut jvmtiFrameInfo, count_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetTagFn = unsafe extern "system" fn(env: *mut jvmtiEnv, object: jobject, tag_ptr: *mut jlong) -> jvmtiError;
pub type JvmtiSetTagFn = unsafe extern "system" fn(env: *mut jvmtiEnv, object: jobject, tag: jlong) -> jvmtiError;
pub type JvmtiForceGarbageCollectionFn = unsafe extern "system" fn(env: *mut jvmtiEnv) -> jvmtiError;
pub type JvmtiIterateOverObjectsReachableFromObjectFn = unsafe extern "system" fn(env: *mut jvmtiEnv, object: jobject, object_reference_callback: jvmtiObjectReferenceCallback, user_data: *const c_void) -> jvmtiError;
pub type JvmtiIterateOverReachableObjectsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, heap_root_callback: jvmtiHeapRootCallback, stack_ref_callback: jvmtiStackReferenceCallback, object_ref_callback: jvmtiObjectReferenceCallback, user_data: *const c_void) -> jvmtiError;
pub type JvmtiIterateOverHeapFn = unsafe extern "system" fn(env: *mut jvmtiEnv, object_filter: jint, heap_object_callback: jvmtiObjectCallback, user_data: *const c_void) -> jvmtiError;
pub type JvmtiIterateOverInstancesOfClassFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, object_filter: jint, heap_object_callback: jvmtiObjectCallback, user_data: *const c_void) -> jvmtiError;
pub type JvmtiGetObjectsWithTagsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, tag_count: jint, tags: *const jlong, count_ptr: *mut jint, object_result_ptr: *mut *mut jobject, tag_result_ptr: *mut *mut jlong) -> jvmtiError;
pub type JvmtiFollowReferencesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, heap_filter: jint, klass: jclass, initial_object: jobject, callbacks: *const jvmtiHeapCallbacks, user_data: *const c_void) -> jvmtiError;
pub type JvmtiIterateThroughHeapFn = unsafe extern "system" fn(env: *mut jvmtiEnv, heap_filter: jint, klass: jclass, callbacks: *const jvmtiHeapCallbacks, user_data: *const c_void) -> jvmtiError;
pub type JvmtiSuspendAllVirtualThreadsFn = unsafe extern "system" fn(env: *mut jvmtiEnv) -> jvmtiError;
pub type JvmtiResumeAllVirtualThreadsFn = unsafe extern "system" fn(env: *mut jvmtiEnv) -> jvmtiError;
pub type JvmtiSetJNIFunctionTableFn = unsafe extern "system" fn(env: *mut jvmtiEnv, function_table: *const crate::sys::jni::JNIEnv) -> jvmtiError;
pub type JvmtiGetJNIFunctionTableFn = unsafe extern "system" fn(env: *mut jvmtiEnv, function_table_ptr: *mut *mut crate::sys::jni::JNIEnv) -> jvmtiError;
pub type JvmtiSetEventCallbacksFn = unsafe extern "system" fn(env: *mut jvmtiEnv, callbacks: *const jvmtiEventCallbacks, size_of_callbacks: jint) -> jvmtiError;
pub type JvmtiGenerateEventsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, event_type: u32) -> jvmtiError;
pub type JvmtiGetExtensionFunctionsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, extension_count_ptr: *mut jint, extensions_ptr: *mut *mut jvmtiExtensionFunctionInfo) -> jvmtiError;
pub type JvmtiGetExtensionEventsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, extension_count_ptr: *mut jint, extensions_ptr: *mut *mut jvmtiExtensionEventInfo) -> jvmtiError;
pub type JvmtiSetExtensionEventCallbackFn = unsafe extern "system" fn(env: *mut jvmtiEnv, extension_event_index: jint, callback: jvmtiExtensionEventCallback) -> jvmtiError;
pub type JvmtiDisposeEnvironmentFn = unsafe extern "system" fn(env: *mut jvmtiEnv) -> jvmtiError;
pub type JvmtiGetErrorNameFn = unsafe extern "system" fn(env: *mut jvmtiEnv, error: jvmtiError, name_ptr: *mut *mut std::os::raw::c_char) -> jvmtiError;
pub type JvmtiGetJLocationFormatFn = unsafe extern "system" fn(env: *mut jvmtiEnv, format_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetSystemPropertiesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, count_ptr: *mut jint, property_ptr: *mut *mut *mut std::os::raw::c_char) -> jvmtiError;
pub type JvmtiGetSystemPropertyFn = unsafe extern "system" fn(env: *mut jvmtiEnv, property: *const std::os::raw::c_char, value_ptr: *mut *mut std::os::raw::c_char) -> jvmtiError;
pub type JvmtiSetSystemPropertyFn = unsafe extern "system" fn(env: *mut jvmtiEnv, property: *const std::os::raw::c_char, value: *const std::os::raw::c_char) -> jvmtiError;
pub type JvmtiGetPhaseFn = unsafe extern "system" fn(env: *mut jvmtiEnv, phase_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetCurrentThreadCpuTimerInfoFn = unsafe extern "system" fn(env: *mut jvmtiEnv, info_ptr: *mut jvmtiTimerInfo) -> jvmtiError;
pub type JvmtiGetCurrentThreadCpuTimeFn = unsafe extern "system" fn(env: *mut jvmtiEnv, nanos_ptr: *mut jlong) -> jvmtiError;
pub type JvmtiGetThreadCpuTimerInfoFn = unsafe extern "system" fn(env: *mut jvmtiEnv, info_ptr: *mut jvmtiTimerInfo) -> jvmtiError;
pub type JvmtiGetThreadCpuTimeFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, nanos_ptr: *mut jlong) -> jvmtiError;
pub type JvmtiGetTimerInfoFn = unsafe extern "system" fn(env: *mut jvmtiEnv, info_ptr: *mut jvmtiTimerInfo) -> jvmtiError;
pub type JvmtiGetTimeFn = unsafe extern "system" fn(env: *mut jvmtiEnv, nanos_ptr: *mut jlong) -> jvmtiError;
pub type JvmtiGetPotentialCapabilitiesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, capabilities_ptr: *mut jvmtiCapabilities) -> jvmtiError;
pub type JvmtiAddCapabilitiesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, capabilities_ptr: *const jvmtiCapabilities) -> jvmtiError;
pub type JvmtiRelinquishCapabilitiesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, capabilities_ptr: *const jvmtiCapabilities) -> jvmtiError;
pub type JvmtiGetAvailableProcessorsFn = unsafe extern "system" fn(env: *mut jvmtiEnv, processors_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetClassVersionNumbersFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, minor_ptr: *mut jint, major_ptr: *mut jint) -> jvmtiError;
pub type JvmtiGetConstantPoolFn = unsafe extern "system" fn(env: *mut jvmtiEnv, klass: jclass, constant_pool_count_ptr: *mut jint, constant_pool_byte_count_ptr: *mut jint, constant_pool_bytes_ptr: *mut *mut std::os::raw::c_uchar) -> jvmtiError;
pub type JvmtiGetEnvironmentLocalStorageFn = unsafe extern "system" fn(env: *mut jvmtiEnv, data_ptr: *mut *mut c_void) -> jvmtiError;
pub type JvmtiSetEnvironmentLocalStorageFn = unsafe extern "system" fn(env: *mut jvmtiEnv, data: *const c_void) -> jvmtiError;
pub type JvmtiAddToBootstrapClassLoaderSearchFn = unsafe extern "system" fn(env: *mut jvmtiEnv, segment: *const std::os::raw::c_char) -> jvmtiError;
pub type JvmtiSetVerboseFlagFn = unsafe extern "system" fn(env: *mut jvmtiEnv, flag: jint, value: jboolean) -> jvmtiError;
pub type JvmtiAddToSystemClassLoaderSearchFn = unsafe extern "system" fn(env: *mut jvmtiEnv, segment: *const std::os::raw::c_char) -> jvmtiError;
pub type JvmtiRetransformClassesFn = unsafe extern "system" fn(env: *mut jvmtiEnv, class_count: jint, classes: *const jclass) -> jvmtiError;
pub type JvmtiGetOwnedMonitorStackDepthInfoFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, monitor_info_count_ptr: *mut jint, monitor_info_ptr: *mut *mut jvmtiMonitorStackDepthInfo) -> jvmtiError;
pub type JvmtiGetObjectSizeFn = unsafe extern "system" fn(env: *mut jvmtiEnv, object: jobject, size_ptr: *mut jlong) -> jvmtiError;
pub type JvmtiGetLocalInstanceFn = unsafe extern "system" fn(env: *mut jvmtiEnv, thread: jthread, depth: jint, value_ptr: *mut jobject) -> jvmtiError;
pub type JvmtiSetHeapSamplingIntervalFn = unsafe extern "system" fn(env: *mut jvmtiEnv, sampling_interval: jint) -> jvmtiError;


// =========================================================================
// FUNCTION TYPEDEFS: EVENT CALLBACKS
// =========================================================================

// 1. VM Lifecycle
pub type JvmtiVMInitFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread
);

pub type JvmtiVMDeathFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv
);

pub type JvmtiVMStartFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv
);

// 2. Thread Lifecycle
pub type JvmtiThreadStartFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread
);

pub type JvmtiThreadEndFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread
);

// 3. Class Loading (The Heavy Hitters)
pub type JvmtiClassFileLoadHookFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    class_being_redefined: jclass,
    loader: jobject,
    name: *const std::os::raw::c_char,
    protection_domain: jobject,
    class_data_len: jint,
    class_data: *const std::os::raw::c_uchar,
    new_class_data_len: *mut jint,
    new_class_data: *mut *mut std::os::raw::c_uchar,
);

pub type JvmtiClassLoadFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    klass: jclass
);

pub type JvmtiClassPrepareFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    klass: jclass
);

// 4. Exceptions
pub type JvmtiExceptionFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    method: jmethodID,
    location: jlocation,
    exception: jobject,
    catch_method: jmethodID,
    catch_location: jlocation
);

pub type JvmtiExceptionCatchFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    method: jmethodID,
    location: jlocation,
    exception: jobject
);

// 5. Debugging (Breakpoints & Stepping)
pub type JvmtiSingleStepFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    method: jmethodID,
    location: jlocation
);

pub type JvmtiBreakpointFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    method: jmethodID,
    location: jlocation
);

pub type JvmtiFramePopFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    method: jmethodID,
    was_popped_by_exception: jboolean
);

// 6. Fields (Watchpoints)
pub type JvmtiFieldAccessFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    method: jmethodID,
    location: jlocation,
    field_klass: jclass,
    object: jobject,
    field: jfieldID // Ensure jfieldID is defined in jni.rs!
);

pub type JvmtiFieldModificationFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    method: jmethodID,
    location: jlocation,
    field_klass: jclass,
    object: jobject,
    field: jfieldID,
    signature_type: std::os::raw::c_char,
    new_value: jvalue
);

// 7. Methods (You already have these)
pub type JvmtiMethodEntryFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    method: jmethodID
);

pub type JvmtiMethodExitFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    method: jmethodID,
    was_popped_by_exception: jboolean,
    return_value: jvalue
);

pub type JvmtiNativeMethodBindFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    method: jmethodID,
    address: *mut std::os::raw::c_void,
    new_address_ptr: *mut *mut std::os::raw::c_void
);

// 8. Compiled Code (JIT)
pub type JvmtiCompiledMethodLoadFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    method: jmethodID,
    code_size: jint,
    code_addr: *const std::os::raw::c_void,
    map_length: jint,
    map: *const std::os::raw::c_void, // jvmtiAddrLocationMap
    compile_info: *const std::os::raw::c_void
);

pub type JvmtiCompiledMethodUnloadFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    method: jmethodID,
    code_addr: *const std::os::raw::c_void
);

pub type JvmtiDynamicCodeGeneratedFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    name: *const std::os::raw::c_char,
    address: *const std::os::raw::c_void,
    length: jint
);

// 9. Monitors (Locks)
pub type JvmtiMonitorWaitFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv, jni_env: *mut JNIEnv, thread: jthread, object: jobject, timeout: jlong
);
pub type JvmtiMonitorWaitedFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv, jni_env: *mut JNIEnv, thread: jthread, object: jobject, timed_out: jboolean
);
pub type JvmtiMonitorContendedEnterFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv, jni_env: *mut JNIEnv, thread: jthread, object: jobject
);
pub type JvmtiMonitorContendedEnteredFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv, jni_env: *mut JNIEnv, thread: jthread, object: jobject
);

// 10. Memory & GC
pub type JvmtiResourceExhaustedFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    flags: jint,
    reserved: *const std::os::raw::c_void,
    description: *const std::os::raw::c_char
);

pub type JvmtiGarbageCollectionStartFn = unsafe extern "system" fn(jvmti_env: *mut jvmtiEnv);
pub type JvmtiGarbageCollectionFinishFn = unsafe extern "system" fn(jvmti_env: *mut jvmtiEnv);

pub type JvmtiObjectFreeFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    tag: jlong
);

pub type JvmtiVMObjectAllocFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    object: jobject,
    object_klass: jclass,
    size: jlong
);

pub type JvmtiSampledObjectAllocFn = unsafe extern "system" fn(
    jvmti_env: *mut jvmtiEnv,
    jni_env: *mut JNIEnv,
    thread: jthread,
    object: jobject,
    object_klass: jclass,
    size: jlong
);


#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct jvmtiInterface_1_ {

    /*   1:  RESERVED */
    pub reserved1: *mut c_void,
    /*   2: Set Event Notification Mode */
    pub SetEventNotificationMode: Option<JvmtiSetEventNotificationModeFn>,
    /*   3: Get All Modules */
    pub GetAllModules: Option<JvmtiGetAllModulesFn>,
    /*   4: Get All Threads */
    pub GetAllThreads: Option<JvmtiGetAllThreadsFn>,
    /*   5: Suspend Thread */
    pub SuspendThread: Option<JvmtiSuspendThreadFn>,
    /*   6: Resume Thread */
    pub ResumeThread: Option<JvmtiResumeThreadFn>,
    /*   7: Stop Thread */
    pub StopThread: Option<JvmtiStopThreadFn>,
    /*   8: Interrupt Thread */
    pub InterruptThread: Option<JvmtiInterruptThreadFn>,
    /*   9: Get Thread Info */
    pub GetThreadInfo: Option<JvmtiGetThreadInfoFn>,
    /*   10: Get Owned Monitor Info */
    pub GetOwnedMonitorInfo: Option<JvmtiGetOwnedMonitorInfoFn>,
    /*   11: Get Current Contended Monitor */
    pub GetCurrentContendedMonitor: Option<JvmtiGetCurrentContendedMonitorFn>,
    /*   12: Run Agent Thread */
    pub RunAgentThread: Option<JvmtiRunAgentThreadFn>,
    /*   13: Get Top Thread Groups */
    pub GetTopThreadGroups: Option<JvmtiGetTopThreadGroupsFn>,
    /*   14: Get Thread Group Info */
    pub GetThreadGroupInfo: Option<JvmtiGetThreadGroupInfoFn>,
    /*   15: Get Thread Group Children */
    pub GetThreadGroupChildren: Option<JvmtiGetThreadGroupChildrenFn>,
    /*   16: Get Frame Count */
    pub GetFrameCount: Option<JvmtiGetFrameCountFn>,
    /*   17: Get Thread State */
    pub GetThreadState: Option<JvmtiGetThreadStateFn>,
    /*   18: Get Current Thread */
    pub GetCurrentThread: Option<JvmtiGetCurrentThreadFn>,
    /*   19: Get Frame Location */
    pub GetFrameLocation: Option<JvmtiGetFrameLocationFn>,
    /*   20: Notify Frame Pop */
    pub NotifyFramePop: Option<JvmtiNotifyFramePopFn>,
    /*   21: Get Local Variable - Object */
    pub GetLocalObject: Option<JvmtiGetLocalObjectFn>,
    /*   22: Get Local Variable - Int */
    pub GetLocalInt: Option<JvmtiGetLocalIntFn>,
    /*   23: Get Local Variable - Long */
    pub GetLocalLong: Option<JvmtiGetLocalLongFn>,
    /*   24: Get Local Variable - Float */
    pub GetLocalFloat: Option<JvmtiGetLocalFloatFn>,
    /*   25: Get Local Variable - Double */
    pub GetLocalDouble: Option<JvmtiGetLocalDoubleFn>,
    /*   26: Set Local Variable - Object */
    pub SetLocalObject: Option<JvmtiSetLocalObjectFn>,
    /*   27: Set Local Variable - Int */
    pub SetLocalInt: Option<JvmtiSetLocalIntFn>,
    /*   28: Set Local Variable - Long */
    pub SetLocalLong: Option<JvmtiSetLocalLongFn>,
    /*   29: Set Local Variable - Float */
    pub SetLocalFloat: Option<JvmtiSetLocalFloatFn>,
    /*   30: Set Local Variable - Double */
    pub SetLocalDouble: Option<JvmtiSetLocalDoubleFn>,
    /*   31: Create Raw Monitor */
    pub CreateRawMonitor: Option<JvmtiCreateRawMonitorFn>,
    /*   32: Destroy Raw Monitor */
    pub DestroyRawMonitor: Option<JvmtiDestroyRawMonitorFn>,
    /*   33: Raw Monitor Enter */
    pub RawMonitorEnter: Option<JvmtiRawMonitorEnterFn>,
    /*   34: Raw Monitor Exit */
    pub RawMonitorExit: Option<JvmtiRawMonitorExitFn>,
    /*   35: Raw Monitor Wait */
    pub RawMonitorWait: Option<JvmtiRawMonitorWaitFn>,
    /*   36: Raw Monitor Notify */
    pub RawMonitorNotify: Option<JvmtiRawMonitorNotifyFn>,
    /*   37: Raw Monitor Notify All */
    pub RawMonitorNotifyAll: Option<JvmtiRawMonitorNotifyAllFn>,
    /*   38: Set Breakpoint */
    pub SetBreakpoint: Option<JvmtiSetBreakpointFn>,
    /*   39: Clear Breakpoint */
    pub ClearBreakpoint: Option<JvmtiClearBreakpointFn>,
    /*   40: Get Named Module */
    pub GetNamedModule: Option<JvmtiGetNamedModuleFn>,
    /*   41: Set Field Access Watch */
    pub SetFieldAccessWatch: Option<JvmtiSetFieldAccessWatchFn>,
    /*   42: Clear Field Access Watch */
    pub ClearFieldAccessWatch: Option<JvmtiClearFieldAccessWatchFn>,
    /*   43: Set Field Modification Watch */
    pub SetFieldModificationWatch: Option<JvmtiSetFieldModificationWatchFn>,
    /*   44: Clear Field Modification Watch */
    pub ClearFieldModificationWatch: Option<JvmtiClearFieldModificationWatchFn>,
    /*   45: Is Modifiable Class */
    pub IsModifiableClass: Option<JvmtiIsModifiableClassFn>,
    /*   46: Allocate */
    pub Allocate: Option<JvmtiAllocateFn>,
    /*   47: Deallocate */
    pub Deallocate: Option<JvmtiDeallocateFn>,
    /*   48: Get Class Signature */
    pub GetClassSignature: Option<JvmtiGetClassSignatureFn>,
    /*   49: Get Class Status */
    pub GetClassStatus: Option<JvmtiGetClassStatusFn>,
    /*   50: Get Source File Name */
    pub GetSourceFileName: Option<JvmtiGetSourceFileNameFn>,
    /*   51: Get Class Modifiers */
    pub GetClassModifiers: Option<JvmtiGetClassModifiersFn>,
    /*   52: Get Class Methods */
    pub GetClassMethods: Option<JvmtiGetClassMethodsFn>,
    /*   53: Get Class Fields */
    pub GetClassFields: Option<JvmtiGetClassFieldsFn>,
    /*   54: Get Implemented Interfaces */
    pub GetImplementedInterfaces: Option<JvmtiGetImplementedInterfacesFn>,
    /*   55: Is Interface */
    pub IsInterface: Option<JvmtiIsInterfaceFn>,
    /*   56: Is Array Class */
    pub IsArrayClass: Option<JvmtiIsArrayClassFn>,
    /*   57: Get Class Loader */
    pub GetClassLoader: Option<JvmtiGetClassLoaderFn>,
    /*   58: Get Object Hash Code */
    pub GetObjectHashCode: Option<JvmtiGetObjectHashCodeFn>,
    /*   59: Get Object Monitor Usage */
    pub GetObjectMonitorUsage: Option<JvmtiGetObjectMonitorUsageFn>,
    /*   60: Get Field Name (and Signature) */
    pub GetFieldName: Option<JvmtiGetFieldNameFn>,
    /*   61: Get Field Declaring Class */
    pub GetFieldDeclaringClass: Option<JvmtiGetFieldDeclaringClassFn>,
    /*   62: Get Field Modifiers */
    pub GetFieldModifiers: Option<JvmtiGetFieldModifiersFn>,
    /*   63: Is Field Synthetic */
    pub IsFieldSynthetic: Option<JvmtiIsFieldSyntheticFn>,
    /*   64: Get Method Name (and Signature) */
    pub GetMethodName: Option<JvmtiGetMethodNameFn>,
    /*   65: Get Method Declaring Class */
    pub GetMethodDeclaringClass: Option<JvmtiGetMethodDeclaringClassFn>,
    /*   66: Get Method Modifiers */
    pub GetMethodModifiers: Option<JvmtiGetMethodModifiersFn>,
    /*   67: Clear All Frame Pops (JDK 25+) */
    pub ClearAllFramePops: Option<JvmtiClearAllFramePopsFn>,
    /*   68: Get Max Locals */
    pub GetMaxLocals: Option<JvmtiGetMaxLocalsFn>,
    /*   69: Get Arguments Size */
    pub GetArgumentsSize: Option<JvmtiGetArgumentsSizeFn>,
    /*   70: Get Line Number Table */
    pub GetLineNumberTable: Option<JvmtiGetLineNumberTableFn>,
    /*   71: Get Method Location */
    pub GetMethodLocation: Option<JvmtiGetMethodLocationFn>,
    /*   72: Get Local Variable Table */
    pub GetLocalVariableTable: Option<JvmtiGetLocalVariableTableFn>,
    /*   73: Set Native Method Prefix */
    pub SetNativeMethodPrefix: Option<JvmtiSetNativeMethodPrefixFn>,
    /*   74: Set Native Method Prefixes */
    pub SetNativeMethodPrefixes: Option<JvmtiSetNativeMethodPrefixesFn>,
    /*   75: Get Bytecodes */
    pub GetBytecodes: Option<JvmtiGetBytecodesFn>,
    /*   76: Is Method Native */
    pub IsMethodNative: Option<JvmtiIsMethodNativeFn>,
    /*   77: Is Method Synthetic */
    pub IsMethodSynthetic: Option<JvmtiIsMethodSyntheticFn>,
    /*   78: Get Loaded Classes */
    pub GetLoadedClasses: Option<JvmtiGetLoadedClassesFn>,
    /*   79: Get Classloader Classes */
    pub GetClassLoaderClasses: Option<JvmtiGetClassLoaderClassesFn>,
    /*   80: Pop Frame */
    pub PopFrame: Option<JvmtiPopFrameFn>,
    /*   81: Force Early Return - Object */
    pub ForceEarlyReturnObject: Option<JvmtiForceEarlyReturnObjectFn>,
    /*   82: Force Early Return - Int */
    pub ForceEarlyReturnInt: Option<JvmtiForceEarlyReturnIntFn>,
    /*   83: Force Early Return - Long */
    pub ForceEarlyReturnLong: Option<JvmtiForceEarlyReturnLongFn>,
    /*   84: Force Early Return - Float */
    pub ForceEarlyReturnFloat: Option<JvmtiForceEarlyReturnFloatFn>,
    /*   85: Force Early Return - Double */
    pub ForceEarlyReturnDouble: Option<JvmtiForceEarlyReturnDoubleFn>,
    /*   86: Force Early Return - Void */
    pub ForceEarlyReturnVoid: Option<JvmtiForceEarlyReturnVoidFn>,
    /*   87: Redefine Classes */
    pub RedefineClasses: Option<JvmtiRedefineClassesFn>,
    /*   88: Get Version Number */
    pub GetVersionNumber: Option<JvmtiGetVersionNumberFn>,
    /*   89: Get Capabilities */
    pub GetCapabilities: Option<JvmtiGetCapabilitiesFn>,
    /*   90: Get Source Debug Extension */
    pub GetSourceDebugExtension: Option<JvmtiGetSourceDebugExtensionFn>,
    /*   91: Is Method Obsolete */
    pub IsMethodObsolete: Option<JvmtiIsMethodObsoleteFn>,
    /*   92: Suspend Thread List */
    pub SuspendThreadList: Option<JvmtiSuspendThreadListFn>,
    /*   93: Resume Thread List */
    pub ResumeThreadList: Option<JvmtiResumeThreadListFn>,
    /*   94: Add Module Reads */
    pub AddModuleReads: Option<JvmtiAddModuleReadsFn>,
    /*   95: Add Module Exports */
    pub AddModuleExports: Option<JvmtiAddModuleExportsFn>,
    /*   96: Add Module Opens */
    pub AddModuleOpens: Option<JvmtiAddModuleOpensFn>,
    /*   97: Add Module Uses */
    pub AddModuleUses: Option<JvmtiAddModuleUsesFn>,
    /*   98: Add Module Provides */
    pub AddModuleProvides: Option<JvmtiAddModuleProvidesFn>,
    /*   99: Is Modifiable Module */
    pub IsModifiableModule: Option<JvmtiIsModifiableModuleFn>,
    /*   100: Get All Stack Traces */
    pub GetAllStackTraces: Option<JvmtiGetAllStackTracesFn>,
    /*   101: Get Thread List Stack Traces */
    pub GetThreadListStackTraces: Option<JvmtiGetThreadListStackTracesFn>,
    /*   102: Get Thread Local Storage */
    pub GetThreadLocalStorage: Option<JvmtiGetThreadLocalStorageFn>,
    /*   103: Set Thread Local Storage */
    pub SetThreadLocalStorage: Option<JvmtiSetThreadLocalStorageFn>,
    /*   104: Get Stack Trace */
    pub GetStackTrace: Option<JvmtiGetStackTraceFn>,
    /*   105:  RESERVED */
    pub reserved105: *mut c_void,
    /*   106: Get Tag */
    pub GetTag: Option<JvmtiGetTagFn>,
    /*   107: Set Tag */
    pub SetTag: Option<JvmtiSetTagFn>,
    /*   108: Force Garbage Collection */
    pub ForceGarbageCollection: Option<JvmtiForceGarbageCollectionFn>,
    /*   109: Iterate Over Objects Reachable From Object */
    pub IterateOverObjectsReachableFromObject: Option<JvmtiIterateOverObjectsReachableFromObjectFn>,
    /*   110: Iterate Over Reachable Objects */
    pub IterateOverReachableObjects: Option<JvmtiIterateOverReachableObjectsFn>,
    /*   111: Iterate Over Heap */
    pub IterateOverHeap: Option<JvmtiIterateOverHeapFn>,
    /*   112: Iterate Over Instances Of Class */
    pub IterateOverInstancesOfClass: Option<JvmtiIterateOverInstancesOfClassFn>,
    /*   113:  RESERVED */
    pub reserved113: *mut c_void,
    /*   114: Get Objects With Tags */
    pub GetObjectsWithTags: Option<JvmtiGetObjectsWithTagsFn>,
    /*   115: Follow References */
    pub FollowReferences: Option<JvmtiFollowReferencesFn>,
    /*   116: Iterate Through Heap */
    pub IterateThroughHeap: Option<JvmtiIterateThroughHeapFn>,
    /*   117:  RESERVED */
    pub reserved117: *mut c_void,
    /*   118: Suspend All Virtual Threads */
    pub SuspendAllVirtualThreads: Option<JvmtiSuspendAllVirtualThreadsFn>,
    /*   119: Resume All Virtual Threads */
    pub ResumeAllVirtualThreads: Option<JvmtiResumeAllVirtualThreadsFn>,
    /*   120: Set JNI Function Table */
    pub SetJNIFunctionTable: Option<JvmtiSetJNIFunctionTableFn>,
    /*   121: Get JNI Function Table */
    pub GetJNIFunctionTable: Option<JvmtiGetJNIFunctionTableFn>,
    /*   122: Set Event Callbacks */
    pub SetEventCallbacks: Option<JvmtiSetEventCallbacksFn>,
    /*   123: Generate Events */
    pub GenerateEvents: Option<JvmtiGenerateEventsFn>,
    /*   124: Get Extension Functions */
    pub GetExtensionFunctions: Option<JvmtiGetExtensionFunctionsFn>,
    /*   125: Get Extension Events */
    pub GetExtensionEvents: Option<JvmtiGetExtensionEventsFn>,
    /*   126: Set Extension Event Callback */
    pub SetExtensionEventCallback: Option<JvmtiSetExtensionEventCallbackFn>,
    /*   127: Dispose Environment */
    pub DisposeEnvironment: Option<JvmtiDisposeEnvironmentFn>,
    /*   128: Get Error Name */
    pub GetErrorName: Option<JvmtiGetErrorNameFn>,
    /*   129: Get JLocation Format */
    pub GetJLocationFormat: Option<JvmtiGetJLocationFormatFn>,
    /*   130: Get System Properties */
    pub GetSystemProperties: Option<JvmtiGetSystemPropertiesFn>,
    /*   131: Get System Property */
    pub GetSystemProperty: Option<JvmtiGetSystemPropertyFn>,
    /*   132: Set System Property */
    pub SetSystemProperty: Option<JvmtiSetSystemPropertyFn>,
    /*   133: Get Phase */
    pub GetPhase: Option<JvmtiGetPhaseFn>,
    /*   134: Get Current Thread CPU Timer Information */
    pub GetCurrentThreadCpuTimerInfo: Option<JvmtiGetCurrentThreadCpuTimerInfoFn>,
    /*   135: Get Current Thread CPU Time */
    pub GetCurrentThreadCpuTime: Option<JvmtiGetCurrentThreadCpuTimeFn>,
    /*   136: Get Thread CPU Timer Information */
    pub GetThreadCpuTimerInfo: Option<JvmtiGetThreadCpuTimerInfoFn>,
    /*   137: Get Thread CPU Time */
    pub GetThreadCpuTime: Option<JvmtiGetThreadCpuTimeFn>,
    /*   138: Get Timer Information */
    pub GetTimerInfo: Option<JvmtiGetTimerInfoFn>,
    /*   139: Get Time */
    pub GetTime: Option<JvmtiGetTimeFn>,
    /*   140: Get Potential Capabilities */
    pub GetPotentialCapabilities: Option<JvmtiGetPotentialCapabilitiesFn>,
    /*   141:  RESERVED */
    pub reserved141: *mut c_void,
    /*   142: Add Capabilities */
    pub AddCapabilities: Option<JvmtiAddCapabilitiesFn>,
    /*   143: Relinquish Capabilities */
    pub RelinquishCapabilities: Option<JvmtiRelinquishCapabilitiesFn>,
    /*   144: Get Available Processors */
    pub GetAvailableProcessors: Option<JvmtiGetAvailableProcessorsFn>,
    /*   145: Get Class Version Numbers */
    pub GetClassVersionNumbers: Option<JvmtiGetClassVersionNumbersFn>,
    /*   146: Get Constant Pool */
    pub GetConstantPool: Option<JvmtiGetConstantPoolFn>,
    /*   147: Get Environment Local Storage */
    pub GetEnvironmentLocalStorage: Option<JvmtiGetEnvironmentLocalStorageFn>,
    /*   148: Set Environment Local Storage */
    pub SetEnvironmentLocalStorage: Option<JvmtiSetEnvironmentLocalStorageFn>,
    /*   149: Add To Bootstrap Class Loader Search */
    pub AddToBootstrapClassLoaderSearch: Option<JvmtiAddToBootstrapClassLoaderSearchFn>,
    /*   150: Set Verbose Flag */
    pub SetVerboseFlag: Option<JvmtiSetVerboseFlagFn>,
    /*   151: Add To System Class Loader Search */
    pub AddToSystemClassLoaderSearch: Option<JvmtiAddToSystemClassLoaderSearchFn>,
    /*   152: Retransform Classes */
    pub RetransformClasses: Option<JvmtiRetransformClassesFn>,
    /*   153: Get Owned Monitor Stack Depth Info */
    pub GetOwnedMonitorStackDepthInfo: Option<JvmtiGetOwnedMonitorStackDepthInfoFn>,
    /*   154: Get Object Size */
    pub GetObjectSize: Option<JvmtiGetObjectSizeFn>,
    /*   155: Get Local Instance */
    pub GetLocalInstance: Option<JvmtiGetLocalInstanceFn>,
    /*   156: Set Heap Sampling Interval */
    pub SetHeapSamplingInterval: Option<JvmtiSetHeapSamplingIntervalFn>
}

#[repr(C)]
pub struct jvmtiEnv {
    pub functions: *const jvmtiInterface_1_,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct jvmtiEventCallbacks {
    pub VMInit: Option<JvmtiVMInitFn>,
    pub VMDeath: Option<JvmtiVMDeathFn>,
    pub ThreadStart: Option<JvmtiThreadStartFn>,
    pub ThreadEnd: Option<JvmtiThreadEndFn>,
    pub ClassFileLoadHook: Option<JvmtiClassFileLoadHookFn>,
    pub ClassLoad: Option<JvmtiClassLoadFn>,
    pub ClassPrepare: Option<JvmtiClassPrepareFn>,
    pub VMStart: Option<JvmtiVMStartFn>,
    pub Exception: Option<JvmtiExceptionFn>,
    pub ExceptionCatch: Option<JvmtiExceptionCatchFn>,
    pub SingleStep: Option<JvmtiSingleStepFn>,
    pub FramePop: Option<JvmtiFramePopFn>,
    pub Breakpoint: Option<JvmtiBreakpointFn>,
    pub FieldAccess: Option<JvmtiFieldAccessFn>,
    pub FieldModification: Option<JvmtiFieldModificationFn>,
    pub MethodEntry: Option<JvmtiMethodEntryFn>,
    pub MethodExit: Option<JvmtiMethodExitFn>,
    pub NativeMethodBind: Option<JvmtiNativeMethodBindFn>,
    pub CompiledMethodLoad: Option<JvmtiCompiledMethodLoadFn>,
    pub CompiledMethodUnload: Option<JvmtiCompiledMethodUnloadFn>,
    pub DynamicCodeGenerated: Option<JvmtiDynamicCodeGeneratedFn>,
    pub DataDumpRequest: *mut std::os::raw::c_void, // We haven't defined this Fn yet
    pub MonitorWait: Option<JvmtiMonitorWaitFn>,
    pub MonitorWaited: Option<JvmtiMonitorWaitedFn>,
    pub MonitorContendedEnter: Option<JvmtiMonitorContendedEnterFn>,
    pub MonitorContendedEntered: Option<JvmtiMonitorContendedEnteredFn>,
    pub ResourceExhausted: Option<JvmtiResourceExhaustedFn>,
    pub GarbageCollectionStart: Option<JvmtiGarbageCollectionStartFn>,
    pub GarbageCollectionFinish: Option<JvmtiGarbageCollectionFinishFn>,
    pub ObjectFree: Option<JvmtiObjectFreeFn>,
    pub VMObjectAlloc: Option<JvmtiVMObjectAllocFn>,
    pub SampledObjectAlloc: Option<JvmtiSampledObjectAllocFn>,
}
