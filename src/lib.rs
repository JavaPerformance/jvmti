#![deny(unsafe_op_in_unsafe_fn)]

//! # jvmti
//!
//! Complete JNI and JVMTI bindings for Rust with **zero third-party crate dependencies**.
//!
//! This crate provides everything you need to build JVM agents in Rust:
//! - Low-level FFI bindings to JNI and JVMTI
//! - High-level wrappers with ergonomic Rust APIs
//! - The [`Agent`] trait and [`export_agent!`] macro for easy agent creation
//!
//! ## Version 3 Migration Notice
//!
//! Version 3.0 is intentionally source-breaking from 2.x. Version 2.4.0 was
//! not published because the planned ABI, callback, ownership, and lifecycle
//! corrections required a major-version release under semantic versioning.
//! Existing agents must migrate their callback implementations and review all
//! affected safety and ownership contracts rather than changing only the Cargo
//! dependency version. See the complete [2.x to 3.0 migration guide] before
//! upgrading a production agent.
//!
//! [2.x to 3.0 migration guide]: https://github.com/JavaPerformance/jvmti/blob/v3.0.0/docs/MIGRATING_2_TO_3.md
//!
//! ## Features
//!
//! - **Complete Coverage**: Complete JDK 28 JNI and JVM TI function tables
//! - **Zero Third-Party Crates**: Including optional features, tests, tools, and benchmarks
//! - **Ergonomic API**: High-level wrappers handle strings, arrays, references
//! - **Type-Safe**: Proper Rust types, `Result` returns, RAII guards
//! - **Release-Aware**: source-ABI verified against pinned OpenJDK 8-28 revisions and live callback-tested through JDK 28 preview
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
//! jvmti-bindings = "3"
//! ```
//!
//! **3. Implement your agent (src/lib.rs):**
//! ```rust,no_run
//! use jvmti_bindings::prelude::*;
//!
//! #[derive(Default)]
//! struct MyAgent;
//!
//! impl Agent for MyAgent {
//!     fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
//!         println!("[MyAgent] Loaded with options: {:?}", context.options_lossy());
//!         jni::JNI_OK
//!     }
//!
//!     fn vm_init(&self, _context: CallbackContext<'_>, _event: ThreadEvent) {
//!         println!("[MyAgent] VM initialized!");
//!     }
//!
//!     fn vm_death(&self, _context: CallbackContext<'_>) {
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
//! │   env::LocalRef, GlobalRef, WeakGlobalRef - RAII guards  │
//! ├─────────────────────────────────────────────────────────┤
//! │              Raw FFI Bindings (sys module)               │
//! │   sys::jni - JNI types, JDK 28 vtable                    │
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
//! | [`mod@env`] | **High-level wrappers** - start here for ergonomic APIs |
//! | [`env::Jvmti`] | JVMTI environment wrapper (153 methods) |
//! | [`env::JniEnv`] | JNI environment wrapper (60+ methods) |
//! | [`classfile`] | Typed JVMS-standard attributes through Java 28; opaque unknown attributes |
//! | [`mutf8`] | Java Modified UTF-8 and exact UTF-16 conversions |
//! | [`prelude`] | Recommended imports for agents |
//! | [`embed`] | Optional JVM embedding helpers (`embed` feature) |
//! | [`advanced`] | Feature-gated advanced helpers (heap graph utilities) |
//!
//! ## Enabling JVMTI Events
//!
//! To receive JVMTI events, you must:
//! 1. Request the required capabilities
//! 2. Set up event callbacks
//! 3. Enable the specific events
//!
//! ```rust,no_run
//! use jvmti_bindings::prelude::*;
//!
//! #[derive(Default)]
//! struct ClassMonitor;
//!
//! impl Agent for ClassMonitor {
//!     fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
//!         let Ok(jvmti_env) = context.vm().jvmti() else {
//!             return jni::JNI_ERR;
//!         };
//!
//!         // 1. Request capabilities
//!         let mut caps = jvmti::jvmtiCapabilities::default();
//!         caps.set_can_generate_all_class_hook_events(true);
//!         if jvmti_env.add_capabilities(&caps).is_err() {
//!             return jni::JNI_ERR;
//!         }
//!
//!         // 2. Set up callbacks (wires all events to your Agent impl)
//!         let callbacks = get_default_callbacks();
//!         if jvmti_env.set_event_callbacks(callbacks).is_err() {
//!             return jni::JNI_ERR;
//!         }
//!
//!         // 3. Enable specific events
//!         if unsafe { jvmti_env.set_event_notification_mode(
//!             true,  // enable
//!             jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK,
//!             std::ptr::null_mut()  // all threads
//!         ) }.is_err() {
//!             return jni::JNI_ERR;
//!         }
//!
//!         jni::JNI_OK
//!     }
//!
//!     fn class_file_load_hook(
//!         &self,
//!         _context: CallbackContext<'_>,
//!         _event: ClassFileLoadHookEvent<'_>,
//!     ) {
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
//! ```rust,no_run
//! use jvmti_bindings::prelude::*;
//!
//! fn print_message(jni: &JniEnv) {
//!
//!     // Find a class
//!     let Some(system_class) = jni.find_class("java/lang/System") else {
//!         return;
//!     };
//!
//!     // Get a static field
//!     let Some(out_field) = (unsafe {
//!         jni.get_static_field_id(system_class, "out", "Ljava/io/PrintStream;")
//!     }) else {
//!         return;
//!     };
//!     let out = unsafe { jni.get_static_object_field(system_class, out_field) };
//!
//!     // Create a Java string
//!     let Some(message) = jni.new_string_utf("Hello from Rust!") else {
//!         return;
//!     };
//!
//!     // Call a method
//!     let Some(print_class) = jni.find_class("java/io/PrintStream") else {
//!         return;
//!     };
//!     let Some(println_method) = (unsafe {
//!         jni.get_method_id(print_class, "println", "(Ljava/lang/String;)V")
//!     }) else {
//!         return;
//!     };
//!     unsafe { jni.call_void_method(out, println_method, &[jni::jvalue { l: message }]) };
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
//! | 8           | 233           | 155             | Supported baseline |
//! | 9           | 234           | 155             | +GetModule, +module functions |
//! | 11          | 234           | 156             | +SetHeapSamplingInterval |
//! | 21          | 235           | 156             | +IsVirtualThread, virtual threads final |
//! | 24          | 236           | 156             | +GetStringUTFLengthAsLong |
//! | 25          | 236           | 156             | +ClearAllFramePops (slot 67) |
//! | 28 preview  | 237           | 156             | +HasIdentity, value-object semantics |

