//! Count loaded class bytes under a Minecraft-related package prefix.
//!
//! Default prefix: `net/minecraft/`. Override it with
//! `prefix=com/example/mod/`. Names vary across loaders, mappings, and releases.
//! This example uses only public `jvmti-bindings` APIs.

use jvmti_bindings::prelude::*;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct MinecraftClassActivity {
    prefix: OnceLock<String>,
    classes: AtomicU64,
    bytes: AtomicU64,
}

impl Agent for MinecraftClassActivity {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let options = context.options_str().ok().flatten().unwrap_or_default();
        let prefix = options
            .split(',')
            .find_map(|part| part.strip_prefix("prefix="))
            .unwrap_or("net/minecraft/");
        let _ = self.prefix.set(prefix.to_owned());

        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti.configure_class_file_load_hook_agent().is_err()
            || jvmti
                .enable_events_global(&[jvmti::JVMTI_EVENT_VM_DEATH])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn class_file_load_hook(
        &self,
        _context: CallbackContext<'_>,
        event: ClassFileLoadHookEvent<'_>,
    ) {
        let Some(name) = event.name_str().ok().flatten() else {
            return;
        };
        if name.starts_with(self.prefix.get().map(String::as_str).unwrap_or("")) {
            self.classes.fetch_add(1, Ordering::Relaxed);
            self.bytes
                .fetch_add(event.class_data().len() as u64, Ordering::Relaxed);
        }
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[minecraft-classes] prefix={} classes={} class_bytes={}",
            self.prefix.get().map(String::as_str).unwrap_or(""),
            self.classes.load(Ordering::Relaxed),
            self.bytes.load(Ordering::Relaxed)
        );
    }
}

export_agent!(MinecraftClassActivity);
