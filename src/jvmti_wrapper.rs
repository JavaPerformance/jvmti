// vliss/jvmti/src/wrapper.rs
use crate::agent::JavaVmRef;
use crate::mutf8;
use crate::sys::jni;
use crate::sys::jvmti;
use crate::version::{JvmtiFeature, jvmti_interface_feature, release_profile};
use std::ffi::CStr;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ptr;

// JVM TI function tables grow across JDK releases. Read only the selected
// pointer-sized slot so an environment from an older JVM is never viewed as a
// reference to the larger latest-version Rust structure.
macro_rules! jvmti_function {
    ($env:expr, $field:ident) => {{
        let table = $env.function_table_ptr()?;
        // This macro is expanded in both ordinary and already-unsafe blocks.
        #[allow(unused_unsafe)]
        let slot = unsafe { &raw const (*table).$field };
        $env.read_function_slot(slot)
    }};
}

/// Thread metadata returned by JVM TI.
///
/// `thread_group` and `context_class_loader` are JNI local references. Delete
/// them explicitly or bound them with a JNI local frame on long-running agent
/// threads.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub name: Option<String>,
    pub priority: jni::jint,
    pub is_daemon: bool,
    pub thread_group: jni::jobject,
    pub context_class_loader: jni::jobject,
}

/// Thread-group metadata returned by JVM TI.
///
/// `parent` is a JNI local reference and requires the same lifecycle handling
/// as any other local reference returned by JVM TI.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ThreadGroupInfo {
    pub parent: jni::jobject,
    pub name: Option<String>,
    pub max_priority: jni::jint,
    pub is_daemon: bool,
}

/// Monitor metadata returned by JVM TI.
///
/// `owner`, `waiters`, and `notify_waiters` contain JNI local references.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct MonitorUsage {
    pub owner: jni::jthread,
    pub entry_count: jni::jint,
    pub waiters: Vec<jni::jthread>,
    pub notify_waiters: Vec<jni::jthread>,
}

/// Stack metadata returned by JVM TI.
///
/// `thread` is a JNI local reference. The frame records contain method IDs,
/// which are not JNI object references.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct StackInfo {
    pub thread: jni::jthread,
    pub state: jni::jint,
    pub frames: Vec<jvmti::jvmtiFrameInfo>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ExtensionParamInfo {
    pub name: Option<String>,
    pub kind: jvmti::jvmtiParamKind,
    pub base_type: jvmti::jvmtiParamTypes,
    pub null_ok: bool,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ExtensionFunctionInfo {
    pub func: Option<jvmti::jvmtiExtensionFunction>,
    pub id: Option<String>,
    pub short_description: Option<String>,
    pub params: Vec<ExtensionParamInfo>,
    pub errors: Vec<jvmti::jvmtiError>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ExtensionEventInfo {
    pub extension_event_index: jni::jint,
    pub id: Option<String>,
    pub short_description: Option<String>,
    pub params: Vec<ExtensionParamInfo>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct LocalVariableEntry {
    pub start_location: jvmti::jlocation,
    pub length: jni::jint,
    pub name: Option<String>,
    pub signature: Option<String>,
    pub generic_signature: Option<String>,
    pub slot: jni::jint,
}

fn ptr_in_range(ptr: *const u8, base: *const u8, len: usize) -> bool {
    if ptr.is_null() || base.is_null() || len == 0 {
        return false;
    }
    let address = ptr.addr();
    let start = base.addr();
    let Some(end) = start.checked_add(len) else {
        return false;
    };
    (start..end).contains(&address)
}

fn jvmti_array_to_vec<T: Copy>(ptr: *mut T, count: jni::jint) -> Result<Vec<T>, jvmti::jvmtiError> {
    let count = usize_count(count)?;
    if count == 0 {
        return Ok(Vec::new());
    }
    if ptr.is_null() {
        return Err(jvmti::jvmtiError::NULL_POINTER);
    }
    Ok(unsafe { std::slice::from_raw_parts(ptr, count).to_vec() })
}

fn with_jvmti_array<T, R>(
    ptr: *mut T,
    count: jni::jint,
    use_slice: impl FnOnce(&[T]) -> Result<R, jvmti::jvmtiError>,
) -> Result<R, jvmti::jvmtiError> {
    let count = usize_count(count)?;
    if count == 0 {
        return use_slice(&[]);
    }
    if ptr.is_null() {
        return Err(jvmti::jvmtiError::NULL_POINTER);
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, count) };
    use_slice(slice)
}

fn jint_len(len: usize) -> Result<jni::jint, jvmti::jvmtiError> {
    jni::jint::try_from(len).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)
}

fn usize_count(count: jni::jint) -> Result<usize, jvmti::jvmtiError> {
    usize::try_from(count).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)
}

struct JvmtiDeallocationGuard<'env, T> {
    env: &'env Jvmti,
    ptr: *mut T,
}

impl<'env, T> JvmtiDeallocationGuard<'env, T> {
    fn new(env: &'env Jvmti, ptr: *mut T) -> Self {
        Self { env, ptr }
    }

    fn as_slice(&self, count: jni::jint) -> Result<&[T], jvmti::jvmtiError> {
        let count = usize_count(count)?;
        if count == 0 {
            return Ok(&[]);
        }
        if self.ptr.is_null() {
            return Err(jvmti::jvmtiError::NULL_POINTER);
        }
        Ok(unsafe { std::slice::from_raw_parts(self.ptr, count) })
    }
}

impl<T: Copy> JvmtiDeallocationGuard<'_, T> {
    fn to_vec(&self, count: jni::jint) -> Result<Vec<T>, jvmti::jvmtiError> {
        Ok(self.as_slice(count)?.to_vec())
    }
}

impl JvmtiDeallocationGuard<'_, std::os::raw::c_char> {
    fn to_string(&self) -> Result<String, jvmti::jvmtiError> {
        if self.ptr.is_null() {
            return Err(jvmti::jvmtiError::NULL_POINTER);
        }
        mutf8::decode_cstr(unsafe { CStr::from_ptr(self.ptr) })
            .map_err(|_| jvmti::jvmtiError::INTERNAL)
    }

    fn to_optional_string(&self) -> Result<Option<String>, jvmti::jvmtiError> {
        cstr_to_string(self.ptr)
    }
}

impl<T> Drop for JvmtiDeallocationGuard<'_, T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            let _ = self.env.deallocate_owned(self.ptr.cast());
        }
    }
}

fn owned_jvmti_array_to_vec<T: Copy>(
    env: &Jvmti,
    ptr: *mut T,
    count: jni::jint,
) -> Result<Vec<T>, jvmti::jvmtiError> {
    let allocation = JvmtiDeallocationGuard::new(env, ptr);
    allocation.to_vec(count)
}

fn owned_jvmti_string(
    env: &Jvmti,
    ptr: *mut std::os::raw::c_char,
) -> Result<String, jvmti::jvmtiError> {
    let allocation = JvmtiDeallocationGuard::new(env, ptr);
    allocation.to_string()
}

fn owned_optional_jvmti_string(
    env: &Jvmti,
    ptr: *mut std::os::raw::c_char,
) -> Result<Option<String>, jvmti::jvmtiError> {
    let allocation = JvmtiDeallocationGuard::new(env, ptr);
    allocation.to_optional_string()
}

fn cstr_to_string(ptr: *const std::os::raw::c_char) -> Result<Option<String>, jvmti::jvmtiError> {
    if ptr.is_null() {
        return Ok(None);
    }
    mutf8::decode_cstr(unsafe { CStr::from_ptr(ptr) })
        .map(Some)
        .map_err(|_| jvmti::jvmtiError::INTERNAL)
}

/// Memory owned by a JVM TI environment and released with `Deallocate`.
///
/// The allocation cannot outlive the environment wrapper that created it.
/// Ownership may be transferred to the JVM explicitly with [`Self::into_raw`].
pub struct JvmtiAllocation<'env, T = u8> {
    env: &'env Jvmti,
    ptr: *mut T,
    byte_len: usize,
    _not_send_sync: PhantomData<*mut T>,
}

impl<'env, T> JvmtiAllocation<'env, T> {
    fn from_raw(env: &'env Jvmti, ptr: *mut T, byte_len: usize) -> Self {
        Self {
            env,
            ptr,
            byte_len,
            _not_send_sync: PhantomData,
        }
    }

    /// Returns the owned pointer. It may be null only for a zero-byte allocation.
    pub fn as_ptr(&self) -> *const T {
        self.ptr
    }

    /// Returns the owned mutable pointer. It may be null only for a zero-byte allocation.
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.ptr
    }

    /// Returns the allocation size in bytes.
    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Transfers ownership to the caller without invoking `Deallocate`.
    ///
    /// # Safety
    ///
    /// The returned pointer must eventually be transferred to an API that takes
    /// ownership or passed to [`Jvmti::deallocate_raw`] on the same environment.
    pub unsafe fn into_raw(self) -> *mut T {
        let this = ManuallyDrop::new(self);
        this.ptr
    }
}

impl JvmtiAllocation<'_, u8> {
    pub fn as_slice(&self) -> &[u8] {
        if self.byte_len == 0 {
            &[]
        } else {
            debug_assert!(!self.ptr.is_null());
            unsafe { std::slice::from_raw_parts(self.ptr, self.byte_len) }
        }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        if self.byte_len == 0 {
            &mut []
        } else {
            debug_assert!(!self.ptr.is_null());
            unsafe { std::slice::from_raw_parts_mut(self.ptr, self.byte_len) }
        }
    }
}

impl<T> Drop for JvmtiAllocation<'_, T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // Drop cannot report a JVM TI error. Explicit callers that need the
            // result can transfer the pointer and call deallocate_raw themselves.
            let _ = self.env.deallocate_owned(self.ptr.cast());
        }
    }
}

/// JVM-owned copy of the active JNI function table.
///
/// This type deliberately does not implement `Deref`: function-table lengths
/// differ between JDK releases, so treating every copy as the latest Rust
/// structure would make older tables unsound to read.
pub struct JniFunctionTable<'env> {
    env: &'env Jvmti,
    ptr: *mut jni::JNINativeInterface_,
    jvmti_interface_feature: u16,
    known_byte_len: Option<usize>,
}

/// Owning JVM TI raw-monitor handle.
///
/// The monitor is destroyed on drop. Use [`Self::close`] to observe the
/// destroy result or [`Self::into_raw`] to transfer ownership deliberately.
pub struct RawMonitor<'env> {
    env: &'env Jvmti,
    monitor: jvmti::jrawMonitorID,
}

impl<'env> RawMonitor<'env> {
    pub fn as_raw(&self) -> jvmti::jrawMonitorID {
        self.monitor
    }

    /// Enter the monitor and return a guard that exits it on drop.
    pub fn enter<'monitor>(
        &'monitor self,
    ) -> Result<RawMonitorGuard<'monitor, 'env>, jvmti::jvmtiError> {
        unsafe { self.env.raw_monitor_enter(self.monitor)? };
        Ok(RawMonitorGuard {
            monitor: self,
            active: true,
        })
    }

    /// Destroy the monitor exactly once and return the JVM TI result.
    pub fn close(mut self) -> Result<(), jvmti::jvmtiError> {
        let result = unsafe { self.env.destroy_raw_monitor(self.monitor) };
        if result.is_ok() {
            self.monitor = ptr::null_mut();
        }
        result
    }

    /// Transfer ownership of the raw monitor to the caller.
    ///
    /// # Safety
    ///
    /// The caller must eventually destroy the monitor with the same live JVM
    /// TI environment and must not use it after destruction.
    pub unsafe fn into_raw(self) -> jvmti::jrawMonitorID {
        let this = ManuallyDrop::new(self);
        this.monitor
    }
}

