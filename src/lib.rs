//! # jvmti
//!
//! Complete JNI and JVMTI bindings for Rust with **zero dependencies**.
//!
//! This crate provides everything you need to build JVM agents in Rust:
//! - Low-level FFI bindings to JNI and JVMTI
//! - High-level wrappers with ergonomic Rust APIs
//! - The [`Agent`] trait and [`export_agent!`] macro for easy agent creation
//!
//! ## Features
//!
//! - **Complete Coverage**: All 236 JNI functions, all 156 JVMTI functions
//! - **Zero Dependencies**: No external crates required
//! - **Ergonomic API**: High-level wrappers handle strings, arrays, references
//! - **Type-Safe**: Proper Rust types, `Result` returns, RAII guards
//! - **JDK 8-27 Compatible**: Verified against JDK 27 headers
//!
//! ## Quick Start
//!
//! Create a minimal agent in 4 steps:
//!
//! **1. Create a new library crate:**
//! ```bash
//! cargo new --lib my_agent
//! ```
//!
//! **2. Configure Cargo.toml:**
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! jvmti = "0.1"
//! ```
//!
//! **3. Implement your agent (src/lib.rs):**
//! ```rust,ignore
//! use jvmti::{Agent, export_agent};
//! use jvmti::sys::jni;
//!
//! #[derive(Default)]
//! struct MyAgent;
//!
//! impl Agent for MyAgent {
//!     fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
//!         println!("[MyAgent] Loaded with options: {}", options);
//!         jni::JNI_OK
//!     }
//!
//!     fn vm_init(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread) {
//!         println!("[MyAgent] VM initialized!");
//!     }
//!
//!     fn vm_death(&self, _jni: *mut jni::JNIEnv) {
//!         println!("[MyAgent] VM shutting down");
//!     }
//! }
//!
//! export_agent!(MyAgent);
//! ```
//!
//! **4. Build and run:**
//! ```bash
//! cargo build --release
//! java -agentpath:./target/release/libmy_agent.so=myoptions MyApp
//! ```
//!
//! ## Architecture
//!
//! The crate is organized in layers:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    Your Agent Code                       │
//! │         impl Agent for MyAgent { ... }                   │
//! ├─────────────────────────────────────────────────────────┤
//! │                   Agent Trait + Macros                   │
//! │      Agent, export_agent!, get_default_callbacks()       │
//! ├─────────────────────────────────────────────────────────┤
//! │              High-Level Wrappers (env module)            │
//! │   env::Jvmti - JVMTI operations with Result returns      │
//! │   env::JniEnv - JNI operations with string helpers       │
//! │   env::LocalRef, GlobalRef - RAII reference guards       │
//! ├─────────────────────────────────────────────────────────┤
//! │              Raw FFI Bindings (sys module)               │
//! │   sys::jni - JNI types, vtable (236 functions)           │
//! │   sys::jvmti - JVMTI types, vtable (156 functions)       │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`sys::jni`] | Raw JNI types and vtable (for FFI) |
//! | [`sys::jvmti`] | Raw JVMTI types, vtable, capabilities, events |
//! | [`env`] | **High-level wrappers** - start here for ergonomic APIs |
//! | [`env::Jvmti`] | JVMTI environment wrapper (153 methods) |
//! | [`env::JniEnv`] | JNI environment wrapper (60+ methods) |
//!
//! ## Enabling JVMTI Events
//!
//! To receive JVMTI events, you must:
//! 1. Request the required capabilities
//! 2. Set up event callbacks
//! 3. Enable the specific events
//!
//! ```rust,ignore
//! use jvmti::{Agent, export_agent, get_default_callbacks};
//! use jvmti::env::Jvmti;
//! use jvmti::sys::{jni, jvmti};
//!
//! #[derive(Default)]
//! struct ClassMonitor;
//!
//! impl Agent for ClassMonitor {
//!     fn on_load(&self, vm: *mut jni::JavaVM, _options: &str) -> jni::jint {
//!         let jvmti_env = Jvmti::new(vm).expect("Failed to get JVMTI");
//!
//!         // 1. Request capabilities
//!         let mut caps = jvmti::jvmtiCapabilities::default();
//!         caps.set_can_generate_all_class_hook_events(true);
//!         jvmti_env.add_capabilities(&caps).expect("capabilities");
//!
//!         // 2. Set up callbacks (wires all events to your Agent impl)
//!         let callbacks = get_default_callbacks();
//!         jvmti_env.set_event_callbacks(callbacks).expect("callbacks");
//!
//!         // 3. Enable specific events
//!         jvmti_env.set_event_notification_mode(
//!             true,  // enable
//!             jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK,
//!             std::ptr::null_mut()  // all threads
//!         ).expect("enable event");
//!
//!         jni::JNI_OK
//!     }
//!
//!     fn class_file_load_hook(&self, _jni: *mut jni::JNIEnv, /* ... */) {
//!         // Called for every class load!
//!     }
//! }
//!
//! export_agent!(ClassMonitor);
//! ```
//!
//! ## Working with JNI
//!
//! Use [`env::JniEnv`] for ergonomic JNI operations:
//!
//! ```rust,ignore
//! use jvmti::env::JniEnv;
//! use jvmti::sys::jni;
//!
//! fn vm_init(&self, jni_ptr: *mut jni::JNIEnv, _thread: jni::jthread) {
//!     let jni = unsafe { JniEnv::from_raw(jni_ptr) };
//!
//!     // Find a class
//!     let system_class = jni.find_class("java/lang/System").unwrap();
//!
//!     // Get a static field
//!     let out_field = jni.get_static_field_id(system_class, "out", "Ljava/io/PrintStream;").unwrap();
//!     let out = jni.get_static_object_field(system_class, out_field);
//!
//!     // Create a Java string
//!     let message = jni.new_string_utf("Hello from Rust!").unwrap();
//!
//!     // Call a method
//!     let print_class = jni.find_class("java/io/PrintStream").unwrap();
//!     let println_method = jni.get_method_id(print_class, "println", "(Ljava/lang/String;)V").unwrap();
//!     jni.call_void_method(out, println_method, &[jni::jvalue { l: message }]);
//!
//!     // Check for exceptions
//!     if jni.exception_check() {
//!         jni.exception_describe();
//!         jni.exception_clear();
//!     }
//! }
//! ```
//!
//! ## Version Compatibility
//!
//! | JDK Version | JNI Functions | JVMTI Functions | Notes |
//! |-------------|---------------|-----------------|-------|
//! | 8           | 232           | 153             | Baseline |
//! | 9           | 233           | 156             | +GetModule, +Module functions |
//! | 11          | 233           | 156             | +SetHeapSamplingInterval |
//! | 21          | 234           | 156             | +IsVirtualThread, +Virtual thread support |
//! | 24/25       | 235           | 156             | +GetStringUTFLengthAsLong |
//! | 27          | 235           | 156             | +ClearAllFramePops (slot 67) |

