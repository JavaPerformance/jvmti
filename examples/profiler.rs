//! Minimal method-entry profiler example.
//!
//! Build:
//!   cargo build --release --example profiler
//! Run:
//!   java -agentpath:./target/release/examples/libprofiler.so MyApp

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct MethodProfiler {
    method_entries: AtomicU64,
}

impl Agent for MethodProfiler {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let jvmti = match context.vm().jvmti() {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[profiler] Failed to get JVMTI: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        if let Err(e) = jvmti.add_capabilities_with(|caps| {
            caps.set_can_generate_method_entry_events(true);
        }) {
            eprintln!("[profiler] Failed to add capabilities: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti.set_default_agent_callbacks() {
            eprintln!("[profiler] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti.enable_events_global(&[jvmti::JVMTI_EVENT_METHOD_ENTRY]) {
            eprintln!("[profiler] Failed to enable events: {:?}", e);
            return jni::JNI_ERR;
        }

        jni::JNI_OK
    }

    fn method_entry(&self, _context: CallbackContext<'_>, _event: MethodEvent) {
        self.method_entries.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        let count = self.method_entries.load(Ordering::Relaxed);
        eprintln!("[profiler] Total method entries: {}", count);
    }
}

export_agent!(MethodProfiler);
