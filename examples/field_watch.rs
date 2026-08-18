//! Install access and modification watchpoints for one named field.
//!
//! Options: `class=Lcom/example/State;,field=value`.

use jvmti_bindings::prelude::*;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

struct WatchConfig {
    class_signature: String,
    field_name: String,
}

#[derive(Default)]
struct FieldWatch {
    config: OnceLock<WatchConfig>,
    installed: AtomicU64,
    reads: AtomicU64,
    writes: AtomicU64,
}

fn option(options: &str, key: &str, default: &str) -> String {
    options
        .split(',')
        .find_map(|part| part.strip_prefix(key))
        .unwrap_or(default)
        .to_owned()
}

impl Agent for FieldWatch {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let options = context.options_str().ok().flatten().unwrap_or_default();
        let _ = self.config.set(WatchConfig {
            class_signature: option(&options, "class=", "Lcom/example/State;"),
            field_name: option(&options, "field=", "value"),
        });

        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|caps| {
                caps.set_can_generate_field_access_events(true);
                caps.set_can_generate_field_modification_events(true);
            })
            .is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_CLASS_PREPARE,
                    jvmti::JVMTI_EVENT_FIELD_ACCESS,
                    jvmti::JVMTI_EVENT_FIELD_MODIFICATION,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn class_prepare(&self, context: CallbackContext<'_>, event: ClassEvent) {
        let Some(config) = self.config.get() else {
            return;
        };
        let Ok((signature, _)) = (unsafe { context.jvmti().get_class_signature(event.class()) })
        else {
            return;
        };
        if signature != config.class_signature {
            return;
        }
        let Ok(fields) = (unsafe { context.jvmti().get_class_fields(event.class()) }) else {
            return;
        };
        for field in fields {
            let Ok((name, _, _)) =
                (unsafe { context.jvmti().get_field_name(event.class(), field) })
            else {
                continue;
            };
            if name == config.field_name
                && unsafe { context.jvmti().set_field_access_watch(event.class(), field) }.is_ok()
                && unsafe {
                    context
                        .jvmti()
                        .set_field_modification_watch(event.class(), field)
                }
                .is_ok()
            {
                self.installed.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn field_access(&self, _context: CallbackContext<'_>, _event: FieldAccessEvent) {
        self.reads.fetch_add(1, Ordering::Relaxed);
    }

    fn field_modification(&self, _context: CallbackContext<'_>, _event: FieldModificationEvent) {
        self.writes.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[field-watch] installed={} reads={} writes={}",
            self.installed.load(Ordering::Relaxed),
            self.reads.load(Ordering::Relaxed),
            self.writes.load(Ordering::Relaxed)
        );
    }
}

export_agent!(FieldWatch);
