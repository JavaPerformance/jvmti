//! Heap sampling example using SampledObjectAlloc.
//!
//! Build:
//!   cargo build --release --example heap_sampler
//! Run:
//!   java -agentpath:./target/release/examples/libheap_sampler.so MyApp

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct HeapSampler {
    sampled_allocs: AtomicU64,
}

impl Agent for HeapSampler {
    fn on_load(&self, vm: *mut jni::JavaVM, _options: &str) -> jni::jint {
        let jvmti = match Jvmti::new(vm) {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[heap] Failed to get JVMTI: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        if let Err(e) = jvmti.add_capabilities_with(|caps| {
            caps.set_can_generate_sampled_object_alloc_events(true);
        }) {
            eprintln!("[heap] Failed to add capabilities: {:?}", e);
            return jni::JNI_ERR;
        }

        let callbacks = get_default_callbacks();
        if let Err(e) = jvmti.set_event_callbacks(callbacks) {
            eprintln!("[heap] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        let _ = jvmti.set_heap_sampling_interval(1024 * 1024);

        if let Err(e) = jvmti.enable_events_global(&[jvmti::JVMTI_EVENT_SAMPLED_OBJECT_ALLOC]) {
            eprintln!("[heap] Failed to enable events: {:?}", e);
            return jni::JNI_ERR;
        }

        jni::JNI_OK
    }

    fn sampled_object_alloc(
        &self,
        _jni: *mut jni::JNIEnv,
        _thread: jni::jthread,
        _object: jni::jobject,
        _klass: jni::jclass,
        _size: jni::jlong,
    ) {
        self.sampled_allocs.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _jni: *mut jni::JNIEnv) {
        let count = self.sampled_allocs.load(Ordering::Relaxed);
        eprintln!("[heap] Sampled allocations: {}", count);
    }
}

export_agent!(HeapSampler);