pub mod sys;
pub mod env;

// Implementation modules (use `env` module for the public API)
#[doc(hidden)]
pub mod jvmti_wrapper;
#[doc(hidden)]
pub mod jni_wrapper;

use std::sync::OnceLock;
pub use crate::sys::jni as jni;
use crate::sys::jvmti as jvmti;

/// The core trait for implementing a JVMTI agent.
///
/// Implement this trait and use [`export_agent!`] to create a loadable agent library.
/// All event methods have default no-op implementations, so you only need to override
/// the ones you care about.
///
/// # Thread Safety
///
/// Your agent must be `Sync + Send` because JVMTI events can fire from any thread.
/// Use appropriate synchronization (e.g., `Mutex`, `RwLock`, atomics) for shared state.
///
/// # Example
///
/// ```rust,ignore
/// use jvmti::{Agent, export_agent, sys::jni};
///
/// #[derive(Default)]
/// struct MyProfiler {
///     method_count: std::sync::atomic::AtomicU64,
/// }
///
/// impl Agent for MyProfiler {
///     fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
///         println!("Profiler loaded!");
///         jni::JNI_OK
///     }
///
///     fn method_entry(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID) {
///         self.method_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
///     }
/// }
///
/// export_agent!(MyProfiler);
/// ```
///
/// # Capabilities
///
/// Many events require specific JVMTI capabilities to be enabled. Use
/// [`wrapper::Jvmti::add_capabilities`] in your `on_load` to request them.
pub trait Agent: Sync + Send {
    /// Called when the agent is loaded into the JVM.
    ///
    /// This is your initialization point. Use it to:
    /// - Parse agent options
    /// - Request JVMTI capabilities
    /// - Set up event callbacks
    /// - Initialize your agent's state
    ///
    /// Return `JNI_OK` (0) on success, or `JNI_ERR` (-1) on failure.
    fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint;

