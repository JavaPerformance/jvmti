//! Example: dynamic attach via Agent_OnAttach.
//!
//! Build as a cdylib and load via the JVM Attach API.
//! This example keeps behavior minimal and just logs the options string.

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct AttachLogger;

impl Agent for AttachLogger {
    fn on_load(&self, _context: AgentLoadContext<'_>) -> jni::jint {
        jni::JNI_OK
    }

    fn on_attach(&self, context: AgentLoadContext<'_>) -> jni::jint {
        println!(
            "[AttachLogger] attached with options: {}",
            context.options_lossy().as_deref().unwrap_or("")
        );

        // You can obtain JVMTI on attach and enable events if needed.
        if let Ok(_jvmti) = context.vm().jvmti() {
            // Configure JVMTI here if desired.
        }

        jni::JNI_OK
    }
}

export_agent!(AttachLogger);
