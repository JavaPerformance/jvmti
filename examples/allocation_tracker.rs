//! Count VM object-allocation callbacks and their reported byte sizes.
//!
//! `VMObjectAlloc` is intentionally expensive. Prefer `heap_sampler` when
//! statistical allocation data is sufficient.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct AllocationTracker {
    objects: AtomicU64,
    bytes: AtomicU64,
}

impl Agent for AllocationTracker {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|caps| caps.set_can_generate_vm_object_alloc_events(true))
            .is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_VM_OBJECT_ALLOC,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn vm_object_alloc(&self, _context: CallbackContext<'_>, event: ObjectAllocationEvent) {
        self.objects.fetch_add(1, Ordering::Relaxed);
        self.bytes
            .fetch_add(event.size().max(0) as u64, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[allocations] objects={} bytes={}",
            self.objects.load(Ordering::Relaxed),
            self.bytes.load(Ordering::Relaxed)
        );
    }
}

export_agent!(AllocationTracker);