    /// Called when the agent is unloaded (JVM shutdown).
    ///
    /// Use this for cleanup: flush buffers, close files, etc.
    fn on_unload(&self) {}

    // =========================================================================
    // VM LIFECYCLE EVENTS
    // =========================================================================

    /// Called when the VM initialization is complete.
    ///
    /// At this point, JNI is fully functional and you can safely call JNI functions.
    /// The `thread` parameter is the main thread.
    fn vm_init(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread) {}

    /// Called when the VM is about to terminate.
    ///
    /// This is your last chance to perform cleanup that requires JNI.
    fn vm_death(&self, _jni: *mut jni::JNIEnv) {}

    /// Called when the VM starts (before `vm_init`).
    ///
    /// JNI is available but limited - you cannot create new threads or load classes.
    /// Requires `can_generate_early_vmstart` capability for early delivery.
    fn vm_start(&self, _jni: *mut jni::JNIEnv) {}

    // =========================================================================
    // THREAD EVENTS
    // =========================================================================

    /// Called when a new thread starts.
    ///
    /// Fired for every thread including the main thread.
    fn thread_start(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread) {}

    /// Called when a thread is about to terminate.
    fn thread_end(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread) {}

    // =========================================================================
    // CLASS EVENTS
    // =========================================================================

    /// Called when a class is first loaded (before linking).
    ///
    /// The class is not yet usable at this point.
    fn class_load(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _klass: jni::jclass) {}

    /// Called when a class is prepared (linked and ready to use).
    ///
    /// At this point you can query the class's methods and fields.
    fn class_prepare(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _klass: jni::jclass) {}

    /// Called when class bytecode is being loaded or redefined.
    ///
    /// This is your hook for bytecode instrumentation (BCI). To modify the class:
    /// 1. Allocate memory with `Jvmti::allocate()`
    /// 2. Write your modified bytecode to it
    /// 3. Set `new_class_data_len` and `new_class_data`
    ///
    /// Requires `can_generate_all_class_hook_events` or `can_retransform_classes`.
    fn class_file_load_hook(&self, _jni: *mut jni::JNIEnv, _class_being_redefined: jni::jclass,
                            _loader: jni::jobject, _name: *const std::os::raw::c_char,
                            _protection_domain: jni::jobject, _class_data_len: jni::jint,
                            _class_data: *const std::os::raw::c_uchar,
                            _new_class_data_len: *mut jni::jint,
                            _new_class_data: *mut *mut std::os::raw::c_uchar) {}

    // =========================================================================
    // METHOD EVENTS
    // =========================================================================

    /// Called when a method is entered.
    ///
    /// **Warning**: This fires for EVERY method call - extremely high overhead.
    /// Requires `can_generate_method_entry_events` capability.
    fn method_entry(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID) {}

    /// Called when a method is about to return.
    ///
    /// **Warning**: This fires for EVERY method return - extremely high overhead.
    /// Requires `can_generate_method_exit_events` capability.
    fn method_exit(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID) {}

    /// Called when a native method is bound to its implementation.
    ///
    /// You can redirect native methods by setting `*new_address_ptr`.
    /// Requires `can_generate_native_method_bind_events` capability.
    fn native_method_bind(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID, _address: *mut std::os::raw::c_void, _new_address_ptr: *mut *mut std::os::raw::c_void) {}

    // =========================================================================
    // COMPILED CODE EVENTS (JIT)
    // =========================================================================

    /// Called when a method is JIT-compiled.
    ///
    /// Useful for profilers that need to map native code addresses to methods.
    /// Requires `can_generate_compiled_method_load_events` capability.
    fn compiled_method_load(&self, _method: jni::jmethodID, _code_size: jni::jint, _code_addr: *const std::os::raw::c_void, _map_length: jni::jint, _map: *const std::os::raw::c_void, _compile_info: *const std::os::raw::c_void) {}

    /// Called when JIT-compiled code is unloaded (deoptimized).
    fn compiled_method_unload(&self, _method: jni::jmethodID, _code_addr: *const std::os::raw::c_void) {}