#[cfg(feature = "advanced")]
pub mod advanced;
pub mod agent;
pub mod callbacks;
pub mod classfile;
#[cfg(feature = "embed")]
mod dynamic_library;
#[cfg(feature = "embed")]
pub mod embed;
pub mod env;
pub mod mutf8;
pub mod prelude;
pub mod sys;
pub mod version;

// Implementation modules (use `env` module for the public API)
#[doc(hidden)]
pub(crate) mod jni_wrapper;
#[doc(hidden)]
pub(crate) mod jvmti_wrapper;

pub use crate::sys::jni;
use crate::sys::jvmti;
use std::sync::OnceLock;

/// Return a display-ready JNI result string, e.g. `JNI_EDETACHED (-2)`.
///
/// This is a convenience wrapper around [`jni::describe_result`].
pub fn describe_jni_result(code: jni::jint) -> String {
    jni::describe_result(code)
}

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
/// ```rust,no_run
/// use jvmti_bindings::prelude::*;
///
/// #[derive(Default)]
/// struct MyProfiler {
///     method_count: std::sync::atomic::AtomicU64,
/// }
///
/// impl Agent for MyProfiler {
///     fn on_load(&self, _context: AgentLoadContext<'_>) -> jni::jint {
///         println!("Profiler loaded!");
///         jni::JNI_OK
///     }
///
///     fn method_entry(&self, _context: CallbackContext<'_>, _event: MethodEvent) {
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
/// [`env::Jvmti::add_capabilities`] in your `on_load` to request them.
pub trait Agent: Sync + Send {
    /// Called when the agent is loaded into the JVM.
    fn on_load(&self, context: agent::AgentLoadContext<'_>) -> jni::jint;

    /// Called when the agent is attached to a running JVM.
    fn on_attach(&self, _context: agent::AgentLoadContext<'_>) -> jni::jint {
        jni::JNI_OK
    }

    /// Called when the agent is unloaded.
    fn on_unload(&self, _context: agent::AgentUnloadContext<'_>) {}

    /// Called after an agent callback panics and the unwind has been contained.
    ///
    /// This hook must not panic. The default deliberately performs no I/O
    /// because several JVMTI callback phases have severe operation limits.
    fn callback_panicked(&self, _event: &'static str) {}

    fn vm_init(&self, _context: callbacks::CallbackContext<'_>, _event: callbacks::ThreadEvent) {}
    fn vm_death(&self, _context: callbacks::CallbackContext<'_>) {}
    fn vm_start(&self, _context: callbacks::CallbackContext<'_>) {}

    fn thread_start(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::ThreadEvent,
    ) {
    }
    fn thread_end(&self, _context: callbacks::CallbackContext<'_>, _event: callbacks::ThreadEvent) {
    }
    fn virtual_thread_start(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::ThreadEvent,
    ) {
    }
    fn virtual_thread_end(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::ThreadEvent,
    ) {
    }

    fn class_load(&self, _context: callbacks::CallbackContext<'_>, _event: callbacks::ClassEvent) {}
    fn class_prepare(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::ClassEvent,
    ) {
    }
    fn class_file_load_hook<'callback>(
        &self,
        _context: callbacks::CallbackContext<'callback>,
        _event: callbacks::ClassFileLoadHookEvent<'callback>,
    ) {
    }

    fn method_entry(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::MethodEvent,
    ) {
    }
    fn method_exit(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::MethodExitEvent,
    ) {
    }
    fn native_method_bind(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::NativeMethodBindEvent,
    ) {
    }

    fn compiled_method_load<'callback>(
        &self,
        _context: callbacks::CallbackContext<'callback>,
        _event: callbacks::CompiledMethodLoadEvent<'callback>,
    ) {
    }
    fn compiled_method_unload(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::CompiledMethodUnloadEvent,
    ) {
    }
    fn dynamic_code_generated<'callback>(
        &self,
        _context: callbacks::CallbackContext<'callback>,
        _event: callbacks::DynamicCodeGeneratedEvent<'callback>,
    ) {
    }
    fn data_dump_request(&self, _context: callbacks::CallbackContext<'_>) {}

    fn exception(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::ExceptionEvent,
    ) {
    }
    fn exception_catch(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::ExceptionCatchEvent,
    ) {
    }
    fn single_step(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::LocationEvent,
    ) {
    }
    fn breakpoint(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::LocationEvent,
    ) {
    }
    fn frame_pop(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::FramePopEvent,
    ) {
    }

    fn monitor_wait(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::MonitorWaitEvent,
    ) {
    }
    fn monitor_waited(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::MonitorWaitedEvent,
    ) {
    }
    fn monitor_contended_enter(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::MonitorEvent,
    ) {
    }
    fn monitor_contended_entered(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::MonitorEvent,
    ) {
    }

    fn field_access(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::FieldAccessEvent,
    ) {
    }
    fn field_modification(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::FieldModificationEvent,
    ) {
    }

    /// No JNI environment is available and only GC-safe JVMTI operations may be used.
    fn garbage_collection_start(&self, _context: callbacks::CallbackContext<'_>) {}
    /// No JNI environment is available and only GC-safe JVMTI operations may be used.
    fn garbage_collection_finish(&self, _context: callbacks::CallbackContext<'_>) {}
    fn resource_exhausted<'callback>(
        &self,
        _context: callbacks::CallbackContext<'callback>,
        _event: callbacks::ResourceExhaustedEvent<'callback>,
    ) {
    }

    /// No JNI environment is available and only object-free-safe JVMTI operations may be used.
    fn object_free(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::ObjectFreeEvent,
    ) {
    }
    fn vm_object_alloc(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::ObjectAllocationEvent,
    ) {
    }
    fn sampled_object_alloc(
        &self,
        _context: callbacks::CallbackContext<'_>,
        _event: callbacks::ObjectAllocationEvent,
    ) {
    }
}

// 2. THE GLOBAL SINGLETON
// This holds the user's Agent instance so static C functions can find it.
pub static GLOBAL_AGENT: OnceLock<Box<dyn Agent>> = OnceLock::new();

/// The process-global agent has already been initialized.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct GlobalAgentAlreadySet;

impl std::fmt::Display for GlobalAgentAlreadySet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the process-global JVM TI agent is already initialized")
    }
}

impl std::error::Error for GlobalAgentAlreadySet {}

/// Helper to initialize the global agent (called by the macro)
pub fn set_global_agent(agent: Box<dyn Agent>) -> Result<(), GlobalAgentAlreadySet> {
    GLOBAL_AGENT.set(agent).map_err(|_| GlobalAgentAlreadySet)
}

fn global_agent_or_init<A>() -> &'static dyn Agent
where
    A: Agent + Default + 'static,
{
    GLOBAL_AGENT.get_or_init(|| Box::new(A::default())).as_ref()
}

#[doc(hidden)]
pub unsafe fn __agent_on_load<A>(
    vm: *mut jni::JavaVM,
    options: *mut std::ffi::c_char,
    reserved: *mut std::ffi::c_void,
) -> jni::jint
where
    A: Agent + Default + 'static,
{
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let context = unsafe { agent::AgentLoadContext::from_raw(vm, options, reserved) }
            .ok_or(jni::JNI_ERR)?;
        Ok(global_agent_or_init::<A>().on_load(context))
    }));

    match outcome {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error,
        Err(_) => {
            if let Some(agent) = GLOBAL_AGENT.get() {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    agent.callback_panicked("Agent_OnLoad")
                }));
            }
            jni::JNI_ERR
        }
    }
}

