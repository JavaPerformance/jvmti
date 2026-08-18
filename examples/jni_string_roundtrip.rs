//! Use callback-scoped JNI safely with an RAII local-reference frame.

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct JniStringRoundtrip;

impl Agent for JniStringRoundtrip {
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
        let Some(jni) = context.jni() else {
            eprintln!("[jni-string] VMInit unexpectedly had no JNIEnv");
            return;
        };
        let Ok(_frame) = jni.push_local_frame(8) else {
            eprintln!("[jni-string] PushLocalFrame failed");
            return;
        };
        let Some(java_string) = jni.new_string_utf("Rust -> Java -> Rust: \0 and unicode ✓")
        else {
            eprintln!("[jni-string] NewStringUTF failed");
            return;
        };
        let Some(roundtrip) = (unsafe { jni.get_string_utf(java_string) }) else {
            eprintln!("[jni-string] GetStringUTFChars failed");
            return;
        };
        eprintln!("[jni-string] {roundtrip:?}");
    }
}

export_agent!(JniStringRoundtrip);
