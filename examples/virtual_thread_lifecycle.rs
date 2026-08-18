//! Count virtual-thread lifecycle events on a JDK that exposes the JVMTI surface.
//!
//! This example intentionally fails agent loading on unsupported runtimes rather
//! than silently pretending that virtual-thread events are active.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct VirtualThreadLifecycle {
    starts: AtomicU64,
    ends: AtomicU64,
}

impl Agent for VirtualThreadLifecycle {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        let configured = jvmti.add_capabilities_with(|caps| {
            caps.set_can_support_virtual_threads(true);
        });
        if configured.is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti.enable_virtual_thread_events().is_err()
            || jvmti
                .enable_events_global(&[jvmti::JVMTI_EVENT_VM_DEATH])
                .is_err()
        {
            eprintln!("[virtual-threads] runtime does not support this JVMTI event surface");
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn virtual_thread_start(&self, _context: CallbackContext<'_>, _event: ThreadEvent) {
        self.starts.fetch_add(1, Ordering::Relaxed);
    }

    fn virtual_thread_end(&self, _context: CallbackContext<'_>, _event: ThreadEvent) {
        self.ends.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[virtual-threads] starts={} ends={}",
            self.starts.load(Ordering::Relaxed),
            self.ends.load(Ordering::Relaxed)
        );
    }
}

export_agent!(VirtualThreadLifecycle);