#[doc(hidden)]
pub unsafe fn __agent_on_attach<A>(
    vm: *mut jni::JavaVM,
    options: *mut std::ffi::c_char,
    reserved: *mut std::ffi::c_void,
) -> jni::jint
where
    A: Agent + Default + 'static,
{
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let context = unsafe { agent::AgentLoadContext::from_raw(vm, options, reserved) }
            .ok_or(jni::JNI_ERR)?;
        // The Attach API invokes Agent_OnAttach even when this library was
        // already loaded. Reuse the process-global agent rather than rejecting
        // every attach after the first one.
        Ok(global_agent_or_init::<A>().on_attach(context))
    }));

    match outcome {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => error,
        Err(_) => {
            if let Some(agent) = GLOBAL_AGENT.get() {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    agent.callback_panicked("Agent_OnAttach")
                }));
            }
            jni::JNI_ERR
        }
    }
}

#[doc(hidden)]
pub unsafe fn __agent_on_unload(vm: *mut jni::JavaVM) {
    let Some(agent) = GLOBAL_AGENT.get() else {
        return;
    };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(context) = unsafe { agent::AgentUnloadContext::from_raw(vm) } {
            agent.on_unload(context);
        }
    }));
    if outcome.is_err() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            agent.callback_panicked("Agent_OnUnload")
        }));
    }
}

