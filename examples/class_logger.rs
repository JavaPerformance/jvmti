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

use ::jvmti::export_agent;
use ::jvmti::sys::jni;
use ::jvmti::sys::jvmti;
use ::jvmti::env::Jvmti;
use ::jvmti::Agent;
use ::jvmti::get_default_callbacks;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct ClassLogger {
    classes_loaded: AtomicU64,
}

impl Agent for ClassLogger {
    fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
        println!("[ClassLogger] Starting class logger...");

        // Parse filter from options (e.g., "filter=com/example")
        let filter: Option<&str> = options
            .split(',')
            .find(|s| s.starts_with("filter="))
            .map(|s| &s[7..]);

        if let Some(f) = filter {
            println!("[ClassLogger] Filtering for classes matching: {}", f);
        }

        let jvmti_env = match Jvmti::new(vm) {
            Ok(env) => env,
            Err(e) => {
                eprintln!("[ClassLogger] Failed to get JVMTI env: {:?}", e);
                return jni::JNI_ERR;
            }
        };

        // Request capabilities
        let mut caps = jvmti::jvmtiCapabilities::default();
        caps.set_can_generate_all_class_hook_events(true);
        caps.set_can_retransform_classes(true);

        if let Err(e) = jvmti_env.add_capabilities(&caps) {
            eprintln!("[ClassLogger] Failed to add capabilities: {:?}", e);
            return jni::JNI_ERR;
        }

        // Set up callbacks
        let callbacks = get_default_callbacks();
        if let Err(e) = jvmti_env.set_event_callbacks(callbacks) {
            eprintln!("[ClassLogger] Failed to set callbacks: {:?}", e);
            return jni::JNI_ERR;
        }

        // Enable class file load hook
        if let Err(e) = jvmti_env.set_event_notification_mode(
            true, // enable
            jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK,
            std::ptr::null_mut(),
        ) {
            eprintln!("[ClassLogger] Failed to enable class hook: {:?}", e);
            return jni::JNI_ERR;
        }

        // Enable VM death for summary
        let _ = jvmti_env.set_event_notification_mode(
            true, // enable
            jvmti::JVMTI_EVENT_VM_DEATH,
            std::ptr::null_mut(),
        );

        println!("[ClassLogger] Ready to log class loads");
        jni::JNI_OK
    }

    fn class_file_load_hook(
        &self,
        _jni: *mut jni::JNIEnv,
        _class_being_redefined: jni::jclass,
        _loader: jni::jobject,
        name: *const std::os::raw::c_char,
        _protection_domain: jni::jobject,
        class_data_len: jni::jint,
        _class_data: *const u8,
        _new_class_data_len: *mut jni::jint,
        _new_class_data: *mut *mut u8,
    ) {
        self.classes_loaded.fetch_add(1, Ordering::Relaxed);

        // Get class name (may be null for some system classes)
        let class_name = if name.is_null() {
            "<unknown>".to_string()
        } else {
            unsafe { std::ffi::CStr::from_ptr(name) }
                .to_str()
                .unwrap_or("<invalid>")
                .to_string()
        };

        // Log the class load
        println!(
            "[ClassLogger] Loaded: {} ({} bytes)",
            class_name, class_data_len
        );

        // Note: To modify the class, you would:
        // 1. Allocate memory with jvmti.allocate()
        // 2. Copy/modify the bytecode
        // 3. Set *new_class_data_len and *new_class_data
        //
        // For this example, we just observe (don't modify).
    }

    fn vm_death(&self, _jni: *mut jni::JNIEnv) {
        let count = self.classes_loaded.load(Ordering::Relaxed);
        println!("[ClassLogger] === Summary ===");
        println!("[ClassLogger] Total classes loaded: {}", count);
    }
}

export_agent!(ClassLogger);
