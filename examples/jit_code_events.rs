//! Count JIT-compiled method and dynamically generated code events.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct JitCodeEvents {
    loads: AtomicU64,
    unloads: AtomicU64,
    generated_blocks: AtomicU64,
    generated_bytes: AtomicU64,
}

impl Agent for JitCodeEvents {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|caps| {
                caps.set_can_generate_compiled_method_load_events(true);
            })
            .is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_COMPILED_METHOD_LOAD,
                    jvmti::JVMTI_EVENT_COMPILED_METHOD_UNLOAD,
                    jvmti::JVMTI_EVENT_DYNAMIC_CODE_GENERATED,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn compiled_method_load(
        &self,
        _context: CallbackContext<'_>,
        _event: CompiledMethodLoadEvent<'_>,
    ) {
        self.loads.fetch_add(1, Ordering::Relaxed);
    }

    fn compiled_method_unload(
        &self,
        _context: CallbackContext<'_>,
        _event: CompiledMethodUnloadEvent,
    ) {
        self.unloads.fetch_add(1, Ordering::Relaxed);
    }

    fn dynamic_code_generated(
        &self,
        _context: CallbackContext<'_>,
        event: DynamicCodeGeneratedEvent<'_>,
    ) {
        self.generated_blocks.fetch_add(1, Ordering::Relaxed);
        self.generated_bytes
            .fetch_add(event.length().max(0) as u64, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[jit] loads={} unloads={} generated_blocks={} generated_bytes={}",
            self.loads.load(Ordering::Relaxed),
            self.unloads.load(Ordering::Relaxed),
            self.generated_blocks.load(Ordering::Relaxed),
            self.generated_bytes.load(Ordering::Relaxed)
        );
    }
}

export_agent!(JitCodeEvents);