fn dispatch_agent(event: &'static str, callback: impl FnOnce(&dyn Agent)) {
    let Some(agent) = GLOBAL_AGENT.get() else {
        return;
    };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(agent.as_ref()))).is_err()
    {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            agent.callback_panicked(event)
        }));
    }
}

macro_rules! dispatch_event {
    ($name:literal, $jvmti:expr, $jni:expr, |$agent:ident, $context:ident| $body:block) => {{
        dispatch_agent($name, |$agent| {
            if let Some($context) = unsafe {
                callbacks::CallbackContext::from_raw($jvmti, $jni)
            } $body
        });
    }};
}

unsafe extern "system" fn trampoline_method_entry(
    jvmti_env: *mut jvmti::jvmtiEnv,
    jni_env: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
) {
    dispatch_event!("MethodEntry", jvmti_env, jni_env, |agent, context| {
        agent.method_entry(context, callbacks::MethodEvent::new(thread, method));
    });
}

unsafe extern "system" fn trampoline_method_exit(
    jvmti_env: *mut jvmti::jvmtiEnv,
    jni_env: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
    was_popped: jni::jboolean,
    return_value: jni::jvalue,
) {
    dispatch_event!("MethodExit", jvmti_env, jni_env, |agent, context| {
        agent.method_exit(
            context,
            callbacks::MethodExitEvent::new(thread, method, was_popped, return_value),
        );
    });
}

unsafe extern "system" fn trampoline_native_method_bind(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
    address: *mut std::ffi::c_void,
    new_address_ptr: *mut *mut std::ffi::c_void,
) {
    // `new_address_ptr` is an in/out value initialized by the VM. Leaving it
    // unchanged preserves the selected native implementation. If user code
    // mutates it and then panics, restore that VM-provided value before the
    // contained panic returns control to the VM.
    let original_address = if new_address_ptr.is_null() {
        None
    } else {
        Some(unsafe { *new_address_ptr })
    };
    let mut callback_completed = false;
    dispatch_event!("NativeMethodBind", env, jni, |agent, context| {
        agent.native_method_bind(
            context,
            callbacks::NativeMethodBindEvent::new(thread, method, address, new_address_ptr),
        );
        callback_completed = true;
    });
    if !callback_completed {
        if let Some(original_address) = original_address {
            unsafe { *new_address_ptr = original_address };
        }
    }
}

unsafe extern "system" fn trampoline_vm_init(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
) {
    dispatch_event!("VMInit", env, jni, |agent, context| {
        agent.vm_init(context, callbacks::ThreadEvent::new(thread));
    });
}

unsafe extern "system" fn trampoline_vm_death(env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv) {
    dispatch_event!("VMDeath", env, jni, |agent, context| {
        agent.vm_death(context);
    });
}

