//! Count platform-thread starts and ends without allocating in hot callbacks.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct ThreadLifecycle {
    starts: AtomicU64,
    ends: AtomicU64,
}

impl Agent for ThreadLifecycle {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_THREAD_START,
                    jvmti::JVMTI_EVENT_THREAD_END,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn thread_start(&self, _context: CallbackContext<'_>, _event: ThreadEvent) {
        self.starts.fetch_add(1, Ordering::Relaxed);
    }

    fn thread_end(&self, _context: CallbackContext<'_>, _event: ThreadEvent) {
        self.ends.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[thread-lifecycle] starts={} ends={}",
            self.starts.load(Ordering::Relaxed),
            self.ends.load(Ordering::Relaxed)
        );
    }
}

export_agent!(ThreadLifecycle);
