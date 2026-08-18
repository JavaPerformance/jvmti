//! Take a one-time inventory of live platform threads at VM initialization.

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct ThreadInventory;

impl Agent for ThreadInventory {
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
        let Ok(threads) = context.jvmti().get_all_threads() else {
            eprintln!("[thread-inventory] GetAllThreads failed");
            return;
        };
        eprintln!("[thread-inventory] live={}", threads.len());

        if let Some(jni) = context.jni() {
            for thread in threads {
                unsafe { jni.delete_local_ref(thread) };
            }
        }
    }
}

export_agent!(ThreadInventory);