unsafe extern "system" fn trampoline_vm_start(env: *mut jvmti::jvmtiEnv, jni: *mut jni::JNIEnv) {
    dispatch_event!("VMStart", env, jni, |agent, context| {
        agent.vm_start(context);
    });
}

unsafe extern "system" fn trampoline_thread_start(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
) {
    dispatch_event!("ThreadStart", env, jni, |agent, context| {
        agent.thread_start(context, callbacks::ThreadEvent::new(thread));
    });
}

unsafe extern "system" fn trampoline_thread_end(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
) {
    dispatch_event!("ThreadEnd", env, jni, |agent, context| {
        agent.thread_end(context, callbacks::ThreadEvent::new(thread));
    });
}

unsafe extern "system" fn trampoline_virtual_thread_start(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
) {
    dispatch_event!("VirtualThreadStart", env, jni, |agent, context| {
        agent.virtual_thread_start(context, callbacks::ThreadEvent::new(thread));
    });
}

unsafe extern "system" fn trampoline_virtual_thread_end(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
) {
    dispatch_event!("VirtualThreadEnd", env, jni, |agent, context| {
        agent.virtual_thread_end(context, callbacks::ThreadEvent::new(thread));
    });
}

unsafe extern "system" fn trampoline_class_load(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    class: jni::jclass,
) {
    dispatch_event!("ClassLoad", env, jni, |agent, context| {
        agent.class_load(context, callbacks::ClassEvent::new(thread, class));
    });
}

unsafe extern "system" fn trampoline_class_prepare(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    class: jni::jclass,
) {
    dispatch_event!("ClassPrepare", env, jni, |agent, context| {
        agent.class_prepare(context, callbacks::ClassEvent::new(thread, class));
    });
}

unsafe extern "system" fn trampoline_class_file_load_hook(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    class_being_redefined: jni::jclass,
    loader: jni::jobject,
    name: *const std::ffi::c_char,
    protection_domain: jni::jobject,
    class_data_len: jni::jint,
    class_data: *const u8,
    new_class_data_len: *mut jni::jint,
    new_class_data: *mut *mut u8,
) {
    if !new_class_data_len.is_null() {
        unsafe { *new_class_data_len = 0 };
    }
    if !new_class_data.is_null() {
        unsafe { *new_class_data = std::ptr::null_mut() };
    }
    let mut callback_completed = false;
    dispatch_event!("ClassFileLoadHook", env, jni, |agent, context| {
        agent.class_file_load_hook(
            context,
            callbacks::ClassFileLoadHookEvent::new(
                class_being_redefined,
                loader,
                name,
                protection_domain,
                class_data_len,
                class_data,
                new_class_data_len,
                new_class_data,
            ),
        );
        callback_completed = true;
    });
    if !callback_completed && !new_class_data.is_null() {
        let transformed = unsafe { *new_class_data };
        if !transformed.is_null() && !env.is_null() {
            // `set_transformed_class` transfers this allocation to the VM only
            // after a successful callback. A contained panic rolls the pending
            // output back and releases it through the originating environment.
            let rollback_env = unsafe { env::Jvmti::from_raw(env) };
            let _ = unsafe { rollback_env.deallocate_raw(transformed) };
        }
        unsafe { *new_class_data = std::ptr::null_mut() };
        if !new_class_data_len.is_null() {
            unsafe { *new_class_data_len = 0 };
        }
    }
}

unsafe extern "system" fn trampoline_compiled_method_load(
    env: *mut jvmti::jvmtiEnv,
    method: jni::jmethodID,
    code_size: jni::jint,
    code_addr: *const std::ffi::c_void,
    map_length: jni::jint,
    map: *const jvmti::jvmtiAddrLocationMap,
    compile_info: *const std::ffi::c_void,
) {
    dispatch_event!(
        "CompiledMethodLoad",
        env,
        std::ptr::null_mut(),
        |agent, context| {
            agent.compiled_method_load(
                context,
                callbacks::CompiledMethodLoadEvent::new(
                    method,
                    code_size,
                    code_addr,
                    map_length,
                    map,
                    compile_info,
                ),
            );
        }
    );
}

unsafe extern "system" fn trampoline_compiled_method_unload(
    env: *mut jvmti::jvmtiEnv,
    method: jni::jmethodID,
    code_addr: *const std::ffi::c_void,
) {
    dispatch_event!(
        "CompiledMethodUnload",
        env,
        std::ptr::null_mut(),
        |agent, context| {
            agent.compiled_method_unload(
                context,
                callbacks::CompiledMethodUnloadEvent::new(method, code_addr),
            );
        }
    );
}

