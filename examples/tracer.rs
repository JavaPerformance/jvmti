//! Minimal class-load tracer example.
//!
//! Build:
//!   cargo build --release --example tracer
//! Run:
//!   java -agentpath:./target/release/examples/libtracer.so MyApp

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct ClassTracer;

impl Agent for ClassTracer {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let jvmti = match context.vm().jvmti() {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[tracer] Failed to get JVMTI: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        if let Err(e) = jvmti.configure_class_file_load_hook_agent() {
            eprintln!("[tracer] Failed to configure class hook: {:?}", e);
            return jni::JNI_ERR;
        }

        jni::JNI_OK
    }

    fn class_file_load_hook(
        &self,
        _context: CallbackContext<'_>,
        event: ClassFileLoadHookEvent<'_>,
    ) {
        let class_name = event.name_str().ok().flatten();

        eprintln!(
            "[tracer] Loaded: {} ({} bytes)",
            class_name.as_deref().unwrap_or("<unknown>"),
            event.class_data().len()
        );
    }
}

export_agent!(ClassTracer);
