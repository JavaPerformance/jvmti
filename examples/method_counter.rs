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

        // Request capabilities for method entry/exit events
        let mut caps = jvmti::jvmtiCapabilities::default();
        caps.set_can_generate_method_entry_events(true);
        caps.set_can_generate_method_exit_events(true);

        if let Err(e) = jvmti_env.add_capabilities(&caps) {
            eprintln!("[MethodCounter] Failed to add capabilities: {:?}", e);
            return jni::JNI_ERR;
        }

        // Set up event callbacks
        let callbacks = get_default_callbacks();
        if let Err(e) = jvmti_env.set_event_callbacks(callbacks) {
            eprintln!("[MethodCounter] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        // Enable method entry/exit events
        // Note: Enabling these for all threads has significant overhead!
        if let Err(e) = jvmti_env.set_event_notification_mode(
            true, // enable
            jvmti::JVMTI_EVENT_METHOD_ENTRY,
            std::ptr::null_mut(),
        ) {
            eprintln!("[MethodCounter] Failed to enable method entry: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti_env.set_event_notification_mode(
            true, // enable
            jvmti::JVMTI_EVENT_METHOD_EXIT,
            std::ptr::null_mut(),
        ) {
            eprintln!("[MethodCounter] Failed to enable method exit: {:?}", e);
            return jni::JNI_ERR;
        }

        // Also enable VM death so we can print summary
        let _ = jvmti_env.set_event_notification_mode(
            true, // enable
            jvmti::JVMTI_EVENT_VM_DEATH,
            std::ptr::null_mut(),
        );

        println!("[MethodCounter] Agent ready, counting methods...");
        jni::JNI_OK
    }

    fn method_entry(
        &self,
        _jni: *mut jni::JNIEnv,
        _thread: jni::jthread,
        _method: jni::jmethodID,
    ) {
        self.method_entries.fetch_add(1, Ordering::Relaxed);
    }

    fn method_exit(
        &self,
        _jni: *mut jni::JNIEnv,
        _thread: jni::jthread,
        _method: jni::jmethodID,
    ) {
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