    /// Called when dynamic code is generated (e.g., JIT stubs).
    fn dynamic_code_generated(&self, _name: *const std::os::raw::c_char, _address: *const std::os::raw::c_void, _length: jni::jint) {}

    // =========================================================================
    // EXCEPTION EVENTS
    // =========================================================================

    /// Called when an exception is thrown.
    ///
    /// `catch_method` and `catch_location` indicate where it will be caught,
    /// or are null/0 if uncaught. Requires `can_generate_exception_events`.
    fn exception(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID,
                 _location: jvmti::jlocation, _exception: jni::jobject,
                 _catch_method: jni::jmethodID, _catch_location: jvmti::jlocation) {}

    /// Called when an exception is caught.
    fn exception_catch(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID,
                       _location: jvmti::jlocation, _exception: jni::jobject) {}

    // =========================================================================
    // DEBUGGING EVENTS
    // =========================================================================

    /// Called before each bytecode instruction (single-stepping).
    ///
    /// **Extreme overhead** - only use for debugging.
    /// Requires `can_generate_single_step_events` capability.
    fn single_step(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID, _location: jvmti::jlocation) {}

    /// Called when a breakpoint is hit.
    ///
    /// Requires `can_generate_breakpoint_events` capability.
    fn breakpoint(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID, _location: jvmti::jlocation) {}

    /// Called when a frame is popped (method returns or exception thrown).
    ///
    /// Must be registered per-frame with `notify_frame_pop`.
    /// Requires `can_generate_frame_pop_events` capability.
    fn frame_pop(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID, _was_popped_by_exception: jni::jboolean) {}

    // =========================================================================
    // MONITOR EVENTS
    // =========================================================================

    /// Called when a thread is about to wait on a monitor (`Object.wait()`).
    ///
    /// Requires `can_generate_monitor_events` capability.
    fn monitor_wait(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _object: jni::jobject, _timeout: jni::jlong) {}

    /// Called when a thread finishes waiting on a monitor.
    ///
    /// `timed_out` indicates if the wait timed out.
    fn monitor_waited(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _object: jni::jobject, _timed_out: jni::jboolean) {}

    /// Called when a thread is about to block on a contended monitor.
    fn monitor_contended_enter(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _object: jni::jobject) {}

    /// Called when a thread acquires a previously contended monitor.
    fn monitor_contended_entered(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _object: jni::jobject) {}

    // =========================================================================
    // FIELD EVENTS (WATCHPOINTS)
    // =========================================================================

    /// Called when a watched field is read.
    ///
    /// Set up with `set_field_access_watch`. Requires `can_generate_field_access_events`.
    fn field_access(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID,
                    _location: jvmti::jlocation, _field_klass: jni::jclass, _object: jni::jobject, _field: jni::jobject) {}

    /// Called when a watched field is modified.
    ///
    /// Set up with `set_field_modification_watch`. Requires `can_generate_field_modification_events`.
    fn field_modification(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _method: jni::jmethodID,
                          _location: jvmti::jlocation, _field_klass: jni::jclass, _object: jni::jobject,
                          _field: jni::jobject, _sig_type: std::os::raw::c_char, _new_value: jni::jvalue) {}

    // =========================================================================
    // GC & MEMORY EVENTS
    // =========================================================================

    /// Called when garbage collection starts.
    ///
    /// **No JNI calls allowed** during this callback.
    /// Requires `can_generate_garbage_collection_events` capability.
    fn garbage_collection_start(&self) {}

    /// Called when garbage collection finishes.
    ///
    /// **No JNI calls allowed** during this callback.
    fn garbage_collection_finish(&self) {}

    /// Called when a critical resource is exhausted (heap, threads, etc.).
    fn resource_exhausted(&self, _jni: *mut jni::JNIEnv, _flags: jni::jint, _description: *const std::os::raw::c_char) {}

    // =========================================================================
    // OBJECT EVENTS
    // =========================================================================

    /// Called when a tagged object is garbage collected.
    ///
    /// Use `set_tag` to tag objects you want to track.
    /// Requires `can_generate_object_free_events` capability.
    fn object_free(&self, _tag: jni::jlong) {}

    /// Called when an object is allocated (VM-internal allocations).
    ///
    /// Does NOT fire for all allocations - use sampling for comprehensive coverage.
    /// Requires `can_generate_vm_object_alloc_events` capability.
    fn vm_object_alloc(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _object: jni::jobject, _klass: jni::jclass, _size: jni::jlong) {}

