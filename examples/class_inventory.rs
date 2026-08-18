//! Take a one-time inventory of classes loaded when the VM becomes live.

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct ClassInventory;

impl Agent for ClassInventory {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti.set_default_agent_callbacks().is_err()
            || jvmti.enable_vm_lifecycle_events().is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn vm_init(&self, context: CallbackContext<'_>, _event: ThreadEvent) {
        let Ok(classes) = context.jvmti().get_loaded_classes() else {
            eprintln!("[class-inventory] GetLoadedClasses failed");
            return;
        };
        eprintln!("[class-inventory] loaded={}", classes.len());

        // The returned handles are JNI local references owned by this callback.
        if let Some(jni) = context.jni() {
            for class in classes {
                unsafe { jni.delete_local_ref(class) };
            }
        }
    }
}

export_agent!(ClassInventory);