impl Drop for RawMonitor<'_> {
    fn drop(&mut self) {
        if !self.monitor.is_null() {
            let monitor = std::mem::replace(&mut self.monitor, ptr::null_mut());
            let _ = unsafe { self.env.destroy_raw_monitor(monitor) };
        }
    }
}

/// Entered JVM TI raw monitor, exited automatically on drop.
pub struct RawMonitorGuard<'monitor, 'env> {
    monitor: &'monitor RawMonitor<'env>,
    active: bool,
}

impl RawMonitorGuard<'_, '_> {
    pub fn wait(&self, millis: jni::jlong) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            self.monitor
                .env
                .raw_monitor_wait(self.monitor.monitor, millis)
        }
    }

    pub fn notify(&self) -> Result<(), jvmti::jvmtiError> {
        unsafe { self.monitor.env.raw_monitor_notify(self.monitor.monitor) }
    }

    pub fn notify_all(&self) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            self.monitor
                .env
                .raw_monitor_notify_all(self.monitor.monitor)
        }
    }

    /// Exit immediately instead of waiting for drop.
    pub fn exit(mut self) -> Result<(), jvmti::jvmtiError> {
        let result = unsafe { self.monitor.env.raw_monitor_exit(self.monitor.monitor) };
        if result.is_ok() {
            self.active = false;
        }
        result
    }
}

impl Drop for RawMonitorGuard<'_, '_> {
    fn drop(&mut self) {
        if self.active {
            self.active = false;
            let _ = unsafe { self.monitor.env.raw_monitor_exit(self.monitor.monitor) };
        }
    }
}

impl JniFunctionTable<'_> {
    /// Opaque table pointer suitable for the unsafe installation API.
    pub fn as_ptr(&self) -> *const jni::JNINativeInterface_ {
        self.ptr
    }

    /// Feature milestone reported by the JVM TI environment.
    ///
    /// This is not necessarily the exact Java release; for example, JDK 10
    /// reports the JDK 9 JVM TI interface revision.
    pub fn jvmti_interface_feature(&self) -> u16 {
        self.jvmti_interface_feature
    }

    /// Exact allocation prefix size for audited JDK 8-28 releases.
    ///
    /// Future runtimes return `None` until their headers have passed the ABI
    /// matrix; the pointer remains owned and is still deallocated correctly.
    pub fn known_byte_len(&self) -> Option<usize> {
        self.known_byte_len
    }

    /// Transfer ownership to the caller.
    ///
    /// # Safety
    ///
    /// The pointer must eventually be passed to `Jvmti::deallocate_raw` on the
    /// same live JVM TI environment.
    pub unsafe fn into_raw(self) -> *mut jni::JNINativeInterface_ {
        let this = ManuallyDrop::new(self);
        this.ptr
    }
}

impl Drop for JniFunctionTable<'_> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            let _ = self.env.deallocate_owned(self.ptr.cast());
        }
    }
}

/// A safe wrapper around the raw JVMTI Environment pointer.
///
/// # JNI local references
///
/// JVM TI functions return `jobject`, `jclass`, `jthread`, and `jthreadGroup`
/// values as JNI local references. Methods on this wrapper preserve those raw
/// handles in their returned values; they cannot delete them without also
/// borrowing the current thread's [`crate::env::JniEnv`]. Event callbacks that
/// promptly return normally get automatic local-reference cleanup. Long-lived
/// agent threads must use `LocalRef`, `PushLocalFrame`/`PopLocalFrame`, or the
/// equivalent raw JNI lifecycle operations.
pub struct Jvmti {
    // We keep this private so the user can't mess with raw pointers directly.
    env: *mut jvmti::jvmtiEnv,
}

impl Jvmti {
    fn function_table_ptr(&self) -> Result<*const jvmti::jvmtiInterface_1_, jvmti::jvmtiError> {
        if self.env.is_null() {
            return Err(jvmti::jvmtiError::INVALID_ENVIRONMENT);
        }
        let functions = unsafe { (*self.env).functions };
        if functions.is_null() {
            return Err(jvmti::jvmtiError::INVALID_ENVIRONMENT);
        }
        Ok(functions)
    }

    fn read_function_slot<F: Copy>(&self, slot: *const Option<F>) -> Result<F, jvmti::jvmtiError> {
        // `slot` is produced only by `jvmti_function!` from the checked table
        // pointer. Reading one slot avoids constructing a reference to a
        // latest-version table when an older JVM owns a shorter prefix.
        unsafe { slot.read() }.ok_or(jvmti::jvmtiError::NOT_AVAILABLE)
    }

    /// Returns whether the active JVM TI environment supports an additive
    /// platform feature.
    pub fn supports_feature(&self, feature: JvmtiFeature) -> Result<bool, jvmti::jvmtiError> {
        let actual = self.get_version_number()?;
        Ok(jvmti_interface_feature(actual) >= feature.required_feature())
    }

    fn require_feature(&self, feature: JvmtiFeature) -> Result<(), jvmti::jvmtiError> {
        if self.supports_feature(feature)? {
            Ok(())
        } else {
            Err(jvmti::jvmtiError::NOT_AVAILABLE)
        }
    }

    fn require_event_type(&self, event_type: jvmti::jvmtiEvent) -> Result<(), jvmti::jvmtiError> {
        let feature = match event_type {
            jvmti::JVMTI_EVENT_SAMPLED_OBJECT_ALLOC => Some(JvmtiFeature::HeapSampling),
            jvmti::JVMTI_EVENT_VIRTUAL_THREAD_START | jvmti::JVMTI_EVENT_VIRTUAL_THREAD_END => {
                Some(JvmtiFeature::VirtualThreads)
            }
            _ => None,
        };
        match feature {
            Some(feature) => self.require_feature(feature),
            None => Ok(()),
        }
    }

    fn validate_versioned_capabilities(
        &self,
        capabilities: &jvmti::jvmtiCapabilities,
    ) -> Result<(), jvmti::jvmtiError> {
        let checks = [
            (
                capabilities.can_generate_early_vmstart()
                    || capabilities.can_generate_early_class_hook_events(),
                JvmtiFeature::Modules,
            ),
            (
                capabilities.can_generate_sampled_object_alloc_events(),
                JvmtiFeature::HeapSampling,
            ),
            (
                capabilities.can_support_virtual_threads(),
                JvmtiFeature::VirtualThreads,
            ),
            (
                capabilities.can_support_value_objects(),
                JvmtiFeature::ValueObjects,
            ),
        ];
        for (requested, feature) in checks {
            if requested {
                self.require_feature(feature)?;
            }
        }
        Ok(())
    }

    /// Connects to the JVM and retrieves the JVMTI environment.
    pub fn new(vm: &JavaVmRef<'_>) -> Result<Self, jni::jint> {
        unsafe { Self::from_java_vm_raw(vm.raw()) }
    }

    /// Connect to a JVM from a trusted raw `JavaVM*`.
    ///
    /// # Safety
    ///
    /// `vm` must point to a live JVM invocation table for the duration of this
    /// operation. Prefer [`Jvmti::new`] with a callback-scoped [`JavaVmRef`].
    pub unsafe fn from_java_vm_raw(vm: *mut jni::JavaVM) -> Result<Self, jni::jint> {
        if vm.is_null() {
            return Err(jni::JNI_ERR);
        }

        let mut env_ptr: *mut std::ffi::c_void = ptr::null_mut();

        unsafe {
            if (*vm).is_null() {
                return Err(jni::JNI_ERR);
            }

            // Access GetEnv directly from the vtable
            // vm: *mut JavaVM = *mut *const JNIInvokeInterface_
            // *vm: *const JNIInvokeInterface_ (vtable pointer)
            // **vm: JNIInvokeInterface_ (vtable itself)
            let get_env_fn = (**vm).GetEnv;

            let res = get_env_fn(vm, &mut env_ptr, jvmti::JVMTI_VERSION_1_2);

            if res != jni::JNI_OK {
                return Err(res);
            }
        }

        if env_ptr.is_null() {
            return Err(jni::JNI_ERR);
        }

        Ok(Jvmti {
            env: env_ptr as *mut jvmti::jvmtiEnv,
        })
    }

    /// Create a Jvmti wrapper from a raw jvmtiEnv pointer
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid for the duration of use.
    pub unsafe fn from_raw(env: *mut jvmti::jvmtiEnv) -> Self {
        Jvmti { env }
    }

    /// Get the raw jvmtiEnv pointer
    pub fn raw(&self) -> *mut jvmti::jvmtiEnv {
        self.env
    }

