use std::mem::{align_of, size_of, MaybeUninit};
use std::ptr::addr_of;

use jvmti_bindings::sys::jvmti;

macro_rules! offset_of {
    ($ty:ty, $field:ident) => {{
        let value = MaybeUninit::<$ty>::uninit();
        let base = value.as_ptr();
        // `addr_of!` forms the field address without reading uninitialized data.
        let field = unsafe { addr_of!((*base).$field) };
        field as usize - base as usize
    }};
}

#[test]
fn event_ids_match_the_jvmti_specification() {
    let expected = [
        (jvmti::JVMTI_EVENT_VM_INIT, 50),
        (jvmti::JVMTI_EVENT_VM_DEATH, 51),
        (jvmti::JVMTI_EVENT_THREAD_START, 52),
        (jvmti::JVMTI_EVENT_THREAD_END, 53),
        (jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK, 54),
        (jvmti::JVMTI_EVENT_CLASS_LOAD, 55),
        (jvmti::JVMTI_EVENT_CLASS_PREPARE, 56),
        (jvmti::JVMTI_EVENT_VM_START, 57),
        (jvmti::JVMTI_EVENT_EXCEPTION, 58),
        (jvmti::JVMTI_EVENT_EXCEPTION_CATCH, 59),
        (jvmti::JVMTI_EVENT_SINGLE_STEP, 60),
        (jvmti::JVMTI_EVENT_FRAME_POP, 61),
        (jvmti::JVMTI_EVENT_BREAKPOINT, 62),
        (jvmti::JVMTI_EVENT_FIELD_ACCESS, 63),
        (jvmti::JVMTI_EVENT_FIELD_MODIFICATION, 64),
        (jvmti::JVMTI_EVENT_METHOD_ENTRY, 65),
        (jvmti::JVMTI_EVENT_METHOD_EXIT, 66),
        (jvmti::JVMTI_EVENT_NATIVE_METHOD_BIND, 67),
        (jvmti::JVMTI_EVENT_COMPILED_METHOD_LOAD, 68),
        (jvmti::JVMTI_EVENT_COMPILED_METHOD_UNLOAD, 69),
        (jvmti::JVMTI_EVENT_DYNAMIC_CODE_GENERATED, 70),
        (jvmti::JVMTI_EVENT_DATA_DUMP_REQUEST, 71),
        (jvmti::JVMTI_EVENT_MONITOR_WAIT, 73),
        (jvmti::JVMTI_EVENT_MONITOR_WAITED, 74),
        (jvmti::JVMTI_EVENT_MONITOR_CONTENDED_ENTER, 75),
        (jvmti::JVMTI_EVENT_MONITOR_CONTENDED_ENTERED, 76),
        (jvmti::JVMTI_EVENT_RESOURCE_EXHAUSTED, 80),
        (jvmti::JVMTI_EVENT_GARBAGE_COLLECTION_START, 81),
        (jvmti::JVMTI_EVENT_GARBAGE_COLLECTION_FINISH, 82),
        (jvmti::JVMTI_EVENT_OBJECT_FREE, 83),
        (jvmti::JVMTI_EVENT_VM_OBJECT_ALLOC, 84),
        (jvmti::JVMTI_EVENT_SAMPLED_OBJECT_ALLOC, 86),
        (jvmti::JVMTI_EVENT_VIRTUAL_THREAD_START, 87),
        (jvmti::JVMTI_EVENT_VIRTUAL_THREAD_END, 88),
    ];

    for (actual, expected) in expected {
        assert_eq!(actual, expected);
    }
}

#[test]
fn callback_table_matches_jdk_8_through_27_abi() {
    type Callbacks = jvmti::jvmtiEventCallbacks;
    let pointer = size_of::<*const ()>();

    assert_eq!(align_of::<Callbacks>(), align_of::<*const ()>());
    assert_eq!(size_of::<Option<jvmti::JvmtiEventReservedFn>>(), pointer);

    let slots = [
        (offset_of!(Callbacks, VMInit), 0),
        (offset_of!(Callbacks, FieldModification), 14),
        (offset_of!(Callbacks, MethodEntry), 15),
        (offset_of!(Callbacks, DataDumpRequest), 21),
        (offset_of!(Callbacks, reserved72), 22),
        (offset_of!(Callbacks, MonitorWait), 23),
        (offset_of!(Callbacks, MonitorContendedEntered), 26),
        (offset_of!(Callbacks, reserved77), 27),
        (offset_of!(Callbacks, reserved78), 28),
        (offset_of!(Callbacks, reserved79), 29),
        (offset_of!(Callbacks, ResourceExhausted), 30),
        (offset_of!(Callbacks, VMObjectAlloc), 34),
        (offset_of!(Callbacks, reserved85), 35),
        (offset_of!(Callbacks, SampledObjectAlloc), 36),
        (offset_of!(Callbacks, VirtualThreadStart), 37),
        (offset_of!(Callbacks, VirtualThreadEnd), 38),
    ];

    for (actual_offset, expected_slot) in slots {
        assert_eq!(actual_offset, expected_slot * pointer);
    }

    // The JVM copies only the prefix it understands. These are the native
    // callback-table sizes through the final event known to each generation.
    assert_eq!(offset_of!(Callbacks, VMObjectAlloc) + pointer, 35 * pointer); // JDK 8-10
    assert_eq!(
        offset_of!(Callbacks, SampledObjectAlloc) + pointer,
        37 * pointer
    ); // JDK 11-20
    assert_eq!(size_of::<Callbacks>(), 39 * pointer); // JDK 21+
}

#[test]
fn default_callback_table_wires_new_events_and_keeps_reserved_slots_null() {
    let callbacks = jvmti_bindings::get_default_callbacks();

    assert!(callbacks.DataDumpRequest.is_some());
    assert!(callbacks.SampledObjectAlloc.is_some());
    assert!(callbacks.VirtualThreadStart.is_some());
    assert!(callbacks.VirtualThreadEnd.is_some());
    assert!(callbacks.reserved72.is_none());
    assert!(callbacks.reserved77.is_none());
    assert!(callbacks.reserved78.is_none());
    assert!(callbacks.reserved79.is_none());
    assert!(callbacks.reserved85.is_none());
}