unsafe extern "system" fn trampoline_dynamic_code_generated(
    env: *mut jvmti::jvmtiEnv,
    name: *const std::ffi::c_char,
    address: *const std::ffi::c_void,
    length: jni::jint,
) {
    dispatch_event!(
        "DynamicCodeGenerated",
        env,
        std::ptr::null_mut(),
        |agent, context| {
            agent.dynamic_code_generated(
                context,
                callbacks::DynamicCodeGeneratedEvent::new(name, address, length),
            );
        }
    );
}

unsafe extern "system" fn trampoline_data_dump_request(env: *mut jvmti::jvmtiEnv) {
    dispatch_event!(
        "DataDumpRequest",
        env,
        std::ptr::null_mut(),
        |agent, context| {
            agent.data_dump_request(context);
        }
    );
}

unsafe extern "system" fn trampoline_exception(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
    location: jvmti::jlocation,
    exception: jni::jobject,
    catch_method: jni::jmethodID,
    catch_location: jvmti::jlocation,
) {
    dispatch_event!("Exception", env, jni, |agent, context| {
        agent.exception(
            context,
            callbacks::ExceptionEvent::new(
                thread,
                method,
                location,
                exception,
                catch_method,
                catch_location,
            ),
        );
    });
}

unsafe extern "system" fn trampoline_exception_catch(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
    location: jvmti::jlocation,
    exception: jni::jobject,
) {
    dispatch_event!("ExceptionCatch", env, jni, |agent, context| {
        agent.exception_catch(
            context,
            callbacks::ExceptionCatchEvent::new(thread, method, location, exception),
        );
    });
}

unsafe extern "system" fn trampoline_single_step(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
    location: jvmti::jlocation,
) {
    dispatch_event!("SingleStep", env, jni, |agent, context| {
        agent.single_step(
            context,
            callbacks::LocationEvent::new(thread, method, location),
        );
    });
}

unsafe extern "system" fn trampoline_breakpoint(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
    location: jvmti::jlocation,
) {
    dispatch_event!("Breakpoint", env, jni, |agent, context| {
        agent.breakpoint(
            context,
            callbacks::LocationEvent::new(thread, method, location),
        );
    });
}

unsafe extern "system" fn trampoline_frame_pop(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
    was_popped: jni::jboolean,
) {
    dispatch_event!("FramePop", env, jni, |agent, context| {
        agent.frame_pop(
            context,
            callbacks::FramePopEvent::new(thread, method, was_popped),
        );
    });
}

unsafe extern "system" fn trampoline_monitor_wait(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    object: jni::jobject,
    timeout: jni::jlong,
) {
    dispatch_event!("MonitorWait", env, jni, |agent, context| {
        agent.monitor_wait(
            context,
            callbacks::MonitorWaitEvent::new(thread, object, timeout),
        );
    });
}

unsafe extern "system" fn trampoline_monitor_waited(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    object: jni::jobject,
    timed_out: jni::jboolean,
) {
    dispatch_event!("MonitorWaited", env, jni, |agent, context| {
        agent.monitor_waited(
            context,
            callbacks::MonitorWaitedEvent::new(thread, object, timed_out),
        );
    });
}

unsafe extern "system" fn trampoline_monitor_contended_enter(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    object: jni::jobject,
) {
    dispatch_event!("MonitorContendedEnter", env, jni, |agent, context| {
        agent.monitor_contended_enter(context, callbacks::MonitorEvent::new(thread, object));
    });
}

unsafe extern "system" fn trampoline_monitor_contended_entered(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    object: jni::jobject,
) {
    dispatch_event!("MonitorContendedEntered", env, jni, |agent, context| {
        agent.monitor_contended_entered(context, callbacks::MonitorEvent::new(thread, object));
    });
}

unsafe extern "system" fn trampoline_field_access(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
    location: jvmti::jlocation,
    field_class: jni::jclass,
    object: jni::jobject,
    field: jni::jfieldID,
) {
    dispatch_event!("FieldAccess", env, jni, |agent, context| {
        agent.field_access(
            context,
            callbacks::FieldAccessEvent::new(thread, method, location, field_class, object, field),
        );
    });
}

unsafe extern "system" fn trampoline_field_modification(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    method: jni::jmethodID,
    location: jvmti::jlocation,
    field_class: jni::jclass,
    object: jni::jobject,
    field: jni::jfieldID,
    signature_type: std::ffi::c_char,
    new_value: jni::jvalue,
) {
    dispatch_event!("FieldModification", env, jni, |agent, context| {
        agent.field_modification(
            context,
            callbacks::FieldModificationEvent::new(
                thread,
                method,
                location,
                field_class,
                object,
                field,
                signature_type,
                new_value,
            ),
        );
    });
}