    pub fn get_capabilities(&self) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError> {
        let mut caps = jvmti::jvmtiCapabilities::default();

        unsafe {
            let get_caps_fn = jvmti_function!(self, GetCapabilities)?;
            let err = get_caps_fn(self.env, &mut caps);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        Ok(caps)
    }

    pub fn add_capabilities(
        &self,
        new_caps: &jvmti::jvmtiCapabilities,
    ) -> Result<(), jvmti::jvmtiError> {
        self.validate_versioned_capabilities(new_caps)?;
        unsafe {
            // Retrieve the function pointer only after validating the active
            // environment. Optional slots must fail closed rather than panic.
            let add_caps_fn = jvmti_function!(self, AddCapabilities)?;

            // 2. Call the C function
            let err = add_caps_fn(self.env, new_caps);

            // 3. Check for success
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Convenience helper to build and add capabilities in one step.
    pub fn add_capabilities_with<F>(
        &self,
        f: F,
    ) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError>
    where
        F: FnOnce(&mut jvmti::jvmtiCapabilities),
    {
        let mut caps = jvmti::jvmtiCapabilities::default();
        f(&mut caps);
        self.add_capabilities(&caps)?;
        Ok(caps)
    }

    /// Request the capabilities required for `ClassFileLoadHook`.
    pub fn add_class_file_load_hook_capabilities(
        &self,
    ) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError> {
        let caps = jvmti::jvmtiCapabilities::for_class_file_load_hook();
        self.add_capabilities(&caps)?;
        Ok(caps)
    }

    /// Request the capabilities required for method-entry and method-exit tracing.
    pub fn add_method_trace_capabilities(
        &self,
    ) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError> {
        let caps = jvmti::jvmtiCapabilities::for_method_trace();
        self.add_capabilities(&caps)?;
        Ok(caps)
    }

    /// Request the capabilities required for exception and exception-catch events.
    pub fn add_exception_capabilities(
        &self,
    ) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError> {
        let caps = jvmti::jvmtiCapabilities::for_exceptions();
        self.add_capabilities(&caps)?;
        Ok(caps)
    }

    /// Request the capabilities required for sampled-object-allocation events.
    pub fn add_heap_sampling_capabilities(
        &self,
    ) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::HeapSampling)?;
        let caps = jvmti::jvmtiCapabilities::for_heap_sampling();
        self.add_capabilities(&caps)?;
        Ok(caps)
    }

    /// Request JDK 28 preview value-object support when the runtime exposes it.
    pub fn add_value_object_capabilities(
        &self,
    ) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::ValueObjects)?;
        let potential = self.get_potential_capabilities()?;
        if !potential.can_support_value_objects() {
            return Err(jvmti::jvmtiError::NOT_AVAILABLE);
        }
        let caps = jvmti::jvmtiCapabilities::for_value_objects();
        self.add_capabilities(&caps)?;
        Ok(caps)
    }

    pub fn set_event_callbacks(
        &self,
        callbacks: jvmti::jvmtiEventCallbacks,
    ) -> Result<(), jvmti::jvmtiError> {
        let interface_feature = jvmti_interface_feature(self.get_version_number()?);
        let size = jvmti::event_callbacks_size_for_feature(interface_feature);
        unsafe {
            let set_callbacks_fn = jvmti_function!(self, SetEventCallbacks)?;

            let err = set_callbacks_fn(self.env, &callbacks, size);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Wire the default Rust agent trampolines from [`crate::get_default_callbacks`].
    pub fn set_default_agent_callbacks(&self) -> Result<(), jvmti::jvmtiError> {
        self.set_event_callbacks(crate::get_default_callbacks())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_event_notification_mode(
        &self,
        enable: bool,
        event_type: jvmti::jvmtiEvent,
        thread: jni::jthread,
    ) -> Result<(), jvmti::jvmtiError> {
        self.require_event_type(event_type)?;
        unsafe {
            let set_mode_fn = jvmti_function!(self, SetEventNotificationMode)?; // Index 1
            let mode = if enable { 1 } else { 0 }; // JVMTI_ENABLE = 1, DISABLE = 0

            // thread can be null (all threads)
            let err = set_mode_fn(self.env, mode, event_type, thread);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Enable a single JVMTI event for a specific thread (or all threads with null).
    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn enable_event(
        &self,
        event_type: jvmti::jvmtiEvent,
        thread: jni::jthread,
    ) -> Result<(), jvmti::jvmtiError> {
        // SAFETY: Forwarded from this function's handle contract.
        unsafe { self.set_event_notification_mode(true, event_type, thread) }
    }

    /// Disable a single JVMTI event for a specific thread (or all threads with null).
    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn disable_event(
        &self,
        event_type: jvmti::jvmtiEvent,
        thread: jni::jthread,
    ) -> Result<(), jvmti::jvmtiError> {
        // SAFETY: Forwarded from this function's handle contract.
        unsafe { self.set_event_notification_mode(false, event_type, thread) }
    }

    /// Enable multiple JVMTI events for all threads.
    pub fn enable_events_global(
        &self,
        events: &[jvmti::jvmtiEvent],
    ) -> Result<(), jvmti::jvmtiError> {
        for &event_type in events {
            // A null thread is the specification-defined global selector.
            unsafe { self.enable_event(event_type, ptr::null_mut())? };
        }
        Ok(())
    }

    /// Disable multiple JVMTI events for all threads.
    pub fn disable_events_global(
        &self,
        events: &[jvmti::jvmtiEvent],
    ) -> Result<(), jvmti::jvmtiError> {
        for &event_type in events {
            // A null thread is the specification-defined global selector.
            unsafe { self.disable_event(event_type, ptr::null_mut())? };
        }
        Ok(())
    }

    /// Enable `ClassFileLoadHook` for all threads.
    pub fn enable_class_file_load_hook_events(&self) -> Result<(), jvmti::jvmtiError> {
        self.enable_events_global(&[jvmti::JVMTI_EVENT_CLASS_FILE_LOAD_HOOK])
    }

    /// Enable method-entry and method-exit events for all threads.
    pub fn enable_method_entry_exit_events(&self) -> Result<(), jvmti::jvmtiError> {
        self.enable_events_global(&[
            jvmti::JVMTI_EVENT_METHOD_ENTRY,
            jvmti::JVMTI_EVENT_METHOD_EXIT,
        ])
    }

    /// Enable exception and exception-catch events for all threads.
    pub fn enable_exception_events(&self) -> Result<(), jvmti::jvmtiError> {
        self.enable_events_global(&[
            jvmti::JVMTI_EVENT_EXCEPTION,
            jvmti::JVMTI_EVENT_EXCEPTION_CATCH,
        ])
    }

    /// Enable sampled-object-allocation events for all threads.
    pub fn enable_heap_sampling_events(&self) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::HeapSampling)?;
        self.enable_events_global(&[jvmti::JVMTI_EVENT_SAMPLED_OBJECT_ALLOC])
    }

    /// Enable virtual-thread lifecycle events on runtimes that expose the
    /// preview or permanent JVM TI virtual-thread surface.
    pub fn enable_virtual_thread_events(&self) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::VirtualThreads)?;
        self.enable_events_global(&[
            jvmti::JVMTI_EVENT_VIRTUAL_THREAD_START,
            jvmti::JVMTI_EVENT_VIRTUAL_THREAD_END,
        ])
    }

    /// Enable VM init and VM death events for all threads.
    pub fn enable_vm_lifecycle_events(&self) -> Result<(), jvmti::jvmtiError> {
        self.enable_events_global(&[jvmti::JVMTI_EVENT_VM_INIT, jvmti::JVMTI_EVENT_VM_DEATH])
    }

    /// Configure a standard class-file-load-hook agent.
    ///
    /// This requests the required capability, wires default callbacks, and
    /// enables `ClassFileLoadHook` globally.
    pub fn configure_class_file_load_hook_agent(&self) -> Result<(), jvmti::jvmtiError> {
        self.add_class_file_load_hook_capabilities()?;
        self.set_default_agent_callbacks()?;
        self.enable_class_file_load_hook_events()
    }

    /// Configure a standard method-entry/method-exit tracing agent.
    ///
    /// This requests the required capabilities, wires default callbacks, and
    /// enables method entry and method exit globally.
    pub fn configure_method_trace_agent(&self) -> Result<(), jvmti::jvmtiError> {
        self.add_method_trace_capabilities()?;
        self.set_default_agent_callbacks()?;
        self.enable_method_entry_exit_events()
    }

    /// Configure a standard exception tracing agent.
    ///
    /// This requests the required capability, wires default callbacks, and
    /// enables exception and exception-catch events globally.
    pub fn configure_exception_agent(&self) -> Result<(), jvmti::jvmtiError> {
        self.add_exception_capabilities()?;
        self.set_default_agent_callbacks()?;
        self.enable_exception_events()
    }

    /// Configure a standard sampled heap allocation agent.
    ///
    /// This requests the required capability, wires default callbacks, and
    /// enables sampled-object-allocation events globally. Use
    /// [`Jvmti::set_heap_sampling_interval`] separately to tune the sample rate.
    pub fn configure_heap_sampling_agent(&self) -> Result<(), jvmti::jvmtiError> {
        self.add_heap_sampling_capabilities()?;
        self.set_default_agent_callbacks()?;
        self.enable_heap_sampling_events()
    }

    pub fn get_all_modules(&self) -> Result<Vec<jni::jobject>, jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::Modules)?;
        let mut module_count: jni::jint = 0;
        let mut modules_ptr: *mut jni::jobject = ptr::null_mut();

        unsafe {
            let get_all_modules_fn = jvmti_function!(self, GetAllModules)?;
            let err = get_all_modules_fn(self.env, &mut module_count, &mut modules_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }

            let modules = owned_jvmti_array_to_vec(self, modules_ptr, module_count)?;

            Ok(modules)
        }
    }

    pub fn get_all_threads(&self) -> Result<Vec<jni::jthread>, jvmti::jvmtiError> {
        let mut threads_count: jni::jint = 0;
        let mut threads_ptr: *mut jni::jthread = ptr::null_mut();

        unsafe {
            let get_all_threads_fn = jvmti_function!(self, GetAllThreads)?;
            let err = get_all_threads_fn(self.env, &mut threads_count, &mut threads_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }

            let threads = owned_jvmti_array_to_vec(self, threads_ptr, threads_count)?;

            Ok(threads)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_thread_info(
        &self,
        thread: jni::jthread,
    ) -> Result<ThreadInfo, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiThreadInfo::default();

        unsafe {
            let get_thread_info_fn = jvmti_function!(self, GetThreadInfo)?;
            let err = get_thread_info_fn(self.env, thread, &mut info);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        let name = owned_optional_jvmti_string(self, info.name)?;

        Ok(ThreadInfo {
            name,
            priority: info.priority,
            is_daemon: info.is_daemon != 0,
            thread_group: info.thread_group,
            context_class_loader: info.context_class_loader,
        })
    }

    /// Allocate JVM TI memory with environment-bound ownership.
    pub fn allocate(&self, size: usize) -> Result<JvmtiAllocation<'_>, jvmti::jvmtiError> {
        let size = jni::jlong::try_from(size).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        let ptr = unsafe { self.allocate_raw(size)? };
        Ok(JvmtiAllocation::from_raw(self, ptr, size as usize))
    }

    /// Allocate raw JVM TI memory.
    ///
    /// # Safety
    ///
    /// The returned pointer is owned by the caller and must be released with
    /// [`Jvmti::deallocate_raw`] on this environment or transferred to the JVM.
    pub unsafe fn allocate_raw(&self, size: jni::jlong) -> Result<*mut u8, jvmti::jvmtiError> {
        if size < 0 {
            return Err(jvmti::jvmtiError::ILLEGAL_ARGUMENT);
        }
        let mut mem_ptr: *mut u8 = ptr::null_mut();

        unsafe {
            let allocate_fn = jvmti_function!(self, Allocate)?;
            let err = allocate_fn(self.env, size, &mut mem_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        Ok(mem_ptr)
    }

    fn deallocate_owned(&self, mem: *mut u8) -> Result<(), jvmti::jvmtiError> {
        if mem.is_null() {
            return Ok(());
        }
        unsafe {
            let deallocate_fn = jvmti_function!(self, Deallocate)?;
            let err = deallocate_fn(self.env, mem);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Release raw memory previously allocated by this JVM TI environment.
    ///
    /// # Safety
    ///
    /// `mem` must be null or an allocation still owned by the caller and
    /// returned by this same environment. It must not have been freed already.
    pub unsafe fn deallocate_raw(&self, mem: *mut u8) -> Result<(), jvmti::jvmtiError> {
        self.deallocate_owned(mem)
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_thread_state(
        &self,
        thread: jni::jthread,
    ) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut thread_state: jni::jint = 0;

        unsafe {
            let get_thread_state_fn = jvmti_function!(self, GetThreadState)?;
            let err = get_thread_state_fn(self.env, thread, &mut thread_state);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        Ok(thread_state)
    }

    pub fn get_current_thread(&self) -> Result<jni::jthread, jvmti::jvmtiError> {
        let mut thread: jni::jthread = ptr::null_mut();

        unsafe {
            let get_current_thread_fn = jvmti_function!(self, GetCurrentThread)?;
            let err = get_current_thread_fn(self.env, &mut thread);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        Ok(thread)
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_class_signature(
        &self,
        klass: jni::jclass,
    ) -> Result<(String, Option<String>), jvmti::jvmtiError> {
        let mut sig_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        let mut gen_ptr: *mut std::os::raw::c_char = ptr::null_mut();

        unsafe {
            let get_class_sig_fn = jvmti_function!(self, GetClassSignature)?;
            let err = get_class_sig_fn(self.env, klass, &mut sig_ptr, &mut gen_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }

            let signature_allocation = JvmtiDeallocationGuard::new(self, sig_ptr);
            let generic_allocation = JvmtiDeallocationGuard::new(self, gen_ptr);
            let signature = signature_allocation.to_string()?;
            let generic = if !gen_ptr.is_null() {
                Some(generic_allocation.to_string()?)
            } else {
                None
            };

            Ok((signature, generic))
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_method_name(
        &self,
        method: jni::jmethodID,
    ) -> Result<(String, String, Option<String>), jvmti::jvmtiError> {
        let mut name_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        let mut sig_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        let mut gen_ptr: *mut std::os::raw::c_char = ptr::null_mut();

        unsafe {
            let get_method_name_fn = jvmti_function!(self, GetMethodName)?;
            let err =
                get_method_name_fn(self.env, method, &mut name_ptr, &mut sig_ptr, &mut gen_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }

            let name_allocation = JvmtiDeallocationGuard::new(self, name_ptr);
            let signature_allocation = JvmtiDeallocationGuard::new(self, sig_ptr);
            let generic_allocation = JvmtiDeallocationGuard::new(self, gen_ptr);
            let name = name_allocation.to_string()?;
            let signature = signature_allocation.to_string()?;
            let generic = if !gen_ptr.is_null() {
                Some(generic_allocation.to_string()?)
            } else {
                None
            };

            Ok((name, signature, generic))
        }
    }

    pub fn get_potential_capabilities(
        &self,
    ) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError> {
        let mut caps = jvmti::jvmtiCapabilities::default();

        unsafe {
            let get_pot_caps_fn = jvmti_function!(self, GetPotentialCapabilities)?;
            let err = get_pot_caps_fn(self.env, &mut caps);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        Ok(caps)
    }

    pub fn dispose_environment(self) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let dispose_env_fn = jvmti_function!(self, DisposeEnvironment)?;
            let err = dispose_env_fn(self.env);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn get_loaded_classes(&self) -> Result<Vec<jni::jclass>, jvmti::jvmtiError> {
        let mut class_count: jni::jint = 0;
        let mut classes_ptr: *mut jni::jclass = ptr::null_mut();

        unsafe {
            let get_loaded_classes_fn = jvmti_function!(self, GetLoadedClasses)?;
            let err = get_loaded_classes_fn(self.env, &mut class_count, &mut classes_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }

            let classes = owned_jvmti_array_to_vec(self, classes_ptr, class_count)?;

            Ok(classes)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    /// JDK 11 and later reject changes to `NestHost` and `NestMembers`; JDK 14
    /// and 15 add the same restriction for `Record` and
    /// `PermittedSubclasses` respectively.
    pub unsafe fn redefine_classes(
        &self,
        class_definitions: &[jvmti::jvmtiClassDefinition],
    ) -> Result<(), jvmti::jvmtiError> {
        let class_count = jint_len(class_definitions.len())?;
        unsafe {
            let redefine_classes_fn = jvmti_function!(self, RedefineClasses)?;
            let err = redefine_classes_fn(self.env, class_count, class_definitions.as_ptr());

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn suspend_thread(&self, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let suspend_fn = jvmti_function!(self, SuspendThread)?;
            let err = suspend_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn resume_thread(&self, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let resume_fn = jvmti_function!(self, ResumeThread)?;
            let err = resume_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn stop_thread(
        &self,
        thread: jni::jthread,
        exception: jni::jobject,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let stop_fn = jvmti_function!(self, StopThread)?;
            let err = stop_fn(self.env, thread, exception);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn interrupt_thread(&self, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let interrupt_fn = jvmti_function!(self, InterruptThread)?;
            let err = interrupt_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn run_agent_thread(
        &self,
        thread: jni::jthread,
        proc: jvmti::jvmtiStartFunction,
        arg: *const std::os::raw::c_void,
        priority: jni::jint,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let run_fn = jvmti_function!(self, RunAgentThread)?;
            let err = run_fn(self.env, thread, Some(proc), arg, priority);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn suspend_thread_list(
        &self,
        request_list: &[jni::jthread],
    ) -> Result<Vec<jvmti::jvmtiError>, jvmti::jvmtiError> {
        let request_count = jint_len(request_list.len())?;
        let mut results = vec![jvmti::jvmtiError::NONE; request_list.len()];
        unsafe {
            let suspend_list_fn = jvmti_function!(self, SuspendThreadList)?;
            let err = suspend_list_fn(
                self.env,
                request_count,
                request_list.as_ptr(),
                results.as_mut_ptr(),
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(results)
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn resume_thread_list(
        &self,
        request_list: &[jni::jthread],
    ) -> Result<Vec<jvmti::jvmtiError>, jvmti::jvmtiError> {
        let request_count = jint_len(request_list.len())?;
        let mut results = vec![jvmti::jvmtiError::NONE; request_list.len()];
        unsafe {
            let resume_list_fn = jvmti_function!(self, ResumeThreadList)?;
            let err = resume_list_fn(
                self.env,
                request_count,
                request_list.as_ptr(),
                results.as_mut_ptr(),
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(results)
    }

    pub fn get_top_thread_groups(&self) -> Result<Vec<jni::jobject>, jvmti::jvmtiError> {
        let mut group_count: jni::jint = 0;
        let mut groups_ptr: *mut jni::jobject = ptr::null_mut();
        unsafe {
            let get_groups_fn = jvmti_function!(self, GetTopThreadGroups)?;
            let err = get_groups_fn(self.env, &mut group_count, &mut groups_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let groups = owned_jvmti_array_to_vec(self, groups_ptr, group_count)?;
            Ok(groups)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_thread_group_info(
        &self,
        group: jni::jobject,
    ) -> Result<ThreadGroupInfo, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiThreadGroupInfo::default();
        unsafe {
            let get_info_fn = jvmti_function!(self, GetThreadGroupInfo)?;
            let err = get_info_fn(self.env, group, &mut info);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        let name = owned_optional_jvmti_string(self, info.name)?;
        Ok(ThreadGroupInfo {
            parent: info.parent,
            name,
            max_priority: info.max_priority,
            is_daemon: info.is_daemon != 0,
        })
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_thread_group_children(
        &self,
        group: jni::jobject,
    ) -> Result<(Vec<jni::jthread>, Vec<jni::jobject>), jvmti::jvmtiError> {
        let mut thread_count: jni::jint = 0;
        let mut threads_ptr: *mut jni::jthread = ptr::null_mut();
        let mut group_count: jni::jint = 0;
        let mut groups_ptr: *mut jni::jobject = ptr::null_mut();
        unsafe {
            let get_children_fn = jvmti_function!(self, GetThreadGroupChildren)?;
            let err = get_children_fn(
                self.env,
                group,
                &mut thread_count,
                &mut threads_ptr,
                &mut group_count,
                &mut groups_ptr,
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let threads_allocation = JvmtiDeallocationGuard::new(self, threads_ptr);
            let groups_allocation = JvmtiDeallocationGuard::new(self, groups_ptr);
            let threads = threads_allocation.to_vec(thread_count)?;
            let groups = groups_allocation.to_vec(group_count)?;
            Ok((threads, groups))
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_owned_monitor_info(
        &self,
        thread: jni::jthread,
    ) -> Result<Vec<jni::jobject>, jvmti::jvmtiError> {
        let mut monitor_count: jni::jint = 0;
        let mut monitors_ptr: *mut jni::jobject = ptr::null_mut();
        unsafe {
            let get_monitors_fn = jvmti_function!(self, GetOwnedMonitorInfo)?;
            let err = get_monitors_fn(self.env, thread, &mut monitor_count, &mut monitors_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let monitors = owned_jvmti_array_to_vec(self, monitors_ptr, monitor_count)?;
            Ok(monitors)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_current_contended_monitor(
        &self,
        thread: jni::jthread,
    ) -> Result<jni::jobject, jvmti::jvmtiError> {
        let mut monitor: jni::jobject = ptr::null_mut();
        unsafe {
            let get_monitor_fn = jvmti_function!(self, GetCurrentContendedMonitor)?;
            let err = get_monitor_fn(self.env, thread, &mut monitor);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(monitor)
        }
    }

    pub fn create_raw_monitor(&self, name: &str) -> Result<RawMonitor<'_>, jvmti::jvmtiError> {
        let monitor = unsafe { self.create_raw_monitor_raw(name)? };
        Ok(RawMonitor { env: self, monitor })
    }

    /// Create a raw monitor without an owning guard.
    ///
    /// Prefer [`Self::create_raw_monitor`].
    /// # Safety
    ///
    /// The returned monitor must be destroyed exactly once with this live JVM
    /// TI environment.
    pub unsafe fn create_raw_monitor_raw(
        &self,
        name: &str,
    ) -> Result<jvmti::jrawMonitorID, jvmti::jvmtiError> {
        let c_name = mutf8::encode_cstring(name);
        let mut monitor: jvmti::jrawMonitorID = ptr::null_mut();
        unsafe {
            let create_fn = jvmti_function!(self, CreateRawMonitor)?;
            let err = create_fn(self.env, c_name.as_ptr(), &mut monitor);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            if monitor.is_null() {
                return Err(jvmti::jvmtiError::INTERNAL);
            }
            Ok(monitor)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn destroy_raw_monitor(
        &self,
        monitor: jvmti::jrawMonitorID,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let destroy_fn = jvmti_function!(self, DestroyRawMonitor)?;
            let err = destroy_fn(self.env, monitor);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn raw_monitor_enter(
        &self,
        monitor: jvmti::jrawMonitorID,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let enter_fn = jvmti_function!(self, RawMonitorEnter)?;
            let err = enter_fn(self.env, monitor);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn raw_monitor_exit(
        &self,
        monitor: jvmti::jrawMonitorID,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let exit_fn = jvmti_function!(self, RawMonitorExit)?;
            let err = exit_fn(self.env, monitor);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn raw_monitor_wait(
        &self,
        monitor: jvmti::jrawMonitorID,
        millis: jni::jlong,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let wait_fn = jvmti_function!(self, RawMonitorWait)?;
            let err = wait_fn(self.env, monitor, millis);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn raw_monitor_notify(
        &self,
        monitor: jvmti::jrawMonitorID,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let notify_fn = jvmti_function!(self, RawMonitorNotify)?;
            let err = notify_fn(self.env, monitor);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn raw_monitor_notify_all(
        &self,
        monitor: jvmti::jrawMonitorID,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let notify_all_fn = jvmti_function!(self, RawMonitorNotifyAll)?;
            let err = notify_all_fn(self.env, monitor);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_frame_count(
        &self,
        thread: jni::jthread,
    ) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        unsafe {
            let get_count_fn = jvmti_function!(self, GetFrameCount)?;
            let err = get_count_fn(self.env, thread, &mut count);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(count)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_frame_location(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
    ) -> Result<(jni::jmethodID, jvmti::jlocation), jvmti::jvmtiError> {
        let mut method: jni::jmethodID = ptr::null_mut();
        let mut location: jvmti::jlocation = 0;
        unsafe {
            let get_loc_fn = jvmti_function!(self, GetFrameLocation)?;
            let err = get_loc_fn(self.env, thread, depth, &mut method, &mut location);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok((method, location))
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn notify_frame_pop(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let notify_fn = jvmti_function!(self, NotifyFramePop)?;
            let err = notify_fn(self.env, thread, depth);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    /// JDK 28 value-object preview may return a construction snapshot.
    pub unsafe fn get_local_object(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
        slot: jni::jint,
    ) -> Result<jni::jobject, jvmti::jvmtiError> {
        let mut value: jni::jobject = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetLocalObject)?;
            let err = get_fn(self.env, thread, depth, slot, &mut value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(value)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_local_int(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
        slot: jni::jint,
    ) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut value: jni::jint = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetLocalInt)?;
            let err = get_fn(self.env, thread, depth, slot, &mut value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(value)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_local_long(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
        slot: jni::jint,
    ) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut value: jni::jlong = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetLocalLong)?;
            let err = get_fn(self.env, thread, depth, slot, &mut value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(value)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_local_float(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
        slot: jni::jint,
    ) -> Result<jni::jfloat, jvmti::jvmtiError> {
        let mut value: jni::jfloat = 0.0;
        unsafe {
            let get_fn = jvmti_function!(self, GetLocalFloat)?;
            let err = get_fn(self.env, thread, depth, slot, &mut value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(value)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_local_double(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
        slot: jni::jint,
    ) -> Result<jni::jdouble, jvmti::jvmtiError> {
        let mut value: jni::jdouble = 0.0;
        unsafe {
            let get_fn = jvmti_function!(self, GetLocalDouble)?;
            let err = get_fn(self.env, thread, depth, slot, &mut value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(value)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_local_object(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
        slot: jni::jint,
        value: jni::jobject,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetLocalObject)?;
            let err = set_fn(self.env, thread, depth, slot, value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_local_int(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
        slot: jni::jint,
        value: jni::jint,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetLocalInt)?;
            let err = set_fn(self.env, thread, depth, slot, value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_local_long(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
        slot: jni::jint,
        value: jni::jlong,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetLocalLong)?;
            let err = set_fn(self.env, thread, depth, slot, value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_local_float(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
        slot: jni::jint,
        value: jni::jfloat,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetLocalFloat)?;
            let err = set_fn(self.env, thread, depth, slot, value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_local_double(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
        slot: jni::jint,
        value: jni::jdouble,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetLocalDouble)?;
            let err = set_fn(self.env, thread, depth, slot, value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    /// JDK 28 value-object preview may return a construction snapshot.
    pub unsafe fn get_local_instance(
        &self,
        thread: jni::jthread,
        depth: jni::jint,
    ) -> Result<jni::jobject, jvmti::jvmtiError> {
        let mut value: jni::jobject = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetLocalInstance)?;
            let err = get_fn(self.env, thread, depth, &mut value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(value)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    /// Since JDK 13, `thread` may be suspended or be the current thread.
    pub unsafe fn pop_frame(&self, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let pop_fn = jvmti_function!(self, PopFrame)?;
            let err = pop_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn force_early_return_object(
        &self,
        thread: jni::jthread,
        value: jni::jobject,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = jvmti_function!(self, ForceEarlyReturnObject)?;
            let err = force_fn(self.env, thread, value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn force_early_return_int(
        &self,
        thread: jni::jthread,
        value: jni::jint,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = jvmti_function!(self, ForceEarlyReturnInt)?;
            let err = force_fn(self.env, thread, value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn force_early_return_long(
        &self,
        thread: jni::jthread,
        value: jni::jlong,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = jvmti_function!(self, ForceEarlyReturnLong)?;
            let err = force_fn(self.env, thread, value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn force_early_return_float(
        &self,
        thread: jni::jthread,
        value: jni::jfloat,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = jvmti_function!(self, ForceEarlyReturnFloat)?;
            let err = force_fn(self.env, thread, value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn force_early_return_double(
        &self,
        thread: jni::jthread,
        value: jni::jdouble,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = jvmti_function!(self, ForceEarlyReturnDouble)?;
            let err = force_fn(self.env, thread, value);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    /// JDK 28 rejects this operation for a value-class constructor.
    pub unsafe fn force_early_return_void(
        &self,
        thread: jni::jthread,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = jvmti_function!(self, ForceEarlyReturnVoid)?;
            let err = force_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_stack_trace(
        &self,
        thread: jni::jthread,
        start_depth: jni::jint,
        max_frame_count: jni::jint,
    ) -> Result<Vec<jvmti::jvmtiFrameInfo>, jvmti::jvmtiError> {
        let max_frames = usize_count(max_frame_count)?;
        let mut frame_buffer = vec![jvmti::jvmtiFrameInfo::default(); max_frames];
        let mut count: jni::jint = 0;
        unsafe {
            let get_stack_fn = jvmti_function!(self, GetStackTrace)?;
            let err = get_stack_fn(
                self.env,
                thread,
                start_depth,
                max_frame_count,
                frame_buffer.as_mut_ptr(),
                &mut count,
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let count = usize_count(count)?;
            if count > frame_buffer.len() {
                return Err(jvmti::jvmtiError::INTERNAL);
            }
            frame_buffer.truncate(count);
            Ok(frame_buffer)
        }
    }

    pub fn get_all_stack_traces(
        &self,
        max_frame_count: jni::jint,
    ) -> Result<Vec<StackInfo>, jvmti::jvmtiError> {
        usize_count(max_frame_count)?;
        let mut stack_info_ptr: *mut jvmti::jvmtiStackInfo = ptr::null_mut();
        let mut thread_count: jni::jint = 0;
        unsafe {
            let get_all_fn = jvmti_function!(self, GetAllStackTraces)?;
            let err = get_all_fn(
                self.env,
                max_frame_count,
                &mut stack_info_ptr,
                &mut thread_count,
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        let allocation = JvmtiDeallocationGuard::new(self, stack_info_ptr);
        let info_slice = allocation.as_slice(thread_count)?;
        let mut out = Vec::with_capacity(info_slice.len());
        for info in info_slice {
            let frames = jvmti_array_to_vec(info.frame_buffer, info.frame_count)?;
            out.push(StackInfo {
                thread: info.thread,
                state: info.state,
                frames,
            });
        }

        // The frame buffers are embedded in `allocation` and must not be
        // deallocated separately.
        Ok(out)
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_thread_list_stack_traces(
        &self,
        thread_list: &[jni::jthread],
        max_frame_count: jni::jint,
    ) -> Result<Vec<StackInfo>, jvmti::jvmtiError> {
        usize_count(max_frame_count)?;
        let thread_count = jint_len(thread_list.len())?;
        let mut stack_info_ptr: *mut jvmti::jvmtiStackInfo = ptr::null_mut();
        unsafe {
            let get_list_fn = jvmti_function!(self, GetThreadListStackTraces)?;
            let err = get_list_fn(
                self.env,
                thread_count,
                thread_list.as_ptr(),
                max_frame_count,
                &mut stack_info_ptr,
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        let allocation = JvmtiDeallocationGuard::new(self, stack_info_ptr);
        let info_slice = allocation.as_slice(thread_count)?;
        let mut out = Vec::with_capacity(thread_list.len());
        for info in info_slice {
            let frames = jvmti_array_to_vec(info.frame_buffer, info.frame_count)?;
            out.push(StackInfo {
                thread: info.thread,
                state: info.state,
                frames,
            });
        }

        // The frame buffers are embedded in `allocation`.
        Ok(out)
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_named_module(
        &self,
        class_loader: jni::jobject,
        package_name: &str,
    ) -> Result<jni::jobject, jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::Modules)?;
        let c_package = mutf8::encode_cstring(package_name);
        let mut module: jni::jobject = ptr::null_mut();
        unsafe {
            let get_module_fn = jvmti_function!(self, GetNamedModule)?;
            let err = get_module_fn(self.env, class_loader, c_package.as_ptr(), &mut module);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(module)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_class_status(
        &self,
        klass: jni::jclass,
    ) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut status: jni::jint = 0;
        unsafe {
            let get_status_fn = jvmti_function!(self, GetClassStatus)?;
            let err = get_status_fn(self.env, klass, &mut status);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(status)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_source_file_name(
        &self,
        klass: jni::jclass,
    ) -> Result<String, jvmti::jvmtiError> {
        let mut name_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetSourceFileName)?;
            let err = get_fn(self.env, klass, &mut name_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let name = owned_jvmti_string(self, name_ptr)?;
            Ok(name)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    /// On JDK 28, bit `0x0020` means `ACC_IDENTITY` when value-object preview
    /// is enabled and retains its historical `ACC_SUPER` meaning otherwise.
    pub unsafe fn get_class_modifiers(
        &self,
        klass: jni::jclass,
    ) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut modifiers: jni::jint = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetClassModifiers)?;
            let err = get_fn(self.env, klass, &mut modifiers);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(modifiers)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_class_methods(
        &self,
        klass: jni::jclass,
    ) -> Result<Vec<jni::jmethodID>, jvmti::jvmtiError> {
        let mut method_count: jni::jint = 0;
        let mut methods_ptr: *mut jni::jmethodID = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetClassMethods)?;
            let err = get_fn(self.env, klass, &mut method_count, &mut methods_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let methods = owned_jvmti_array_to_vec(self, methods_ptr, method_count)?;
            Ok(methods)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_class_fields(
        &self,
        klass: jni::jclass,
    ) -> Result<Vec<jni::jfieldID>, jvmti::jvmtiError> {
        let mut field_count: jni::jint = 0;
        let mut fields_ptr: *mut jni::jfieldID = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetClassFields)?;
            let err = get_fn(self.env, klass, &mut field_count, &mut fields_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let fields = owned_jvmti_array_to_vec(self, fields_ptr, field_count)?;
            Ok(fields)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_implemented_interfaces(
        &self,
        klass: jni::jclass,
    ) -> Result<Vec<jni::jclass>, jvmti::jvmtiError> {
        let mut interface_count: jni::jint = 0;
        let mut interfaces_ptr: *mut jni::jclass = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetImplementedInterfaces)?;
            let err = get_fn(self.env, klass, &mut interface_count, &mut interfaces_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let interfaces = owned_jvmti_array_to_vec(self, interfaces_ptr, interface_count)?;
            Ok(interfaces)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn is_interface(&self, klass: jni::jclass) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = jvmti_function!(self, IsInterface)?;
            let err = get_fn(self.env, klass, &mut res);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(res != 0)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn is_array_class(&self, klass: jni::jclass) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = jvmti_function!(self, IsArrayClass)?;
            let err = get_fn(self.env, klass, &mut res);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(res != 0)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_class_loader(
        &self,
        klass: jni::jclass,
    ) -> Result<jni::jobject, jvmti::jvmtiError> {
        let mut loader: jni::jobject = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetClassLoader)?;
            let err = get_fn(self.env, klass, &mut loader);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(loader)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_field_name(
        &self,
        klass: jni::jclass,
        field: jni::jfieldID,
    ) -> Result<(String, String, Option<String>), jvmti::jvmtiError> {
        let mut name_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        let mut sig_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        let mut gen_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetFieldName)?;
            let err = get_fn(
                self.env,
                klass,
                field,
                &mut name_ptr,
                &mut sig_ptr,
                &mut gen_ptr,
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let name_allocation = JvmtiDeallocationGuard::new(self, name_ptr);
            let signature_allocation = JvmtiDeallocationGuard::new(self, sig_ptr);
            let generic_allocation = JvmtiDeallocationGuard::new(self, gen_ptr);
            let name = name_allocation.to_string()?;
            let sig = signature_allocation.to_string()?;
            let generic_signature = if gen_ptr.is_null() {
                None
            } else {
                Some(generic_allocation.to_string()?)
            };
            Ok((name, sig, generic_signature))
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_field_declaring_class(
        &self,
        klass: jni::jclass,
        field: jni::jfieldID,
    ) -> Result<jni::jclass, jvmti::jvmtiError> {
        let mut declaring_class: jni::jclass = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetFieldDeclaringClass)?;
            let err = get_fn(self.env, klass, field, &mut declaring_class);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(declaring_class)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_field_modifiers(
        &self,
        klass: jni::jclass,
        field: jni::jfieldID,
    ) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut modifiers: jni::jint = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetFieldModifiers)?;
            let err = get_fn(self.env, klass, field, &mut modifiers);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(modifiers)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn is_field_synthetic(
        &self,
        klass: jni::jclass,
        field: jni::jfieldID,
    ) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = jvmti_function!(self, IsFieldSynthetic)?;
            let err = get_fn(self.env, klass, field, &mut res);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(res != 0)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_method_declaring_class(
        &self,
        method: jni::jmethodID,
    ) -> Result<jni::jclass, jvmti::jvmtiError> {
        let mut declaring_class: jni::jclass = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetMethodDeclaringClass)?;
            let err = get_fn(self.env, method, &mut declaring_class);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(declaring_class)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_method_modifiers(
        &self,
        method: jni::jmethodID,
    ) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut modifiers: jni::jint = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetMethodModifiers)?;
            let err = get_fn(self.env, method, &mut modifiers);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(modifiers)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_max_locals(
        &self,
        method: jni::jmethodID,
    ) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut max: jni::jint = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetMaxLocals)?;
            let err = get_fn(self.env, method, &mut max);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(max)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_arguments_size(
        &self,
        method: jni::jmethodID,
    ) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut size: jni::jint = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetArgumentsSize)?;
            let err = get_fn(self.env, method, &mut size);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(size)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_line_number_table(
        &self,
        method: jni::jmethodID,
    ) -> Result<Vec<jvmti::jvmtiLineNumberEntry>, jvmti::jvmtiError> {
        let mut entry_count: jni::jint = 0;
        let mut table_ptr: *mut jvmti::jvmtiLineNumberEntry = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetLineNumberTable)?;
            let err = get_fn(self.env, method, &mut entry_count, &mut table_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let table = owned_jvmti_array_to_vec(self, table_ptr, entry_count)?;
            Ok(table)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_method_location(
        &self,
        method: jni::jmethodID,
    ) -> Result<(jvmti::jlocation, jvmti::jlocation), jvmti::jvmtiError> {
        let mut start: jvmti::jlocation = 0;
        let mut end: jvmti::jlocation = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetMethodLocation)?;
            let err = get_fn(self.env, method, &mut start, &mut end);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok((start, end))
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_local_variable_table(
        &self,
        method: jni::jmethodID,
    ) -> Result<Vec<LocalVariableEntry>, jvmti::jvmtiError> {
        let mut entry_count: jni::jint = 0;
        let mut table_ptr: *mut jvmti::jvmtiLocalVariableEntry = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetLocalVariableTable)?;
            let err = get_fn(self.env, method, &mut entry_count, &mut table_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        let table_allocation = JvmtiDeallocationGuard::new(self, table_ptr);
        let table = table_allocation.as_slice(entry_count)?;
        let base = table_ptr as *const u8;
        let len = std::mem::size_of_val(table);

        let mut out = Vec::with_capacity(table.len());
        for entry in table {
            let _name_allocation =
                if !entry.name.is_null() && !ptr_in_range(entry.name as *const u8, base, len) {
                    Some(JvmtiDeallocationGuard::new(self, entry.name))
                } else {
                    None
                };
            let _signature_allocation = if !entry.signature.is_null()
                && !ptr_in_range(entry.signature as *const u8, base, len)
            {
                Some(JvmtiDeallocationGuard::new(self, entry.signature))
            } else {
                None
            };
            let _generic_allocation = if !entry.generic_signature.is_null()
                && !ptr_in_range(entry.generic_signature as *const u8, base, len)
            {
                Some(JvmtiDeallocationGuard::new(self, entry.generic_signature))
            } else {
                None
            };
            let name = cstr_to_string(entry.name)?;
            let signature = cstr_to_string(entry.signature)?;
            let generic_signature = cstr_to_string(entry.generic_signature)?;
            out.push(LocalVariableEntry {
                start_location: entry.start_location,
                length: entry.length,
                name,
                signature,
                generic_signature,
                slot: entry.slot,
            });
        }
        Ok(out)
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_bytecodes(
        &self,
        method: jni::jmethodID,
    ) -> Result<Vec<u8>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut bytecodes_ptr: *mut u8 = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetBytecodes)?;
            let err = get_fn(self.env, method, &mut count, &mut bytecodes_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let bytecodes = owned_jvmti_array_to_vec(self, bytecodes_ptr, count)?;
            Ok(bytecodes)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn is_method_native(
        &self,
        method: jni::jmethodID,
    ) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = jvmti_function!(self, IsMethodNative)?;
            let err = get_fn(self.env, method, &mut res);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(res != 0)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn is_method_synthetic(
        &self,
        method: jni::jmethodID,
    ) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = jvmti_function!(self, IsMethodSynthetic)?;
            let err = get_fn(self.env, method, &mut res);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(res != 0)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn is_method_obsolete(
        &self,
        method: jni::jmethodID,
    ) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = jvmti_function!(self, IsMethodObsolete)?;
            let err = get_fn(self.env, method, &mut res);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(res != 0)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_classloader_classes(
        &self,
        initiating_loader: jni::jobject,
    ) -> Result<Vec<jni::jclass>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut classes_ptr: *mut jni::jclass = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetClassLoaderClasses)?;
            let err = get_fn(self.env, initiating_loader, &mut count, &mut classes_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let classes = owned_jvmti_array_to_vec(self, classes_ptr, count)?;
            Ok(classes)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_object_hash_code(
        &self,
        object: jni::jobject,
    ) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut hash: jni::jint = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetObjectHashCode)?;
            let err = get_fn(self.env, object, &mut hash);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(hash)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    /// JDK 28 value objects have no monitor identity and report empty state.
    pub unsafe fn get_object_monitor_usage(
        &self,
        object: jni::jobject,
    ) -> Result<MonitorUsage, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiMonitorUsage {
            owner: ptr::null_mut(),
            entry_count: 0,
            waiter_count: 0,
            waiters: ptr::null_mut(),
            notify_waiter_count: 0,
            notify_waiters: ptr::null_mut(),
        };
        unsafe {
            let get_fn = jvmti_function!(self, GetObjectMonitorUsage)?;
            let err = get_fn(self.env, object, &mut info);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        let waiters_allocation = JvmtiDeallocationGuard::new(self, info.waiters);
        let notify_waiters_allocation = JvmtiDeallocationGuard::new(self, info.notify_waiters);
        let waiters = waiters_allocation.to_vec(info.waiter_count)?;
        let notify_waiters = notify_waiters_allocation.to_vec(info.notify_waiter_count)?;

        Ok(MonitorUsage {
            owner: info.owner,
            entry_count: info.entry_count,
            waiters,
            notify_waiters,
        })
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    /// JDK 28 value-object tags use value equality, not stable identity.
    pub unsafe fn get_tag(&self, object: jni::jobject) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut tag: jni::jlong = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetTag)?;
            let err = get_fn(self.env, object, &mut tag);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(tag)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    /// JDK 28 value-object tags use value equality and do not produce
    /// `ObjectFree` notifications.
    pub unsafe fn set_tag(
        &self,
        object: jni::jobject,
        tag: jni::jlong,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetTag)?;
            let err = set_fn(self.env, object, tag);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn force_garbage_collection(&self) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = jvmti_function!(self, ForceGarbageCollection)?;
            let err = force_fn(self.env);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    #[deprecated(
        since = "3.0.0",
        note = "deprecated by JVM TI since JDK 17; use follow_references"
    )]
    pub unsafe fn iterate_over_objects_reachable_from_object(
        &self,
        object: jni::jobject,
        cb: jvmti::jvmtiObjectReferenceCallback,
        user_data: *const std::os::raw::c_void,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let iter_fn = jvmti_function!(self, IterateOverObjectsReachableFromObject)?;
            let err = iter_fn(self.env, object, Some(cb), user_data);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    #[deprecated(
        since = "3.0.0",
        note = "deprecated by JVM TI since JDK 17; use follow_references"
    )]
    pub unsafe fn iterate_over_reachable_objects(
        &self,
        root_cb: jvmti::jvmtiHeapRootCallback,
        stack_cb: jvmti::jvmtiStackReferenceCallback,
        obj_cb: jvmti::jvmtiObjectReferenceCallback,
        user_data: *const std::os::raw::c_void,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let iter_fn = jvmti_function!(self, IterateOverReachableObjects)?;
            let err = iter_fn(
                self.env,
                Some(root_cb),
                Some(stack_cb),
                Some(obj_cb),
                user_data,
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    #[deprecated(
        since = "3.0.0",
        note = "deprecated by JVM TI since JDK 17; use iterate_through_heap"
    )]
    pub unsafe fn iterate_over_heap(
        &self,
        filter: jvmti::jvmtiHeapObjectFilter,
        cb: jvmti::jvmtiHeapObjectCallback,
        user_data: *const std::os::raw::c_void,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let iter_fn = jvmti_function!(self, IterateOverHeap)?;
            let err = iter_fn(self.env, filter, Some(cb), user_data);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    #[deprecated(
        since = "3.0.0",
        note = "deprecated by JVM TI since JDK 17; use iterate_through_heap"
    )]
    pub unsafe fn iterate_over_instances_of_class(
        &self,
        klass: jni::jclass,
        filter: jvmti::jvmtiHeapObjectFilter,
        cb: jvmti::jvmtiHeapObjectCallback,
        user_data: *const std::os::raw::c_void,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let iter_fn = jvmti_function!(self, IterateOverInstancesOfClass)?;
            let err = iter_fn(self.env, klass, filter, Some(cb), user_data);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn get_objects_with_tags(
        &self,
        tags: &[jni::jlong],
    ) -> Result<(Vec<jni::jobject>, Vec<jni::jlong>), jvmti::jvmtiError> {
        let tag_count = jint_len(tags.len())?;
        let mut count: jni::jint = 0;
        let mut objects_ptr: *mut jni::jobject = ptr::null_mut();
        let mut tags_ptr: *mut jni::jlong = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetObjectsWithTags)?;
            let err = get_fn(
                self.env,
                tag_count,
                tags.as_ptr(),
                &mut count,
                &mut objects_ptr,
                &mut tags_ptr,
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let objects_allocation = JvmtiDeallocationGuard::new(self, objects_ptr);
            let tags_allocation = JvmtiDeallocationGuard::new(self, tags_ptr);
            let objects = objects_allocation.to_vec(count)?;
            let res_tags = tags_allocation.to_vec(count)?;
            Ok((objects, res_tags))
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn follow_references(
        &self,
        heap_filter: jni::jint,
        klass: jni::jclass,
        initial_object: jni::jobject,
        callbacks: &jvmti::jvmtiHeapCallbacks,
        user_data: *const std::os::raw::c_void,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let follow_fn = jvmti_function!(self, FollowReferences)?;
            let err = follow_fn(
                self.env,
                heap_filter,
                klass,
                initial_object,
                callbacks,
                user_data,
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn iterate_through_heap(
        &self,
        heap_filter: jni::jint,
        klass: jni::jclass,
        callbacks: &jvmti::jvmtiHeapCallbacks,
        user_data: *const std::os::raw::c_void,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let iter_fn = jvmti_function!(self, IterateThroughHeap)?;
            let err = iter_fn(self.env, heap_filter, klass, callbacks, user_data);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_object_size(
        &self,
        object: jni::jobject,
    ) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut size: jni::jlong = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetObjectSize)?;
            let err = get_fn(self.env, object, &mut size);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(size)
        }
    }

    pub fn set_heap_sampling_interval(&self, interval: jni::jint) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::HeapSampling)?;
        unsafe {
            let set_fn = jvmti_function!(self, SetHeapSamplingInterval)?;
            let err = set_fn(self.env, interval);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_breakpoint(
        &self,
        method: jni::jmethodID,
        location: jvmti::jlocation,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetBreakpoint)?;
            let err = set_fn(self.env, method, location);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn clear_breakpoint(
        &self,
        method: jni::jmethodID,
        location: jvmti::jlocation,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let clear_fn = jvmti_function!(self, ClearBreakpoint)?;
            let err = clear_fn(self.env, method, location);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_field_access_watch(
        &self,
        klass: jni::jclass,
        field: jni::jfieldID,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetFieldAccessWatch)?;
            let err = set_fn(self.env, klass, field);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn clear_field_access_watch(
        &self,
        klass: jni::jclass,
        field: jni::jfieldID,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let clear_fn = jvmti_function!(self, ClearFieldAccessWatch)?;
            let err = clear_fn(self.env, klass, field);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_field_modification_watch(
        &self,
        klass: jni::jclass,
        field: jni::jfieldID,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetFieldModificationWatch)?;
            let err = set_fn(self.env, klass, field);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn clear_field_modification_watch(
        &self,
        klass: jni::jclass,
        field: jni::jfieldID,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let clear_fn = jvmti_function!(self, ClearFieldModificationWatch)?;
            let err = clear_fn(self.env, klass, field);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn is_modifiable_class(
        &self,
        klass: jni::jclass,
    ) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let is_fn = jvmti_function!(self, IsModifiableClass)?;
            let err = is_fn(self.env, klass, &mut res);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(res != 0)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn retransform_classes(
        &self,
        classes: &[jni::jclass],
    ) -> Result<(), jvmti::jvmtiError> {
        let class_count = jint_len(classes.len())?;
        unsafe {
            let retransform_fn = jvmti_function!(self, RetransformClasses)?;
            let err = retransform_fn(self.env, class_count, classes.as_ptr());
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn is_modifiable_module(
        &self,
        module: jni::jobject,
    ) -> Result<bool, jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::Modules)?;
        let mut res: jni::jboolean = 0;
        unsafe {
            let is_fn = jvmti_function!(self, IsModifiableModule)?;
            let err = is_fn(self.env, module, &mut res);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(res != 0)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn add_module_reads(
        &self,
        module: jni::jobject,
        source_module: jni::jobject,
    ) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::Modules)?;
        unsafe {
            let add_fn = jvmti_function!(self, AddModuleReads)?;
            let err = add_fn(self.env, module, source_module);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn add_module_exports(
        &self,
        module: jni::jobject,
        package: &str,
        to_module: jni::jobject,
    ) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::Modules)?;
        let c_package = mutf8::encode_cstring(package);
        unsafe {
            let add_fn = jvmti_function!(self, AddModuleExports)?;
            let err = add_fn(self.env, module, c_package.as_ptr(), to_module);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn add_module_opens(
        &self,
        module: jni::jobject,
        package: &str,
        to_module: jni::jobject,
    ) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::Modules)?;
        let c_package = mutf8::encode_cstring(package);
        unsafe {
            let add_fn = jvmti_function!(self, AddModuleOpens)?;
            let err = add_fn(self.env, module, c_package.as_ptr(), to_module);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn add_module_uses(
        &self,
        module: jni::jobject,
        service: jni::jclass,
    ) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::Modules)?;
        unsafe {
            let add_fn = jvmti_function!(self, AddModuleUses)?;
            let err = add_fn(self.env, module, service);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn add_module_provides(
        &self,
        module: jni::jobject,
        service: jni::jclass,
        implementation: jni::jclass,
    ) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::Modules)?;
        unsafe {
            let add_fn = jvmti_function!(self, AddModuleProvides)?;
            let err = add_fn(self.env, module, service, implementation);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn get_version_number(&self) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut version: jni::jint = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetVersionNumber)?;
            let err = get_fn(self.env, &mut version);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(version)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_source_debug_extension(
        &self,
        klass: jni::jclass,
    ) -> Result<String, jvmti::jvmtiError> {
        let mut ext_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetSourceDebugExtension)?;
            let err = get_fn(self.env, klass, &mut ext_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let ext = owned_jvmti_string(self, ext_ptr)?;
            Ok(ext)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_thread_local_storage(
        &self,
        thread: jni::jthread,
    ) -> Result<*mut std::os::raw::c_void, jvmti::jvmtiError> {
        let mut data: *mut std::os::raw::c_void = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetThreadLocalStorage)?;
            let err = get_fn(self.env, thread, &mut data);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(data)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_thread_local_storage(
        &self,
        thread: jni::jthread,
        data: *const std::os::raw::c_void,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetThreadLocalStorage)?;
            let err = set_fn(self.env, thread, data);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn suspend_all_virtual_threads(
        &self,
        exceptions: &[jni::jthread],
    ) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::VirtualThreads)?;
        let count = jint_len(exceptions.len())?;
        unsafe {
            let suspend_fn = jvmti_function!(self, SuspendAllVirtualThreads)?;
            let err = suspend_fn(self.env, count, exceptions.as_ptr());
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn resume_all_virtual_threads(
        &self,
        exceptions: &[jni::jthread],
    ) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::VirtualThreads)?;
        let count = jint_len(exceptions.len())?;
        unsafe {
            let resume_fn = jvmti_function!(self, ResumeAllVirtualThreads)?;
            let err = resume_fn(self.env, count, exceptions.as_ptr());
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Installs a complete JNI function table for this JVM.
    ///
    /// # Safety
    ///
    /// The table must be valid for this JVM and remain alive while installed.
    pub unsafe fn set_jni_function_table(
        &self,
        function_table: *const jni::JNINativeInterface_,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetJNIFunctionTable)?;
            let err = set_fn(self.env, function_table);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Returns a JVM-allocated JNI table copy.
    ///
    /// The returned table is released automatically with JVM TI `Deallocate`.
    pub fn get_jni_function_table(&self) -> Result<JniFunctionTable<'_>, jvmti::jvmtiError> {
        let interface_feature = jvmti_interface_feature(self.get_version_number()?);
        let mut table_ptr: *mut jni::JNINativeInterface_ = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetJNIFunctionTable)?;
            let err = get_fn(self.env, &mut table_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            if table_ptr.is_null() {
                return Err(jvmti::jvmtiError::NULL_POINTER);
            }
            Ok(JniFunctionTable {
                env: self,
                ptr: table_ptr,
                jvmti_interface_feature: interface_feature,
                known_byte_len: release_profile(interface_feature)
                    .map(|profile| profile.jni_table_bytes()),
            })
        }
    }

    pub fn generate_events(&self, event_type: jvmti::jvmtiEvent) -> Result<(), jvmti::jvmtiError> {
        self.require_event_type(event_type)?;
        unsafe {
            let gen_fn = jvmti_function!(self, GenerateEvents)?;
            let err = gen_fn(self.env, event_type);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn get_extension_functions(&self) -> Result<Vec<ExtensionFunctionInfo>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut ext_ptr: *mut jvmti::jvmtiExtensionFunctionInfo = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetExtensionFunctions)?;
            let err = get_fn(self.env, &mut count, &mut ext_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        let ext_allocation = JvmtiDeallocationGuard::new(self, ext_ptr);
        let exts = ext_allocation.as_slice(count)?;
        let base = ext_ptr as *const u8;
        let len = std::mem::size_of_val(exts);

        let mut out = Vec::with_capacity(exts.len());
        for ext in exts {
            let _id_allocation =
                if !ext.id.is_null() && !ptr_in_range(ext.id as *const u8, base, len) {
                    Some(JvmtiDeallocationGuard::new(self, ext.id))
                } else {
                    None
                };
            let _description_allocation = if !ext.short_description.is_null()
                && !ptr_in_range(ext.short_description as *const u8, base, len)
            {
                Some(JvmtiDeallocationGuard::new(self, ext.short_description))
            } else {
                None
            };
            let id = cstr_to_string(ext.id)?;
            let short_description = cstr_to_string(ext.short_description)?;

            let _params_allocation =
                if !ext.params.is_null() && !ptr_in_range(ext.params as *const u8, base, len) {
                    Some(JvmtiDeallocationGuard::new(self, ext.params))
                } else {
                    None
                };
            let _errors_allocation =
                if !ext.errors.is_null() && !ptr_in_range(ext.errors as *const u8, base, len) {
                    Some(JvmtiDeallocationGuard::new(self, ext.errors))
                } else {
                    None
                };
            let params = with_jvmti_array(ext.params, ext.param_count, |param_slice| {
                let params_base = ext.params as *const u8;
                let params_len = std::mem::size_of_val(param_slice);
                let mut params = Vec::with_capacity(param_slice.len());
                for p in param_slice {
                    let _name_allocation = if !p.name.is_null()
                        && !ptr_in_range(p.name as *const u8, params_base, params_len)
                        && !ptr_in_range(p.name as *const u8, base, len)
                    {
                        Some(JvmtiDeallocationGuard::new(self, p.name))
                    } else {
                        None
                    };
                    let name = cstr_to_string(p.name)?;
                    params.push(ExtensionParamInfo {
                        name,
                        kind: p.kind,
                        base_type: p.base_type,
                        null_ok: p.null_ok != 0,
                    });
                }
                Ok(params)
            })?;

            let errors =
                with_jvmti_array(ext.errors, ext.error_count, |errors| Ok(errors.to_vec()))?;

            out.push(ExtensionFunctionInfo {
                func: ext.func,
                id,
                short_description,
                params,
                errors,
            });
        }

        Ok(out)
    }

    pub fn get_extension_events(&self) -> Result<Vec<ExtensionEventInfo>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut ext_ptr: *mut jvmti::jvmtiExtensionEventInfo = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetExtensionEvents)?;
            let err = get_fn(self.env, &mut count, &mut ext_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        let ext_allocation = JvmtiDeallocationGuard::new(self, ext_ptr);
        let exts = ext_allocation.as_slice(count)?;
        let base = ext_ptr as *const u8;
        let len = std::mem::size_of_val(exts);

        let mut out = Vec::with_capacity(exts.len());
        for ext in exts {
            let _id_allocation =
                if !ext.id.is_null() && !ptr_in_range(ext.id as *const u8, base, len) {
                    Some(JvmtiDeallocationGuard::new(self, ext.id))
                } else {
                    None
                };
            let _description_allocation = if !ext.short_description.is_null()
                && !ptr_in_range(ext.short_description as *const u8, base, len)
            {
                Some(JvmtiDeallocationGuard::new(self, ext.short_description))
            } else {
                None
            };
            let id = cstr_to_string(ext.id)?;
            let short_description = cstr_to_string(ext.short_description)?;

            let _params_allocation =
                if !ext.params.is_null() && !ptr_in_range(ext.params as *const u8, base, len) {
                    Some(JvmtiDeallocationGuard::new(self, ext.params))
                } else {
                    None
                };
            let params = with_jvmti_array(ext.params, ext.param_count, |param_slice| {
                let params_base = ext.params as *const u8;
                let params_len = std::mem::size_of_val(param_slice);
                let mut params = Vec::with_capacity(param_slice.len());
                for p in param_slice {
                    let _name_allocation = if !p.name.is_null()
                        && !ptr_in_range(p.name as *const u8, params_base, params_len)
                        && !ptr_in_range(p.name as *const u8, base, len)
                    {
                        Some(JvmtiDeallocationGuard::new(self, p.name))
                    } else {
                        None
                    };
                    let name = cstr_to_string(p.name)?;
                    params.push(ExtensionParamInfo {
                        name,
                        kind: p.kind,
                        base_type: p.base_type,
                        null_ok: p.null_ok != 0,
                    });
                }
                Ok(params)
            })?;

            out.push(ExtensionEventInfo {
                extension_event_index: ext.extension_event_index,
                id,
                short_description,
                params,
            });
        }

        Ok(out)
    }

    pub fn set_extension_event_callback(
        &self,
        extension_event_index: jni::jint,
        callback: jvmti::jvmtiExtensionEventCallback,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetExtensionEventCallback)?;
            let err = set_fn(self.env, extension_event_index, callback);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn get_error_name(&self, error: jvmti::jvmtiError) -> Result<String, jvmti::jvmtiError> {
        let mut name_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetErrorName)?;
            let err = get_fn(self.env, error, &mut name_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let name = owned_jvmti_string(self, name_ptr)?;
            Ok(name)
        }
    }

    /// Return the JVM-provided JVMTI error name as an owned string.
    pub fn get_error_name_string(
        &self,
        error: jvmti::jvmtiError,
    ) -> Result<String, jvmti::jvmtiError> {
        self.get_error_name(error)
    }

    /// Best-effort conversion of a JVMTI error to a readable string.
    pub fn error_to_string(&self, error: jvmti::jvmtiError) -> String {
        self.get_error_name_string(error)
            .unwrap_or_else(|_| jvmti::error_name(error).to_string())
    }

    pub fn get_jlocation_format(&self) -> Result<jvmti::jvmtiJlocationFormat, jvmti::jvmtiError> {
        let mut format: jvmti::jvmtiJlocationFormat = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetJLocationFormat)?;
            let err = get_fn(self.env, &mut format);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(format)
        }
    }

    pub fn get_system_properties(&self) -> Result<Vec<String>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut props_ptr: *mut *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetSystemProperties)?;
            let err = get_fn(self.env, &mut count, &mut props_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let props_allocation = JvmtiDeallocationGuard::new(self, props_ptr);
            let prop_ptrs = props_allocation.as_slice(count)?;
            let base = props_ptr as *const u8;
            let len = std::mem::size_of_val(prop_ptrs);
            if prop_ptrs.iter().any(|property| property.is_null()) {
                for &property in prop_ptrs {
                    if !property.is_null() && !ptr_in_range(property as *const u8, base, len) {
                        let _allocation = JvmtiDeallocationGuard::new(self, property);
                    }
                }
                return Err(jvmti::jvmtiError::NULL_POINTER);
            }

            let mut props = Vec::with_capacity(prop_ptrs.len());
            for &property in prop_ptrs {
                let _allocation = if !ptr_in_range(property as *const u8, base, len) {
                    Some(JvmtiDeallocationGuard::new(self, property))
                } else {
                    None
                };
                props.push(
                    mutf8::decode_cstr(CStr::from_ptr(property))
                        .map_err(|_| jvmti::jvmtiError::INTERNAL)?,
                );
            }
            Ok(props)
        }
    }

    pub fn get_system_property(&self, property: &str) -> Result<String, jvmti::jvmtiError> {
        let c_property = mutf8::encode_cstring(property);
        let mut value_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetSystemProperty)?;
            let err = get_fn(self.env, c_property.as_ptr(), &mut value_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let value = owned_jvmti_string(self, value_ptr)?;
            Ok(value)
        }
    }

    pub fn set_system_property(
        &self,
        property: &str,
        value: &str,
    ) -> Result<(), jvmti::jvmtiError> {
        let c_property = mutf8::encode_cstring(property);
        let c_value = mutf8::encode_cstring(value);
        unsafe {
            let set_fn = jvmti_function!(self, SetSystemProperty)?;
            let err = set_fn(self.env, c_property.as_ptr(), c_value.as_ptr());
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn get_phase(&self) -> Result<jvmti::jvmtiPhase, jvmti::jvmtiError> {
        let mut phase: jvmti::jvmtiPhase = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetPhase)?;
            let err = get_fn(self.env, &mut phase);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(phase)
        }
    }

    pub fn get_current_thread_cpu_timer_info(
        &self,
    ) -> Result<jvmti::jvmtiTimerInfo, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiTimerInfo::default();
        unsafe {
            let get_fn = jvmti_function!(self, GetCurrentThreadCpuTimerInfo)?;
            let err = get_fn(self.env, &mut info);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(info)
        }
    }

    pub fn get_current_thread_cpu_time(&self) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut nanos: jni::jlong = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetCurrentThreadCpuTime)?;
            let err = get_fn(self.env, &mut nanos);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(nanos)
        }
    }

    pub fn get_thread_cpu_timer_info(&self) -> Result<jvmti::jvmtiTimerInfo, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiTimerInfo::default();
        unsafe {
            let get_fn = jvmti_function!(self, GetThreadCpuTimerInfo)?;
            let err = get_fn(self.env, &mut info);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(info)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_thread_cpu_time(
        &self,
        thread: jni::jthread,
    ) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut nanos: jni::jlong = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetThreadCpuTime)?;
            let err = get_fn(self.env, thread, &mut nanos);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(nanos)
        }
    }

    pub fn get_timer_info(&self) -> Result<jvmti::jvmtiTimerInfo, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiTimerInfo::default();
        unsafe {
            let get_fn = jvmti_function!(self, GetTimerInfo)?;
            let err = get_fn(self.env, &mut info);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(info)
        }
    }

    pub fn get_time(&self) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut nanos: jni::jlong = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetTime)?;
            let err = get_fn(self.env, &mut nanos);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(nanos)
        }
    }

    pub fn relinquish_capabilities(
        &self,
        caps: &jvmti::jvmtiCapabilities,
    ) -> Result<(), jvmti::jvmtiError> {
        self.validate_versioned_capabilities(caps)?;
        unsafe {
            let rel_fn = jvmti_function!(self, RelinquishCapabilities)?;
            let err = rel_fn(self.env, caps);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn get_available_processors(&self) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut processors: jni::jint = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetAvailableProcessors)?;
            let err = get_fn(self.env, &mut processors);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(processors)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_class_version_numbers(
        &self,
        klass: jni::jclass,
    ) -> Result<(jni::jint, jni::jint), jvmti::jvmtiError> {
        let mut minor: jni::jint = 0;
        let mut major: jni::jint = 0;
        unsafe {
            let get_fn = jvmti_function!(self, GetClassVersionNumbers)?;
            let err = get_fn(self.env, klass, &mut minor, &mut major);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok((minor, major))
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_constant_pool(
        &self,
        klass: jni::jclass,
    ) -> Result<Vec<u8>, jvmti::jvmtiError> {
        let mut pool_count: jni::jint = 0;
        let mut byte_count: jni::jint = 0;
        let mut bytes_ptr: *mut u8 = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetConstantPool)?;
            let err = get_fn(
                self.env,
                klass,
                &mut pool_count,
                &mut byte_count,
                &mut bytes_ptr,
            );
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let bytes = owned_jvmti_array_to_vec(self, bytes_ptr, byte_count)?;
            Ok(bytes)
        }
    }

    pub fn get_environment_local_storage(
        &self,
    ) -> Result<*mut std::os::raw::c_void, jvmti::jvmtiError> {
        let mut data: *mut std::os::raw::c_void = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetEnvironmentLocalStorage)?;
            let err = get_fn(self.env, &mut data);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            Ok(data)
        }
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn set_environment_local_storage(
        &self,
        data: *const std::os::raw::c_void,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetEnvironmentLocalStorage)?;
            let err = set_fn(self.env, data);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn add_to_bootstrap_class_loader_search(
        &self,
        segment: &str,
    ) -> Result<(), jvmti::jvmtiError> {
        let c_segment = mutf8::encode_cstring(segment);
        unsafe {
            let add_fn = jvmti_function!(self, AddToBootstrapClassLoaderSearch)?;
            let err = add_fn(self.env, c_segment.as_ptr());
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn set_verbose_flag(
        &self,
        flag: jvmti::jvmtiVerboseFlag,
        value: bool,
    ) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = jvmti_function!(self, SetVerboseFlag)?;
            let err = set_fn(self.env, flag, if value { 1 } else { 0 });
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn add_to_system_class_loader_search(
        &self,
        segment: &str,
    ) -> Result<(), jvmti::jvmtiError> {
        let c_segment = mutf8::encode_cstring(segment);
        unsafe {
            let add_fn = jvmti_function!(self, AddToSystemClassLoaderSearch)?;
            let err = add_fn(self.env, c_segment.as_ptr());
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn get_owned_monitor_stack_depth_info(
        &self,
        thread: jni::jthread,
    ) -> Result<Vec<jvmti::jvmtiMonitorStackDepthInfo>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut info_ptr: *mut jvmti::jvmtiMonitorStackDepthInfo = ptr::null_mut();
        unsafe {
            let get_fn = jvmti_function!(self, GetOwnedMonitorStackDepthInfo)?;
            let err = get_fn(self.env, thread, &mut count, &mut info_ptr);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
            let info = owned_jvmti_array_to_vec(self, info_ptr, count)?;
            Ok(info)
        }
    }

    // =========================================================================
    // Native Method Prefixes
    // =========================================================================

    /// Sets a prefix for native method resolution.
    ///
    /// When the JVM attempts to resolve a native method, it will first try the
    /// prefixed name before falling back to the original name. This is useful
    /// for wrapping native methods with instrumentation.
    ///
    /// Requires `can_set_native_method_prefix` capability.
    ///
    /// # Example
    ///
    /// If prefix is "wrapped_" and native method is `native void foo()`,
    /// the JVM will first look for `wrapped_foo` before `foo`.
    pub fn set_native_method_prefix(&self, prefix: &str) -> Result<(), jvmti::jvmtiError> {
        let c_prefix = mutf8::encode_cstring(prefix);
        unsafe {
            let set_fn = jvmti_function!(self, SetNativeMethodPrefix)?;
            let err = set_fn(self.env, c_prefix.as_ptr() as *mut _);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Sets multiple prefixes for native method resolution.
    ///
    /// The JVM will try each prefix in order when resolving native methods.
    /// This allows multiple agents to each wrap native methods.
    ///
    /// Requires `can_set_native_method_prefix` capability.
    pub fn set_native_method_prefixes(&self, prefixes: &[&str]) -> Result<(), jvmti::jvmtiError> {
        let prefix_count = jint_len(prefixes.len())?;
        let c_prefixes: Vec<_> = prefixes
            .iter()
            .map(|prefix| mutf8::encode_cstring(prefix))
            .collect();
        let mut prefix_ptrs: Vec<*mut std::os::raw::c_char> = c_prefixes
            .iter()
            .map(|s| s.as_ptr() as *mut std::os::raw::c_char)
            .collect();
        unsafe {
            let set_fn = jvmti_function!(self, SetNativeMethodPrefixes)?;
            let err = set_fn(self.env, prefix_count, prefix_ptrs.as_mut_ptr());
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    // =========================================================================
    // Frame Pops (JDK 25+)
    // =========================================================================

    /// Clears all pending frame pop notifications for a thread.
    ///
    /// This removes all frame pop notifications that were requested via
    /// `notify_frame_pop` for the specified thread.
    ///
    /// This function occupies a formerly reserved slot starting in JDK 25.
    /// Older runtimes return `JVMTI_ERROR_NOT_AVAILABLE` without reading it.
    ///
    /// Requires `can_generate_frame_pop_events` capability.
    /// # Safety
    ///
    /// Every JNI/JVM TI handle, callback, and pointer argument must satisfy this operation contract and belong to the same live VM or environment.
    pub unsafe fn clear_all_frame_pops(
        &self,
        thread: jni::jthread,
    ) -> Result<(), jvmti::jvmtiError> {
        self.require_feature(JvmtiFeature::ClearAllFramePops)?;
        unsafe {
            let clear_fn = jvmti_function!(self, ClearAllFramePops)?;
            let err = clear_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{cstr_to_string, ptr_in_range};
    use crate::sys::jvmti;

    #[test]
    fn pointer_range_checks_are_half_open_and_overflow_safe() {
        let base = std::ptr::without_provenance::<u8>(0x1000);
        assert!(ptr_in_range(base, base, 8));
        assert!(ptr_in_range(
            std::ptr::without_provenance::<u8>(0x1007),
            base,
            8
        ));
        assert!(!ptr_in_range(
            std::ptr::without_provenance::<u8>(0x1008),
            base,
            8
        ));
        assert!(!ptr_in_range(
            std::ptr::without_provenance::<u8>(usize::MAX),
            std::ptr::without_provenance::<u8>(usize::MAX - 3),
            8
        ));
        assert!(!ptr_in_range(std::ptr::null(), base, 8));
        assert!(!ptr_in_range(base, base, 0));
    }

    #[test]
    fn optional_native_strings_distinguish_null_from_malformed() {
        assert_eq!(cstr_to_string(std::ptr::null()).unwrap(), None);

        let malformed = [0xff_u8, 0];
        assert_eq!(
            cstr_to_string(malformed.as_ptr().cast()),
            Err(jvmti::jvmtiError::INTERNAL)
        );
    }
}
