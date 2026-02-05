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
    fn on_load(&self, vm: *mut jni::JavaVM, _options: &str) -> jni::jint {
        let jvmti = match Jvmti::new(vm) {
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

        let callbacks = get_default_callbacks();
        if let Err(e) = jvmti.set_event_callbacks(callbacks) {
            eprintln!("[profiler] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti.enable_events_global(&[jvmti::JVMTI_EVENT_METHOD_ENTRY]) {
            eprintln!("[profiler] Failed to enable events: {:?}", e);
            return jni::JNI_ERR;
        }

        jni::JNI_OK
    }

    fn method_entry(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID) {
        self.method_entries.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _jni: *mut jni::JNIEnv) {
        let count = self.method_entries.load(Ordering::Relaxed);
        eprintln!("[profiler] Total method entries: {}", count);
    }
}

export_agent!(MethodProfiler);