    /// Called for sampled object allocations (JDK 11+).
    ///
    /// Configure sampling rate with `set_heap_sampling_interval`.
    /// Requires `can_generate_sampled_object_alloc_events` capability.
    fn sampled_object_alloc(&self, _jni: *mut jni::JNIEnv, _thread: jni::jthread, _object: jni::jobject, _klass: jni::jclass, _size: jni::jlong) {}
}

// 2. THE GLOBAL SINGLETON
// This holds the user's Agent instance so static C functions can find it.
pub static GLOBAL_AGENT: OnceLock<Box<dyn Agent>> = OnceLock::new();

/// Helper to initialize the global agent (called by the macro)
pub fn set_global_agent(agent: Box<dyn Agent>) -> Result<(), ()> {
    GLOBAL_AGENT.set(agent).map_err(|_| ())
}

unsafe extern "system" fn trampoline_method_entry(
    _jvmti_env: *mut sys::jvmti::jvmtiEnv,
    jni_env: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
) {
    if let Some(agent) = GLOBAL_AGENT.get() {
        agent.method_entry(jni_env, thread, method);
    }
}

unsafe extern "system" fn trampoline_method_exit(
    _jvmti_env: *mut sys::jvmti::jvmtiEnv,
    jni_env: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
    _was_popped: jni::jboolean,
    _ret_val: jni::jvalue,
) {
    if let Some(agent) = GLOBAL_AGENT.get() {
        agent.method_exit(jni_env, thread, method);
    }
}

unsafe extern "system" fn trampoline_native_method_bind(
    _env: *mut sys::jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, method: jni::jmethodID,
    address: *mut std::os::raw::c_void, new_address_ptr: *mut *mut std::os::raw::c_void
) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.native_method_bind(jni, thread, method, address, new_address_ptr); }
}


// --- 1. Lifecycle ---
unsafe extern "system" fn trampoline_vm_init(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.vm_init(jni, thread); }
}
unsafe extern "system" fn trampoline_vm_death(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.vm_death(jni); }
}
unsafe extern "system" fn trampoline_vm_start(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.vm_start(jni); }
}

// --- 2. Threads ---
unsafe extern "system" fn trampoline_thread_start(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.thread_start(jni, thread); }
}
unsafe extern "system" fn trampoline_thread_end(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.thread_end(jni, thread); }
}

// --- 3. Classes ---
unsafe extern "system" fn trampoline_class_load(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, klass: jni::jclass) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.class_load(jni, thread, klass); }
}
unsafe extern "system" fn trampoline_class_prepare(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, klass: jni::jclass) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.class_prepare(jni, thread, klass); }
}

// --- 3.5 Compiled Code ---
unsafe extern "system" fn trampoline_compiled_method_load(
    _env: *mut jvmti::jvmtiEnv, method: jni::jmethodID, code_size: jni::jint, code_addr: *const std::os::raw::c_void,
    map_length: jni::jint, map: *const std::os::raw::c_void, compile_info: *const std::os::raw::c_void
) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.compiled_method_load(method, code_size, code_addr, map_length, map, compile_info); }
}
unsafe extern "system" fn trampoline_compiled_method_unload(_env: *mut jvmti::jvmtiEnv, method: jni::jmethodID, code_addr: *const std::os::raw::c_void) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.compiled_method_unload(method, code_addr); }
}
unsafe extern "system" fn trampoline_dynamic_code_generated(_env: *mut jvmti::jvmtiEnv, name: *const std::os::raw::c_char, address: *const std::os::raw::c_void, length: jni::jint) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.dynamic_code_generated(name, address, length); }
}
unsafe extern "system" fn trampoline_class_file_load_hook(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv,
    class_being_redefined: jni::jclass, loader: jni::jobject, name: *const std::os::raw::c_char,
    protection_domain: jni::jobject, class_data_len: jni::jint, class_data: *const std::os::raw::c_uchar,
    new_class_data_len: *mut jni::jint, new_class_data: *mut *mut std::os::raw::c_uchar
) {
    if let Some(agent) = GLOBAL_AGENT.get() {
        agent.class_file_load_hook(jni, class_being_redefined, loader, name, protection_domain, class_data_len, class_data, new_class_data_len, new_class_data);
    }
}

