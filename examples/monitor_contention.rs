//! Observe Java monitor waits and contention.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct MonitorContention {
    waits: AtomicU64,
    timed_out_waits: AtomicU64,
    contended_enters: AtomicU64,
    contended_entered: AtomicU64,
}

impl Agent for MonitorContention {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|caps| caps.set_can_generate_monitor_events(true))
            .is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_MONITOR_WAIT,
                    jvmti::JVMTI_EVENT_MONITOR_WAITED,
                    jvmti::JVMTI_EVENT_MONITOR_CONTENDED_ENTER,
                    jvmti::JVMTI_EVENT_MONITOR_CONTENDED_ENTERED,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn monitor_wait(&self, _context: CallbackContext<'_>, _event: MonitorWaitEvent) {
        self.waits.fetch_add(1, Ordering::Relaxed);
    }

    fn monitor_waited(&self, _context: CallbackContext<'_>, event: MonitorWaitedEvent) {
        if event.timed_out() {
            self.timed_out_waits.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn monitor_contended_enter(&self, _context: CallbackContext<'_>, _event: MonitorEvent) {
        self.contended_enters.fetch_add(1, Ordering::Relaxed);
    }

    fn monitor_contended_entered(&self, _context: CallbackContext<'_>, _event: MonitorEvent) {
        self.contended_entered.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[monitors] waits={} timed_out={} contended={} acquired={}",
            self.waits.load(Ordering::Relaxed),
            self.timed_out_waits.load(Ordering::Relaxed),
            self.contended_enters.load(Ordering::Relaxed),
            self.contended_entered.load(Ordering::Relaxed)
        );
    }
}

export_agent!(MonitorContention);
