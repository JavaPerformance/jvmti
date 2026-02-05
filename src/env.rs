//! High-level environment wrappers for JVMTI and JNI.
//!
//! This module provides ergonomic Rust wrappers around the raw JVMTI and JNI
//! environment pointers. These wrappers handle memory management, string conversion,
//! and provide Rust-friendly return types.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use jvmti_bindings::prelude::*;
//!
//! #[derive(Default)]
//! struct MyAgent;
//!
//! impl Agent for MyAgent {
//!     fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
//!         // Get JVMTI environment
//!         let jvmti = Jvmti::new(vm).expect("Failed to get JVMTI env");
//!
//!         // Request capabilities
//!         let mut caps = jvmti::jvmtiCapabilities::default();
//!         caps.set_can_generate_all_class_hook_events(true);
//!         jvmti.add_capabilities(&caps).expect("Failed to add capabilities");
//!
//!         // Set up event callbacks
//!         let callbacks = get_default_callbacks();
//!         jvmti.set_event_callbacks(callbacks).expect("Failed to set callbacks");
//!
//!         jni::JNI_OK
//!     }
//!
//!     fn vm_init(&self, jni_ptr: *mut jni::JNIEnv, _thread: jni::jthread) {
//!         // Get JNI environment
//!         let jni = unsafe { JniEnv::from_raw(jni_ptr) };
//!
//!         // Find a class
//!         if let Some(cls) = jni.find_class("java/lang/System") {
//!             println!("Found System class!");
//!         }
//!
//!         // Create a string
//!         if let Some(s) = jni.new_string_utf("Hello from Rust!") {
//!             println!("Created Java string");
//!         }
//!     }
//! }
//!
//! export_agent!(MyAgent);
//! ```
//!
//! # JVMTI Environment
//!
//! The [`Jvmti`] struct wraps the JVMTI environment and provides methods for:
//!
//! - **Capabilities**: Request and query agent capabilities
//! - **Events**: Set up event callbacks and enable/disable events
//! - **Threads**: Enumerate, suspend, resume threads
//! - **Classes**: Get loaded classes, class info, bytecode
//! - **Methods**: Get method info, line numbers, local variables
//! - **Heap**: Object tagging, heap iteration, garbage collection
//! - **Stack**: Get stack traces, frame info, local variables
//! - **Monitors**: Create and manage raw monitors
//! - **Breakpoints**: Set and clear breakpoints
//! - **Class Transformation**: Redefine and retransform classes
//!
//! # JNI Environment
//!
//! The [`JniEnv`] struct wraps the JNI environment and provides methods for:
//!
//! - **Classes**: Find classes, check inheritance
//! - **Objects**: Create objects, check types
//! - **Strings**: Convert between Java strings and Rust strings
//! - **Arrays**: Create and access Java arrays
//! - **Methods**: Call instance and static methods
//! - **Fields**: Get and set instance and static fields
//! - **Exceptions**: Check, throw, and clear exceptions
//! - **References**: Manage local, global, and weak references
//!
//! # Reference Guards
//!
//! The module also provides RAII guards for automatic reference cleanup:
//!
//! - [`LocalRef`]: Automatically deletes a local reference when dropped
//! - [`GlobalRef`]: Automatically deletes a global reference when dropped
//!
//! ```rust,ignore
//! use jvmti_bindings::prelude::*;
//!
//! fn do_something(jni: &JniEnv) {
//!     // LocalRef automatically cleans up when it goes out of scope
//!     let class = LocalRef::new(jni, jni.find_class("java/lang/String").unwrap());
//!
//!     // Use class.get() to access the underlying jclass
//!     let method = jni.get_method_id(class.get(), "length", "()I");
//!
//!     // class is automatically deleted here
//! }
//! ```

// Re-export the JVMTI wrapper
mod jvmti_impl {
    pub use crate::jvmti_wrapper::{
        ExtensionEventInfo, ExtensionFunctionInfo, ExtensionParamInfo, Jvmti, LocalVariableEntry,
        MonitorUsage, StackInfo, ThreadGroupInfo, ThreadInfo,
    };
}

// Re-export the JNI wrapper
mod jni_impl {
    pub use crate::jni_wrapper::{JniEnv, LocalRef, GlobalRef};
}

pub use jvmti_impl::{
    ExtensionEventInfo, ExtensionFunctionInfo, ExtensionParamInfo, Jvmti, LocalVariableEntry,
    MonitorUsage, StackInfo, ThreadGroupInfo, ThreadInfo,
};
pub use jni_impl::{JniEnv, LocalRef, GlobalRef};
