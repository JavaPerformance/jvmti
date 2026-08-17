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
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let jvmti = match context.vm().jvmti() {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[heap] Failed to get JVMTI: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        if let Err(e) = jvmti.add_heap_sampling_capabilities() {
            eprintln!("[heap] Failed to add capabilities: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti.set_default_agent_callbacks() {
            eprintln!("[heap] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        let _ = jvmti.set_heap_sampling_interval(1024 * 1024);

        if let Err(e) = jvmti.enable_heap_sampling_events() {
            eprintln!("[heap] Failed to enable events: {:?}", e);
            return jni::JNI_ERR;
        }

        jni::JNI_OK
    }

    fn sampled_object_alloc(&self, _context: CallbackContext<'_>, _event: ObjectAllocationEvent) {
        self.sampled_allocs.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        let count = self.sampled_allocs.load(Ordering::Relaxed);
        eprintln!("[heap] Sampled allocations: {}", count);
    }
}

export_agent!(HeapSampler);
