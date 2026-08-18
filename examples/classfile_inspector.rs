//! Parse selected class files in `ClassFileLoadHook` without transforming them.
//!
//! Pass `prefix=com/example/` as the agent option. Parsing allocates, so a
//! narrow prefix is strongly recommended for real applications.

use jvmti_bindings::classfile::ClassFile;
use jvmti_bindings::prelude::*;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct ClassfileInspector {
    prefix: OnceLock<String>,
    parsed: AtomicU64,
    rejected: AtomicU64,
    methods: AtomicU64,
}

impl Agent for ClassfileInspector {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let options = context.options_str().ok().flatten().unwrap_or_default();
        let prefix = options
            .split(',')
            .find_map(|part| part.strip_prefix("prefix="))
            .unwrap_or("com/example/");
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
        if !name.starts_with(self.prefix.get().map(String::as_str).unwrap_or("")) {
            return;
        }
        match ClassFile::parse(event.class_data()) {
            Ok(class) => {
                self.parsed.fetch_add(1, Ordering::Relaxed);
                self.methods
                    .fetch_add(class.methods.len() as u64, Ordering::Relaxed);
            }
            Err(_) => {
                self.rejected.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[classfile-inspector] parsed={} rejected={} methods={}",
            self.parsed.load(Ordering::Relaxed),
            self.rejected.load(Ordering::Relaxed),
            self.methods.load(Ordering::Relaxed)
        );
    }
}

export_agent!(ClassfileInspector);
