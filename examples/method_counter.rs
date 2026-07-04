//! Method counting profiler example.
//!
//! This agent counts method entries and prints a summary on VM death.
//! Demonstrates:
//! - Requesting JVMTI capabilities
//! - Enabling event notifications
//! - Thread-safe state using atomics
//! - Using the JVMTI wrapper API
//!
//! # Building
//!
//! ```bash
//! cargo build --release --example method_counter
//! ```
//!
//! # Running
//!
//! ```bash
//! java -agentpath:./target/release/examples/libmethod_counter.so MyApp
//! ```
//!
//! # Note
//!
//! Method entry events have significant overhead. For production profiling,
//! consider using sampling-based approaches or bytecode instrumentation.

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct MethodCounter {
    method_entries: AtomicU64,
    method_exits: AtomicU64,
}

impl Agent for MethodCounter {
    fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
        println!("[MethodCounter] Loading agent...");

        // Parse options (example: "verbose" flag)
        let verbose = options.contains("verbose");
        if verbose {
            println!("[MethodCounter] Verbose mode enabled");
        }

        // Get JVMTI environment
        let jvmti_env = match Jvmti::new(vm) {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[MethodCounter] Failed to get JVMTI env: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        // Request capabilities, wire callbacks, and enable method entry/exit.
        // Note: Enabling these for all threads has significant overhead.
        if let Err(e) = jvmti_env.configure_method_trace_agent() {
            eprintln!(
                "[MethodCounter] Failed to configure method tracing: {:?}",
                e
            );
            return jni::JNI_ERR;
        }

        // Also enable VM death so we can print summary
        let _ = jvmti_env.enable_event(jvmti::JVMTI_EVENT_VM_DEATH, std::ptr::null_mut());

        println!("[MethodCounter] Agent ready, counting methods...");
        jni::JNI_OK
    }

    fn method_entry(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID) {
        self.method_entries.fetch_add(1, Ordering::Relaxed);
    }

    fn method_exit(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID) {
        self.method_exits.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _jni: *mut jni::JNIEnv) {
        let entries = self.method_entries.load(Ordering::Relaxed);
        let exits = self.method_exits.load(Ordering::Relaxed);
        println!("[MethodCounter] === Summary ===");
        println!("[MethodCounter] Method entries: {}", entries);
        println!("[MethodCounter] Method exits:   {}", exits);
    }
}

export_agent!(MethodCounter);
