//! Observe native-method bindings without replacing their implementations.
//!
//! Rebinding requires an exact JNI ABI match and is deliberately not attempted
//! by this safe observation example.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct NativeMethodBind {
    bindings: AtomicU64,
}

impl Agent for NativeMethodBind {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|caps| caps.set_can_generate_native_method_bind_events(true))
            .is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_NATIVE_METHOD_BIND,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn native_method_bind(&self, _context: CallbackContext<'_>, _event: NativeMethodBindEvent) {
        self.bindings.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[native-bind] bindings={}",
            self.bindings.load(Ordering::Relaxed)
        );
    }
}

export_agent!(NativeMethodBind);
