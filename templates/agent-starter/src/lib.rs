use jvmti_bindings::prelude::*;

#[derive(Default)]
struct MyAgent;

impl Agent for MyAgent {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        eprintln!(
            "[agent] on_load: {}",
            context.options_lossy().as_deref().unwrap_or("")
        );

        let jvmti = match context.vm().jvmti() {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[agent] Failed to get JVMTI: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        if let Err(e) = jvmti.add_capabilities_with(|caps| {
            caps.set_can_generate_all_class_hook_events(true);
        }) {
            eprintln!("[agent] Failed to add capabilities: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti.set_default_agent_callbacks() {
            eprintln!("[agent] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti.enable_class_file_load_hook_events() {
            eprintln!("[agent] Failed to enable events: {:?}", e);
            return jni::JNI_ERR;
        }

        jni::JNI_OK
    }

    fn class_file_load_hook(
        &self,
        _context: CallbackContext<'_>,
        event: ClassFileLoadHookEvent<'_>,
    ) {
        let class_name = event
            .name()
            .and_then(|name| name.to_str().ok())
            .unwrap_or("<unknown>");

        eprintln!(
            "[agent] Loaded: {} ({} bytes)",
            class_name,
            event.class_data().len()
        );
    }
}

export_agent!(MyAgent);
