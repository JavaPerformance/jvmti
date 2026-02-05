//! Minimal JVMTI agent example.
//!
//! This is the simplest possible agent - it just prints messages on load/unload.
//!
//! # Building
//!
//! ```bash
//! cargo build --release --example minimal
//! ```
//!
//! # Running
//!
//! ```bash
//! java -agentpath:./target/release/examples/libminimal.so=hello MyApp
//! ```

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct MinimalAgent;

impl Agent for MinimalAgent {
    fn on_load(&self, _vm: *mut jni::JavaVM, options: &str) -> jni::jint {
        println!("[MinimalAgent] Loaded with options: '{}'", options);
        jni::JNI_OK
    }

    fn on_unload(&self) {
        println!("[MinimalAgent] Unloading...");
    }

    fn vm_init(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread) {
        println!("[MinimalAgent] VM initialized!");
    }

    fn vm_death(&self, _jni: *mut jni::JNIEnv) {
        println!("[MinimalAgent] VM shutting down...");
    }
}

export_agent!(MinimalAgent);
