//! Class loading logger example.
//!
//! Logs class files as they load. Pass `filter=com/example` to restrict names.
//! Demonstrates:
//! - Using ClassFileLoadHook for bytecode observation
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
//! java -agentpath:./target/release/examples/libclass_logger.so=filter=com/example MyApp
//! ```

use jvmti_bindings::prelude::*;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct ClassLogger {
    filter: OnceLock<String>,
    classes_loaded: AtomicU64,
}

impl Agent for ClassLogger {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        println!("[ClassLogger] Starting class logger...");

        let options = context.options_str().ok().flatten().unwrap_or_default();
        let filter = options
            .split(',')
            .find_map(|part| part.strip_prefix("filter="))
            .unwrap_or("");
        let _ = self.filter.set(filter.to_owned());
        if !filter.is_empty() {
            println!("[ClassLogger] Filtering for classes matching: {filter}");
        }

        let jvmti_env = match context.vm().jvmti() {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[ClassLogger] Failed to get JVMTI env: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        // Retransform is requested so this template can be extended to rewrite
        // already-loaded classes; this example only observes load-hook bytes.
        if let Err(e) = jvmti_env.add_capabilities_with(|caps| {
            caps.set_can_generate_all_class_hook_events(true);
            caps.set_can_retransform_classes(true);
        }) {
            eprintln!("[ClassLogger] Failed to add capabilities: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti_env.set_default_agent_callbacks() {
            eprintln!("[ClassLogger] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti_env.enable_class_file_load_hook_events() {
            eprintln!("[ClassLogger] Failed to enable class hook: {:?}", e);
            return jni::JNI_ERR;
        }

        if let Err(e) = jvmti_env.enable_events_global(&[jvmti::JVMTI_EVENT_VM_DEATH]) {
            eprintln!("[ClassLogger] Failed to enable VM death: {:?}", e);
            return jni::JNI_ERR;
        }

        println!("[ClassLogger] Ready to log class loads");
        jni::JNI_OK
    }

    fn class_file_load_hook(
        &self,
        _context: CallbackContext<'_>,
        event: ClassFileLoadHookEvent<'_>,
    ) {
        let class_name = event.name_str().ok().flatten();
        let filter = self.filter.get().map(String::as_str).unwrap_or("");
        if !filter.is_empty()
            && !class_name
                .as_deref()
                .is_some_and(|name| name.starts_with(filter))
        {
            return;
        }

        self.classes_loaded.fetch_add(1, Ordering::Relaxed);
        println!(
            "[ClassLogger] Loaded: {} ({} bytes)",
            class_name.as_deref().unwrap_or("<unknown>"),
            event.class_data().len()
        );
        // Replacement uses ClassFileLoadHookEvent::set_transformed_class; this
        // example only observes.
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        let count = self.classes_loaded.load(Ordering::Relaxed);
        println!("[ClassLogger] === Summary ===");
        println!("[ClassLogger] Total classes loaded: {}", count);
    }
}

export_agent!(ClassLogger);
