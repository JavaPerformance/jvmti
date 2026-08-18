//! Count thrown and caught exceptions.
//!
//! Exception callbacks can be frequent. This example performs only relaxed
//! atomic increments in callbacks and prints once during VM shutdown.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct ExceptionTracker {
    thrown: AtomicU64,
    caught: AtomicU64,
}

impl Agent for ExceptionTracker {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti.configure_exception_agent().is_err()
            || jvmti
                .enable_events_global(&[jvmti::JVMTI_EVENT_VM_DEATH])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn exception(&self, _context: CallbackContext<'_>, _event: ExceptionEvent) {
        self.thrown.fetch_add(1, Ordering::Relaxed);
    }

    fn exception_catch(&self, _context: CallbackContext<'_>, _event: ExceptionCatchEvent) {
        self.caught.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[exceptions] thrown={} caught={}",
            self.thrown.load(Ordering::Relaxed),
            self.caught.load(Ordering::Relaxed)
        );
    }
}

export_agent!(ExceptionTracker);
