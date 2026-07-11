//! Compatibility smoke agent for the JVMTI event callback ABI.
//!
//! This intentionally exercises events on both sides of the reserved callback
//! slots. Run it through `scripts/prove-event-callback-matrix.sh` rather than as
//! an application-facing example.

use std::sync::atomic::{AtomicU64, Ordering};

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct EventAbiSmokeAgent {
    method_entries: AtomicU64,
    gc_starts: AtomicU64,
    gc_finishes: AtomicU64,
}

impl Agent for EventAbiSmokeAgent {
    fn on_load(&self, vm: *mut jni::JavaVM, _options: &str) -> jni::jint {
        let jvmti = match Jvmti::new(vm) {
            Ok(env) => env,
            Err(error) => {
                eprintln!("[event-abi] cannot acquire JVMTI: {error:?}");
                return jni::JNI_ERR;
            }
        };

        if let Err(error) = jvmti.add_capabilities_with(|caps| {
            caps.set_can_generate_method_entry_events(true);
            caps.set_can_generate_garbage_collection_events(true);
        }) {
            eprintln!("[event-abi] cannot add capabilities: {error:?}");
            return jni::JNI_ERR;
        }

        if let Err(error) = jvmti.set_default_agent_callbacks() {
            eprintln!("[event-abi] cannot set callbacks: {error:?}");
            return jni::JNI_ERR;
        }

        if let Err(error) = jvmti.enable_events_global(&[
            jvmti::JVMTI_EVENT_METHOD_ENTRY,
            jvmti::JVMTI_EVENT_GARBAGE_COLLECTION_START,
            jvmti::JVMTI_EVENT_GARBAGE_COLLECTION_FINISH,
            jvmti::JVMTI_EVENT_VM_DEATH,
        ]) {
            eprintln!("[event-abi] cannot enable callbacks: {error:?}");
            return jni::JNI_ERR;
        }

        jni::JNI_OK
    }

    fn method_entry(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID) {
        self.method_entries.fetch_add(1, Ordering::Relaxed);
    }

    fn garbage_collection_start(&self) {
        self.gc_starts.fetch_add(1, Ordering::Relaxed);
    }

    fn garbage_collection_finish(&self) {
        self.gc_finishes.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _jni: *mut jni::JNIEnv) {
        eprintln!(
            "[event-abi] PASS method_entries={} gc_starts={} gc_finishes={}",
            self.method_entries.load(Ordering::Relaxed),
            self.gc_starts.load(Ordering::Relaxed),
            self.gc_finishes.load(Ordering::Relaxed),
        );
    }
}

export_agent!(EventAbiSmokeAgent);