unsafe extern "system" fn trampoline_garbage_collection_start(env: *mut jvmti::jvmtiEnv) {
    dispatch_event!(
        "GarbageCollectionStart",
        env,
        std::ptr::null_mut(),
        |agent, context| {
            agent.garbage_collection_start(context);
        }
    );
}

unsafe extern "system" fn trampoline_garbage_collection_finish(env: *mut jvmti::jvmtiEnv) {
    dispatch_event!(
        "GarbageCollectionFinish",
        env,
        std::ptr::null_mut(),
        |agent, context| {
            agent.garbage_collection_finish(context);
        }
    );
}

unsafe extern "system" fn trampoline_resource_exhausted(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    flags: jni::jint,
    reserved: *const std::ffi::c_void,
    description: *const std::ffi::c_char,
) {
    dispatch_event!("ResourceExhausted", env, jni, |agent, context| {
        agent.resource_exhausted(
            context,
            callbacks::ResourceExhaustedEvent::new(flags, reserved, description),
        );
    });
}

unsafe extern "system" fn trampoline_object_free(env: *mut jvmti::jvmtiEnv, tag: jni::jlong) {
    dispatch_event!("ObjectFree", env, std::ptr::null_mut(), |agent, context| {
        agent.object_free(context, callbacks::ObjectFreeEvent::new(tag));
    });
}

unsafe extern "system" fn trampoline_vm_object_alloc(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    object: jni::jobject,
    class: jni::jclass,
    size: jni::jlong,
) {
    dispatch_event!("VMObjectAlloc", env, jni, |agent, context| {
        agent.vm_object_alloc(
            context,
            callbacks::ObjectAllocationEvent::new(thread, object, class, size),
        );
    });
}

unsafe extern "system" fn trampoline_sampled_object_alloc(
    env: *mut jvmti::jvmtiEnv,
    jni: *mut jni::JNIEnv,
    thread: jni::jthread,
    object: jni::jobject,
    class: jni::jclass,
    size: jni::jlong,
) {
    dispatch_event!("SampledObjectAlloc", env, jni, |agent, context| {
        agent.sampled_object_alloc(
            context,
            callbacks::ObjectAllocationEvent::new(thread, object, class, size),
        );
    });
}