// --- 4. Exceptions ---
unsafe extern "system" fn trampoline_exception(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, method: jni::jmethodID,
    location: jvmti::jlocation, exception: jni::jobject, catch_method: jni::jmethodID, catch_location: jvmti::jlocation
) {
    if let Some(agent) = GLOBAL_AGENT.get() {
        agent.exception(jni, thread, method, location, exception, catch_method, catch_location);
    }
}
unsafe extern "system" fn trampoline_exception_catch(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, method: jni::jmethodID,
    location: jvmti::jlocation, exception: jni::jobject
) {
    if let Some(agent) = GLOBAL_AGENT.get() {
        agent.exception_catch(jni, thread, method, location, exception);
    }
}

// --- 5. Debugging ---
unsafe extern "system" fn trampoline_single_step(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, method: jni::jmethodID, location: jvmti::jlocation
) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.single_step(jni, thread, method, location); }
}
unsafe extern "system" fn trampoline_breakpoint(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, method: jni::jmethodID, location: jvmti::jlocation
) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.breakpoint(jni, thread, method, location); }
}
unsafe extern "system" fn trampoline_frame_pop(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, method: jni::jmethodID, was_popped: jni::jboolean
) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.frame_pop(jni, thread, method, was_popped); }
}

// --- 5.5 Monitors ---
unsafe extern "system" fn trampoline_monitor_wait(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, object: jni::jobject, timeout: jni::jlong) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.monitor_wait(jni, thread, object, timeout); }
}
unsafe extern "system" fn trampoline_monitor_waited(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, object: jni::jobject, timed_out: jni::jboolean) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.monitor_waited(jni, thread, object, timed_out); }
}
unsafe extern "system" fn trampoline_monitor_contended_enter(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, object: jni::jobject) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.monitor_contended_enter(jni, thread, object); }
}
unsafe extern "system" fn trampoline_monitor_contended_entered(_env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, object: jni::jobject) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.monitor_contended_entered(jni, thread, object); }
}

// --- 6. Fields ---
unsafe extern "system" fn trampoline_field_access(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, method: jni::jmethodID,
    location: jvmti::jlocation, field_klass: jni::jclass, object: jni::jobject, field: crate::sys::jni::jfieldID
) {
    // Cast fieldID to jobject (void*) to match trait signature, or update trait to use jfieldID
    if let Some(agent) = GLOBAL_AGENT.get() { agent.field_access(jni, thread, method, location, field_klass, object, field as jni::jobject); }
}
unsafe extern "system" fn trampoline_field_modification(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread, method: jni::jmethodID,
    location: jvmti::jlocation, field_klass: jni::jclass, object: jni::jobject, field: crate::sys::jni::jfieldID,
    sig_type: std::os::raw::c_char, new_value: jni::jvalue
) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.field_modification(jni, thread, method, location, field_klass, object, field as jni::jobject, sig_type, new_value); }
}

// --- 7. GC & Resource ---
unsafe extern "system" fn trampoline_garbage_collection_start(_env: *mut jvmti::jvmtiEnv) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.garbage_collection_start(); }
}
unsafe extern "system" fn trampoline_garbage_collection_finish(_env: *mut jvmti::jvmtiEnv) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.garbage_collection_finish(); }
}
unsafe extern "system" fn trampoline_resource_exhausted(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, flags: jni::jint,
    _reserved: *const std::os::raw::c_void, description: *const std::os::raw::c_char
) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.resource_exhausted(jni, flags, description); }
}

// --- 8. Objects ---
unsafe extern "system" fn trampoline_object_free(_env: *mut jvmti::jvmtiEnv, tag: jni::jlong) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.object_free(tag); }
}
unsafe extern "system" fn trampoline_vm_object_alloc(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread,
    object: jni::jobject, klass: jni::jclass, size: jni::jlong
) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.vm_object_alloc(jni, thread, object, klass, size); }
}
unsafe extern "system" fn trampoline_sampled_object_alloc(
    _env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv, thread: jni::jthread,
    object: jni::jobject, klass: jni::jclass, size: jni::jlong
) {
    if let Some(agent) = GLOBAL_AGENT.get() { agent.sampled_object_alloc(jni, thread, object, klass, size); }
}




