//! Class loading logger example.
//!
//! This agent logs all classes as they are loaded by the JVM.
//! Demonstrates:
//! - Using ClassFileLoadHook for bytecode interception
//! - Requesting retransform capabilities
//! - Working with raw class data
//!
//! # Building
//!
//! ```bash
//! cargo build --release --example class_logger
//! ```
//!
//! # Running
//!
//! ```bash
//! java -agentpath:./target/release/examples/libclass_logger.so MyApp
//! ```

use jvmti_bindings::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct ClassLogger {
    classes_loaded: AtomicU64,
}

impl Agent for ClassLogger {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        println!("[ClassLogger] Starting class logger...");

        // Parse filter from options (e.g., "filter=com/example")
        let options = context.options_str().ok().flatten().unwrap_or_default();
        let filter: Option<&str> = options
            .split(',')
            .find(|s| s.starts_with("filter="))
            .map(|s| &s[7..]);

        if let Some(f) = filter {
            println!("[ClassLogger] Filtering for classes matching: {}", f);
        }

        let jvmti_env = match context.vm().jvmti() {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[ClassLogger] Failed to get JVMTI env: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        // Request capabilities. This example also asks for retransform support.
        if let Err(e) = jvmti_env.add_capabilities_with(|caps| {
            caps.set_can_generate_all_class_hook_events(true);
            caps.set_can_retransform_classes(true);
        }) {
            eprintln!("[ClassLogger] Failed to add capabilities: {:?}", e);
            return jni::JNI_ERR;
        }

        // Set up callbacks
        if let Err(e) = jvmti_env.set_default_agent_callbacks() {
            eprintln!("[ClassLogger] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        // Enable class file load hook
        if let Err(e) = jvmti_env.enable_class_file_load_hook_events() {
            eprintln!("[ClassLogger] Failed to enable class hook: {:?}", e);
            return jni::JNI_ERR;
        }

        // Enable VM death for summary
        let _ = jvmti_env.enable_events_global(&[jvmti::JVMTI_EVENT_VM_DEATH]);

        println!("[ClassLogger] Ready to log class loads");
        jni::JNI_OK
    }

    fn class_file_load_hook(
        &self,
        _context: CallbackContext<'_>,
        event: ClassFileLoadHookEvent<'_>,
    ) {
        self.classes_loaded.fetch_add(1, Ordering::Relaxed);

        // Get class name (may be null for some system classes)
        let class_name = event.name_str().ok().flatten();

        // Log the class load
        println!(
            "[ClassLogger] Loaded: {} ({} bytes)",
            class_name.as_deref().unwrap_or("<unknown>"),
            event.class_data().len()
        );

        // Note: To modify the class, you would:
        // 1. Allocate memory with jvmti.allocate()
        // 2. Copy/modify the bytecode
        // 3. Set *new_class_data_len and *new_class_data
        //
        // For this example, we just observe (don't modify).
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        let count = self.classes_loaded.load(Ordering::Relaxed);
        println!("[ClassLogger] === Summary ===");
        println!("[ClassLogger] Total classes loaded: {}", count);
    }
}

export_agent!(ClassLogger);
