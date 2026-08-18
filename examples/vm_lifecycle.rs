//! Observe JVM start, initialization, and shutdown.
//!
//! Build with `cargo build --release --example vm_lifecycle`, then run with
//! `java -agentpath:./target/release/examples/libvm_lifecycle.so MyApp`.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
struct VmLifecycle {
    started: AtomicBool,
    initialized: AtomicBool,
}

impl Agent for VmLifecycle {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_VM_START,
                    jvmti::JVMTI_EVENT_VM_INIT,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn vm_start(&self, _context: CallbackContext<'_>) {
        self.started.store(true, Ordering::Release);
    }

    fn vm_init(&self, _context: CallbackContext<'_>, _event: ThreadEvent) {
        self.initialized.store(true, Ordering::Release);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[vm-lifecycle] started={} initialized={}",
            self.started.load(Ordering::Acquire),
            self.initialized.load(Ordering::Acquire)
        );
    }
}

export_agent!(VmLifecycle);