/// Returns a pre-configured `jvmtiEventCallbacks` struct with all event trampolines wired up.
///
/// This function populates a callbacks struct that routes all JVMTI events to your
/// [`Agent`] implementation via the global agent instance. Use this with
/// [`wrapper::Jvmti::set_event_callbacks`] to enable event delivery.
///
/// # Example
///
/// ```rust,ignore
/// fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
///     let jvmti = jvmti::wrapper::Jvmti::from_java_vm(vm).unwrap();
///
///     // Wire up all event callbacks
///     let callbacks = jvmti::get_default_callbacks();
///     jvmti.set_event_callbacks(&callbacks);
///
///     // Enable specific events you care about
///     jvmti.set_event_notification_mode(
///         jvmti::sys::jvmti::JVMTI_ENABLE,
///         jvmti::sys::jvmti::JVMTI_EVENT_VM_INIT,
///         std::ptr::null_mut()
///     );
///
///     jni::JNI_OK
/// }
/// ```
///
/// # Events Wired
///
/// All standard JVMTI events are wired:
/// - VM lifecycle: `VMInit`, `VMDeath`, `VMStart`
/// - Threads: `ThreadStart`, `ThreadEnd`
/// - Classes: `ClassLoad`, `ClassPrepare`, `ClassFileLoadHook`
/// - Methods: `MethodEntry`, `MethodExit`, `NativeMethodBind`
/// - Compilation: `CompiledMethodLoad`, `CompiledMethodUnload`, `DynamicCodeGenerated`
/// - Exceptions: `Exception`, `ExceptionCatch`
/// - Debugging: `SingleStep`, `Breakpoint`, `FramePop`
/// - Monitors: `MonitorWait`, `MonitorWaited`, `MonitorContendedEnter`, `MonitorContendedEntered`
/// - Fields: `FieldAccess`, `FieldModification`
/// - GC: `GarbageCollectionStart`, `GarbageCollectionFinish`, `ResourceExhausted`
/// - Objects: `ObjectFree`, `VMObjectAlloc`, `SampledObjectAlloc`
pub fn get_default_callbacks() -> jvmti::jvmtiEventCallbacks {

    let mut callbacks = jvmti::jvmtiEventCallbacks::default();

    callbacks.VMInit = Some(trampoline_vm_init);
    callbacks.VMDeath = Some(trampoline_vm_death);
    callbacks.VMStart = Some(trampoline_vm_start);

    callbacks.ThreadStart = Some(trampoline_thread_start);
    callbacks.ThreadEnd = Some(trampoline_thread_end);

    callbacks.ClassLoad = Some(trampoline_class_load);
    callbacks.ClassPrepare = Some(trampoline_class_prepare);
    callbacks.ClassFileLoadHook = Some(trampoline_class_file_load_hook);

    callbacks.Exception = Some(trampoline_exception);
    callbacks.ExceptionCatch = Some(trampoline_exception_catch);

    callbacks.SingleStep = Some(trampoline_single_step);
    callbacks.Breakpoint = Some(trampoline_breakpoint);
    callbacks.FramePop = Some(trampoline_frame_pop);

    callbacks.FieldAccess = Some(trampoline_field_access);
    callbacks.FieldModification = Some(trampoline_field_modification);

    callbacks.MethodEntry = Some(trampoline_method_entry);
    callbacks.MethodExit = Some(trampoline_method_exit);
    callbacks.NativeMethodBind = Some(trampoline_native_method_bind);

    callbacks.CompiledMethodLoad = Some(trampoline_compiled_method_load);
    callbacks.CompiledMethodUnload = Some(trampoline_compiled_method_unload);
    callbacks.DynamicCodeGenerated = Some(trampoline_dynamic_code_generated);

    callbacks.MonitorWait = Some(trampoline_monitor_wait);
    callbacks.MonitorWaited = Some(trampoline_monitor_waited);
    callbacks.MonitorContendedEnter = Some(trampoline_monitor_contended_enter);
    callbacks.MonitorContendedEntered = Some(trampoline_monitor_contended_entered);

    callbacks.GarbageCollectionStart = Some(trampoline_garbage_collection_start);
    callbacks.GarbageCollectionFinish = Some(trampoline_garbage_collection_finish);
    callbacks.ResourceExhausted = Some(trampoline_resource_exhausted);

    callbacks.ObjectFree = Some(trampoline_object_free);
    callbacks.VMObjectAlloc = Some(trampoline_vm_object_alloc);
    callbacks.SampledObjectAlloc = Some(trampoline_sampled_object_alloc);

    callbacks
}


