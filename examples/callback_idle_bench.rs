//! Callback benchmark control agent: loaded, but no JVMTI events enabled.

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct CallbackIdleBench;

impl Agent for CallbackIdleBench {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        match context.vm().jvmti() {
            Ok(_) => jni::JNI_OK,
            Err(_) => jni::JNI_ERR,
        }
    }
}

export_agent!(CallbackIdleBench);
