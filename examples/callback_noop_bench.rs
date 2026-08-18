//! Callback benchmark agent: method-entry delivery with an empty handler.

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct CallbackNoopBench;

impl Agent for CallbackNoopBench {
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
            .and_then(|_| jvmti.enable_events_global(&[jvmti::JVMTI_EVENT_METHOD_ENTRY]))
            .is_err()
        {
            return jni::JNI_ERR;
        }

        jni::JNI_OK
    }

    // The default Agent::method_entry implementation is intentionally used.
}

export_agent!(CallbackNoopBench);
