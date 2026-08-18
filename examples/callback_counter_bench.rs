//! Callback benchmark agent: method-entry delivery plus a relaxed atomic count.

use std::sync::atomic::{AtomicU64, Ordering};

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct CallbackCounterBench {
    callbacks: AtomicU64,
}

impl Agent for CallbackCounterBench {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let jvmti = match context.vm().jvmti() {
            Ok(jvmti) => jvmti,
            Err(_) => return jni::JNI_ERR,
        };

        if jvmti
            .add_capabilities_with(|capabilities| {
                capabilities.set_can_generate_method_entry_events(true);
            })
            .and_then(|_| jvmti.set_default_agent_callbacks())
            .and_then(|_| {
                jvmti.enable_events_global(&[
                    jvmti::JVMTI_EVENT_METHOD_ENTRY,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
            })
            .is_err()
        {
            return jni::JNI_ERR;
        }

        jni::JNI_OK
    }

    fn method_entry(&self, _context: CallbackContext<'_>, _event: MethodEvent) {
        self.callbacks.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "callback_bench_agent=rust_counter callbacks={}",
            self.callbacks.load(Ordering::Relaxed)
        );
    }
}

export_agent!(CallbackCounterBench);
