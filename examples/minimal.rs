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
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        println!(
            "[MinimalAgent] Loaded with options: '{}'",
            context.options_lossy().as_deref().unwrap_or("")
        );
        jni::JNI_OK
    }

    fn on_unload(&self, _context: AgentUnloadContext<'_>) {
        println!("[MinimalAgent] Unloading...");
    }

    fn vm_init(&self, _context: CallbackContext<'_>, _event: ThreadEvent) {
        println!("[MinimalAgent] VM initialized!");
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        println!("[MinimalAgent] VM shutting down...");
    }
}

export_agent!(MinimalAgent);
