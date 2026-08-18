//! Distinguish normal method returns from exception-induced frame pops.
//!
//! The `jvalue` union is intentionally left uninterpreted here: decoding it
//! requires the target method's return descriptor.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct MethodExitValues {
    normal: AtomicU64,
    exceptional: AtomicU64,
}

impl Agent for MethodExitValues {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|caps| caps.set_can_generate_method_exit_events(true))
            .is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_METHOD_EXIT,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn method_exit(&self, _context: CallbackContext<'_>, event: MethodExitEvent) {
        if event.return_value().is_some() {
            self.normal.fetch_add(1, Ordering::Relaxed);
        } else {
            self.exceptional.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[method-exit] normal={} exceptional={}",
            self.normal.load(Ordering::Relaxed),
            self.exceptional.load(Ordering::Relaxed)
        );
    }
}

export_agent!(MethodExitValues);
