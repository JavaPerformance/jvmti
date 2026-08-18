//! Handle JVMTI data-dump requests.
//!
//! The JVM decides how this event is triggered on each platform. Keep the
//! callback small; real agents should hand work to an agent-owned worker.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct DataDumpRequest {
    requests: AtomicU64,
}

impl Agent for DataDumpRequest {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_DATA_DUMP_REQUEST,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn data_dump_request(&self, _context: CallbackContext<'_>) {
        self.requests.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[data-dump] requests={}",
            self.requests.load(Ordering::Relaxed)
        );
    }
}

export_agent!(DataDumpRequest);
