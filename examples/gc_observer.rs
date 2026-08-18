//! Count garbage-collection cycles without invoking JNI from GC callbacks.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct GcObserver {
    starts: AtomicU64,
    finishes: AtomicU64,
}

impl Agent for GcObserver {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|caps| caps.set_can_generate_garbage_collection_events(true))
            .is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_GARBAGE_COLLECTION_START,
                    jvmti::JVMTI_EVENT_GARBAGE_COLLECTION_FINISH,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn garbage_collection_start(&self, _context: CallbackContext<'_>) {
        self.starts.fetch_add(1, Ordering::Relaxed);
    }

    fn garbage_collection_finish(&self, _context: CallbackContext<'_>) {
        self.finishes.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[gc] starts={} finishes={}",
            self.starts.load(Ordering::Relaxed),
            self.finishes.load(Ordering::Relaxed)
        );
    }
}

export_agent!(GcObserver);