/// Exports your agent type as a loadable JVMTI agent library.
///
/// This macro generates the required `Agent_OnLoad` and `Agent_OnUnload` FFI entry points
/// that the JVM expects when loading an agent via `-agentpath` or `-agentlib`.
///
/// # Requirements
///
/// Your agent type must implement:
/// - [`Agent`] trait - for handling JVMTI events
/// - [`Default`] trait - for instantiation (the macro calls `<YourType>::default()`)
/// - [`Sync`] + [`Send`] - for thread-safe event handling (enforced by `Agent` trait bounds)
///
/// # Generated Functions
///
/// The macro generates two `extern "system"` functions:
///
/// - **`Agent_OnLoad`**: Called by the JVM when the agent is loaded. Creates your agent
///   instance, registers it globally, and calls your [`Agent::on_load`] method.
///
/// - **`Agent_OnUnload`**: Called by the JVM during shutdown. Calls your [`Agent::on_unload`]
///   method for cleanup.
///
/// # Example
///
/// ```rust,ignore
/// use jvmti::{Agent, export_agent, sys::jni};
///
/// #[derive(Default)]
/// struct MyAgent {
///     // Your agent state here
/// }
///
/// impl Agent for MyAgent {
///     fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
///         println!("Agent loaded with options: {}", options);
///         jni::JNI_OK
///     }
/// }
///
/// // This generates Agent_OnLoad and Agent_OnUnload
/// export_agent!(MyAgent);
/// ```
///
/// # Building
///
/// Your crate must be built as a C dynamic library. Add to `Cargo.toml`:
///
/// ```toml
/// [lib]
/// crate-type = ["cdylib"]
/// ```
///
/// # Loading the Agent
///
/// ```bash
/// # Build your agent
/// cargo build --release
///
/// # Load with JVM (Linux)
/// java -agentpath:./target/release/libmyagent.so=option1,option2 MyApp
///
/// # Load with JVM (macOS)
/// java -agentpath:./target/release/libmyagent.dylib=option1,option2 MyApp
///
/// # Load with JVM (Windows)
/// java -agentpath:./target/release/myagent.dll=option1,option2 MyApp
/// ```
///
/// # Options String
///
/// The options string (everything after `=` in `-agentpath`) is passed to your
/// [`Agent::on_load`] method. Parse it however you like - common patterns include
/// comma-separated key=value pairs or simple flags.
///
/// # Thread Safety Notes
///
/// - Only one agent instance is created per JVM (stored in a global `OnceLock`)
/// - Your agent's methods may be called concurrently from multiple JVM threads
/// - Use interior mutability (`Mutex`, `RwLock`, `AtomicXxx`) for mutable state
///
/// # Return Values
///
/// Your `on_load` must return:
/// - [`jni::JNI_OK`] (0) on success - JVM continues loading
/// - [`jni::JNI_ERR`] (-1) on failure - JVM aborts startup with an error
#[macro_export]
macro_rules! export_agent {
    ($agent_type:ty) => {
        #[no_mangle]
        pub unsafe extern "system" fn Agent_OnLoad(
            vm: *mut $crate::sys::jni::JavaVM,
            options: *mut std::ffi::c_char,
            reserved: *mut std::ffi::c_void,
        ) -> $crate::sys::jni::jint {

            // 1. Create and Register the Agent
            let agent = Box::new(<$agent_type>::default());
            if let Err(_) = $crate::set_global_agent(agent) {
                return $crate::sys::jni::JNI_ERR;
            }

            // 2. Handle Options
            let options_str = if options.is_null() {
                ""
            } else {
                std::ffi::CStr::from_ptr(options).to_str().unwrap_or("")
            };

            // 3. Call the User's Logic
            if let Some(global_agent) = $crate::GLOBAL_AGENT.get() {
                return global_agent.on_load(vm, options_str);
            }

            $crate::sys::jni::JNI_ERR
        }

        #[no_mangle]
        pub unsafe extern "system" fn Agent_OnUnload(vm: *mut $crate::sys::jni::JavaVM) {
             if let Some(agent) = $crate::GLOBAL_AGENT.get() {
                agent.on_unload();
            }
        }
    };
}