/// Returns a pre-configured `jvmtiEventCallbacks` struct with all event trampolines wired up.
///
/// This function populates a callbacks struct that routes all JVMTI events to your
/// [`Agent`] implementation via the global agent instance. Use this with
/// [`env::Jvmti::set_event_callbacks`] to enable event delivery.
///
/// # Example
///
/// ```rust,no_run
/// use jvmti_bindings::prelude::*;
///
/// #[derive(Default)]
/// struct LifecycleAgent;
///
/// impl Agent for LifecycleAgent {
/// fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
///     let Ok(jvmti) = context.vm().jvmti() else {
///         return jni::JNI_ERR;
///     };
///
///     // Wire up all event callbacks
///     let callbacks = get_default_callbacks();
///     if jvmti.set_event_callbacks(callbacks).is_err() {
///         return jni::JNI_ERR;
///     }
///
///     // Enable specific events you care about
///     if unsafe { jvmti.set_event_notification_mode(
///         true,
///         jvmti::JVMTI_EVENT_VM_INIT,
///         std::ptr::null_mut(),
///     ) }.is_err() {
///         return jni::JNI_ERR;
///     }
///
///     jni::JNI_OK
/// }
/// }
/// ```
///
/// # Events Wired
///
/// All standard JVMTI events are wired:
/// - VM lifecycle: `VMInit`, `VMDeath`, `VMStart`
/// - Threads: `ThreadStart`, `ThreadEnd`, `VirtualThreadStart`, `VirtualThreadEnd`
/// - Classes: `ClassLoad`, `ClassPrepare`, `ClassFileLoadHook`
/// - Methods: `MethodEntry`, `MethodExit`, `NativeMethodBind`
/// - Compilation: `CompiledMethodLoad`, `CompiledMethodUnload`, `DynamicCodeGenerated`, `DataDumpRequest`
/// - Exceptions: `Exception`, `ExceptionCatch`
/// - Debugging: `SingleStep`, `Breakpoint`, `FramePop`
/// - Monitors: `MonitorWait`, `MonitorWaited`, `MonitorContendedEnter`, `MonitorContendedEntered`
/// - Fields: `FieldAccess`, `FieldModification`
/// - GC: `GarbageCollectionStart`, `GarbageCollectionFinish`, `ResourceExhausted`
/// - Objects: `ObjectFree`, `VMObjectAlloc`, `SampledObjectAlloc`
pub fn get_default_callbacks() -> jvmti::jvmtiEventCallbacks {
    jvmti::jvmtiEventCallbacks {
        VMInit: Some(trampoline_vm_init),
        VMDeath: Some(trampoline_vm_death),
        ThreadStart: Some(trampoline_thread_start),
        ThreadEnd: Some(trampoline_thread_end),
        ClassFileLoadHook: Some(trampoline_class_file_load_hook),
        ClassLoad: Some(trampoline_class_load),
        ClassPrepare: Some(trampoline_class_prepare),
        VMStart: Some(trampoline_vm_start),
        Exception: Some(trampoline_exception),
        ExceptionCatch: Some(trampoline_exception_catch),
        SingleStep: Some(trampoline_single_step),
        FramePop: Some(trampoline_frame_pop),
        Breakpoint: Some(trampoline_breakpoint),
        FieldAccess: Some(trampoline_field_access),
        FieldModification: Some(trampoline_field_modification),
        MethodEntry: Some(trampoline_method_entry),
        MethodExit: Some(trampoline_method_exit),
        NativeMethodBind: Some(trampoline_native_method_bind),
        CompiledMethodLoad: Some(trampoline_compiled_method_load),
        CompiledMethodUnload: Some(trampoline_compiled_method_unload),
        DynamicCodeGenerated: Some(trampoline_dynamic_code_generated),
        DataDumpRequest: Some(trampoline_data_dump_request),
        MonitorWait: Some(trampoline_monitor_wait),
        MonitorWaited: Some(trampoline_monitor_waited),
        MonitorContendedEnter: Some(trampoline_monitor_contended_enter),
        MonitorContendedEntered: Some(trampoline_monitor_contended_entered),
        ResourceExhausted: Some(trampoline_resource_exhausted),
        GarbageCollectionStart: Some(trampoline_garbage_collection_start),
        GarbageCollectionFinish: Some(trampoline_garbage_collection_finish),
        ObjectFree: Some(trampoline_object_free),
        VMObjectAlloc: Some(trampoline_vm_object_alloc),
        SampledObjectAlloc: Some(trampoline_sampled_object_alloc),
        VirtualThreadStart: Some(trampoline_virtual_thread_start),
        VirtualThreadEnd: Some(trampoline_virtual_thread_end),
        ..Default::default()
    }
}

/// Exports your agent type as a loadable JVMTI agent library.
///
/// This macro generates the required `Agent_OnLoad`, `Agent_OnAttach`, and
/// `Agent_OnUnload` FFI entry points used for startup and dynamic agent loading.
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
/// The macro generates three `extern "system"` functions:
///
/// - **`Agent_OnLoad`**: Called by the JVM when the agent is loaded. Creates your agent
///   instance, registers it globally, and calls your [`Agent::on_load`] method.
///
/// - **`Agent_OnAttach`**: Called for each successful dynamic attach request. Reuses the
///   process-global agent and calls [`Agent::on_attach`] with that request's exact context.
///
/// - **`Agent_OnUnload`**: Called by the JVM during shutdown. Calls your [`Agent::on_unload`]
///   method for cleanup.
///
/// # Example
///
/// ```rust,no_run
/// use jvmti_bindings::prelude::*;
///
/// #[derive(Default)]
/// struct MyAgent {
///     // Your agent state here
/// }
///
/// impl Agent for MyAgent {
///     fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
///         println!("Agent loaded with options: {:?}", context.options_lossy());
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
        #[unsafe(no_mangle)]
        pub unsafe extern "system" fn Agent_OnLoad(
            vm: *mut $crate::sys::jni::JavaVM,
            options: *mut std::ffi::c_char,
            reserved: *mut std::ffi::c_void,
        ) -> $crate::sys::jni::jint {
            unsafe { $crate::__agent_on_load::<$agent_type>(vm, options, reserved) }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "system" fn Agent_OnAttach(
            vm: *mut $crate::sys::jni::JavaVM,
            options: *mut std::ffi::c_char,
            reserved: *mut std::ffi::c_void,
        ) -> $crate::sys::jni::jint {
            unsafe { $crate::__agent_on_attach::<$agent_type>(vm, options, reserved) }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "system" fn Agent_OnUnload(vm: *mut $crate::sys::jni::JavaVM) {
            unsafe { $crate::__agent_on_unload(vm) }
        }
    };
}
