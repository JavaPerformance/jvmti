//! Report JVM resource-exhaustion notifications.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct ResourceExhaustion {
    events: AtomicU64,
    observed_flags: AtomicU64,
}

impl Agent for ResourceExhaustion {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|caps| {
                caps.set_can_generate_resource_exhaustion_heap_events(true);
                caps.set_can_generate_resource_exhaustion_threads_events(true);
            })
            .is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_RESOURCE_EXHAUSTED,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn resource_exhausted(&self, _context: CallbackContext<'_>, event: ResourceExhaustedEvent<'_>) {
        self.events.fetch_add(1, Ordering::Relaxed);
        // Do not allocate, decode strings, or perform I/O while the JVM is
        // already reporting resource exhaustion.
        self.observed_flags
            .fetch_or(event.flags() as u64, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[resource-exhaustion] total={} combined_flags={:#x}",
            self.events.load(Ordering::Relaxed),
            self.observed_flags.load(Ordering::Relaxed)
        );
    }
}

export_agent!(ResourceExhaustion);
