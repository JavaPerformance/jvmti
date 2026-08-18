//! Safe wrapper around the JNI environment.
//!
//! This module provides ergonomic Rust wrappers for common JNI operations.
//!
//! # Example
//!
//! ```rust,no_run
//! use jvmti_bindings::prelude::*;
//!
//! fn vm_init(jni: *mut jni::JNIEnv, thread: jni::jthread) {
//!     let env = unsafe { JniEnv::from_raw(jni) };
//!
//!     // Find a class
//!     let Some(string_class) = env.find_class("java/lang/String") else {
//!         return;
//!     };
//!
//!     // Create a string
//!     let Some(greeting) = env.new_string_utf("Hello from Rust!") else {
//!         return;
//!     };
//!
//!     // Check for exceptions
//!     if env.exception_check() {
//!         env.exception_describe();
//!         env.exception_clear();
//!     }
//! }
//! ```
//!
//! # Thread-Local Safety
//!
//! `JniEnv`, `GlobalRef`, and `WeakGlobalRef` are intentionally `!Send` and
//! `!Sync`.
//! The following examples must fail to compile:
//!
//! ```compile_fail
//! use jvmti_bindings::prelude::*;
//! fn assert_send<T: Send>() {}
//! fn test(env: JniEnv) { assert_send(env); }
//! ```
//!
//! ```compile_fail
//! use jvmti_bindings::prelude::*;
//! fn assert_send<T: Send>() {}
//! fn test(r: GlobalRef) { assert_send(r); }
//! ```
//!
//! ```compile_fail
//! use jvmti_bindings::prelude::*;
//! fn assert_send<T: Send>() {}
//! fn test(r: WeakGlobalRef) { assert_send(r); }
//! ```

use crate::mutf8;
use crate::sys::jni;
use crate::version::JniFeature;
use std::ffi::CStr;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::ptr::NonNull;
use std::rc::Rc;

// JNI tables grow by appending slots. Load only one pointer-sized field at a
// time so a JDK 8 table is never viewed as the complete JDK 28 structure.
macro_rules! jni_function {
    ($env:expr, $field:ident) => {{
        let table = $env.function_table_ptr();
        // Form one typed slot pointer without reading the complete newest-JDK
        // table. The allow is needed because this macro is used from both safe
        // and already-unsafe lexical contexts.
        #[allow(unused_unsafe)]
        let slot = unsafe { &raw const (*table).$field };
        $env.read_function_slot(slot)
    }};
}

macro_rules! jni_instance_call_a {
    ($name:ident, $slot:ident, $return_type:ty) => {
        #[doc = concat!("Call `", stringify!($slot), "` using a `jvalue` argument slice.")]
        ///
        /// # Safety
        ///
        /// The object, method ID, argument types, and return type must match a
        /// live method in this JNI environment and current thread.
        pub unsafe fn $name(
            &self,
            object: jni::jobject,
            method: jni::jmethodID,
            arguments: &[jni::jvalue],
        ) -> $return_type {
            let call = jni_function!(self, $slot);
            unsafe { call(self.env, object, method, arguments.as_ptr()) }
        }
    };
}

macro_rules! jni_nonvirtual_call_a {
    ($name:ident, $slot:ident, $return_type:ty) => {
        #[doc = concat!("Call `", stringify!($slot), "` using a `jvalue` argument slice.")]
        ///
        /// # Safety
        ///
        /// The object, declaring class, method ID, argument types, and return
        /// type must describe the same live nonvirtual method invocation.
        pub unsafe fn $name(
            &self,
            object: jni::jobject,
            class: jni::jclass,
            method: jni::jmethodID,
            arguments: &[jni::jvalue],
        ) -> $return_type {
            let call = jni_function!(self, $slot);
            unsafe { call(self.env, object, class, method, arguments.as_ptr()) }
        }
    };
}

macro_rules! jni_nonvirtual_bool_call_a {
    ($name:ident, $slot:ident) => {
        #[doc = concat!("Call `", stringify!($slot), "` using a `jvalue` argument slice.")]
        ///
        /// # Safety
        ///
        /// The object, declaring class, method ID, and argument types must
        /// describe the same live nonvirtual boolean method invocation.
        pub unsafe fn $name(
            &self,
            object: jni::jobject,
            class: jni::jclass,
            method: jni::jmethodID,
            arguments: &[jni::jvalue],
        ) -> bool {
            let call = jni_function!(self, $slot);
            unsafe { call(self.env, object, class, method, arguments.as_ptr()) != jni::JNI_FALSE }
        }
    };
}

macro_rules! jni_static_call_a {
    ($name:ident, $slot:ident, $return_type:ty) => {
        #[doc = concat!("Call `", stringify!($slot), "` using a `jvalue` argument slice.")]
        ///
        /// # Safety
        ///
        /// The class, method ID, argument types, and return type must match a
        /// live static method in this JNI environment and current thread.
        pub unsafe fn $name(
            &self,
            class: jni::jclass,
            method: jni::jmethodID,
            arguments: &[jni::jvalue],
        ) -> $return_type {
            let call = jni_function!(self, $slot);
            unsafe { call(self.env, class, method, arguments.as_ptr()) }
        }
    };
}

macro_rules! jni_static_bool_call_a {
    ($name:ident, $slot:ident) => {
        #[doc = concat!("Call `", stringify!($slot), "` using a `jvalue` argument slice.")]
        ///
        /// # Safety
        ///
        /// The class, method ID, and argument types must match a live static
        /// boolean method in this JNI environment and current thread.
        pub unsafe fn $name(
            &self,
            class: jni::jclass,
            method: jni::jmethodID,
            arguments: &[jni::jvalue],
        ) -> bool {
            let call = jni_function!(self, $slot);
            unsafe { call(self.env, class, method, arguments.as_ptr()) != jni::JNI_FALSE }
        }
    };
}

macro_rules! jni_get_field {
    ($name:ident, $slot:ident, $owner:ident, $return_type:ty) => {
        #[doc = concat!("Read a field through `", stringify!($slot), "`.")]
        ///
        /// # Safety
        ///
        /// The owner and field ID must be live, compatible, and valid for this
        /// JNI environment and current thread.
        pub unsafe fn $name(&self, $owner: jni::$owner, field: jni::jfieldID) -> $return_type {
            let get = jni_function!(self, $slot);
            unsafe { get(self.env, $owner, field) }
        }
    };
}

macro_rules! jni_get_bool_field {
    ($name:ident, $slot:ident, $owner:ident) => {
        #[doc = concat!("Read a boolean field through `", stringify!($slot), "`.")]
        ///
        /// # Safety
        ///
        /// The owner and field ID must be live, compatible, and valid for this
        /// JNI environment and current thread.
        pub unsafe fn $name(&self, $owner: jni::$owner, field: jni::jfieldID) -> bool {
            let get = jni_function!(self, $slot);
            unsafe { get(self.env, $owner, field) != jni::JNI_FALSE }
        }
    };
}

macro_rules! jni_set_field {
    ($name:ident, $slot:ident, $owner:ident, $value_type:ty) => {
        #[doc = concat!("Write a field through `", stringify!($slot), "`.")]
        ///
        /// # Safety
        ///
        /// The owner and field ID must be live and compatible, and the field
        /// must be legally mutable under the active Java runtime policy.
        pub unsafe fn $name(&self, $owner: jni::$owner, field: jni::jfieldID, value: $value_type) {
            let set = jni_function!(self, $slot);
            unsafe { set(self.env, $owner, field, value) }
        }
    };
}

macro_rules! jni_set_bool_field {
    ($name:ident, $slot:ident, $owner:ident) => {
        #[doc = concat!("Write a boolean field through `", stringify!($slot), "`.")]
        ///
        /// # Safety
        ///
        /// The owner and field ID must be live and compatible, and the field
        /// must be legally mutable under the active Java runtime policy.
        pub unsafe fn $name(&self, $owner: jni::$owner, field: jni::jfieldID, value: bool) {
            let set = jni_function!(self, $slot);
            unsafe {
                set(
                    self.env,
                    $owner,
                    field,
                    if value { jni::JNI_TRUE } else { jni::JNI_FALSE },
                )
            }
        }
    };
}

macro_rules! jni_new_primitive_array {
    ($name:ident, $slot:ident, $array_type:ty) => {
        #[doc = concat!("Create an array through `", stringify!($slot), "`.")]
        pub fn $name(&self, length: jni::jsize) -> Option<$array_type> {
            let create = jni_function!(self, $slot);
            let array = unsafe { create(self.env, length) };
            (!array.is_null()).then_some(array)
        }
    };
}

macro_rules! jni_primitive_array_elements {
    ($get_name:ident, $get_slot:ident, $release_slot:ident, $array_type:ty, $element_type:ty) => {
        #[doc = concat!("Acquire array storage through `", stringify!($get_slot), "`.")]
        ///
        /// The returned allocation-free guard releases the lease exactly once.
        ///
        /// # Safety
        ///
        /// The array must be live, have the matching primitive element type,
        /// and belong to this VM and current JNI attachment.
        pub unsafe fn $get_name(
            &self,
            array: $array_type,
        ) -> Option<PrimitiveArrayElements<'_, $element_type>> {
            let raw_length = unsafe { self.get_array_length(array) };
            let length = usize::try_from(raw_length).ok()?;
            let mut is_copy = jni::JNI_FALSE;
            let get = jni_function!(self, $get_slot);
            let release = jni_function!(self, $release_slot);
            let elements = unsafe { get(self.env, array, &mut is_copy) };
            Some(PrimitiveArrayElements::new(
                self,
                array,
                NonNull::new(elements)?,
                length,
                is_copy != jni::JNI_FALSE,
                release,
            ))
        }
    };
}

macro_rules! jni_primitive_array_region {
    ($get_name:ident, $get_slot:ident, $set_name:ident, $set_slot:ident,
     $array_type:ty, $element_type:ty) => {
        #[doc = concat!("Read an array region through `", stringify!($get_slot), "`.")]
        ///
        /// # Safety
        ///
        /// The array and range must be valid, and `length` must not exceed the
        /// writable length of `buffer`.
        pub unsafe fn $get_name(
            &self,
            array: $array_type,
            start: jni::jsize,
            length: jni::jsize,
            buffer: &mut [$element_type],
        ) {
            let get = jni_function!(self, $get_slot);
            unsafe { get(self.env, array, start, length, buffer.as_mut_ptr()) }
        }

        #[doc = concat!("Write an array region through `", stringify!($set_slot), "`.")]
        ///
        /// # Safety
        ///
        /// The array and range must be valid, and `length` must not exceed the
        /// readable length of `buffer`.
        pub unsafe fn $set_name(
            &self,
            array: $array_type,
            start: jni::jsize,
            length: jni::jsize,
            buffer: &[$element_type],
        ) {
            let set = jni_function!(self, $set_slot);
            unsafe { set(self.env, array, start, length, buffer.as_ptr()) }
        }
    };
}

/// A JNI operation is newer than the active JVM's native interface.
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct JniVersionError {
    pub feature: &'static str,
    pub required: jni::jint,
    pub actual: jni::jint,
}

/// Owning lease over primitive-array elements returned by JNI.
///
/// The guard is stack-only and allocation-free. Mutable changes are copied
/// back, when required by the JVM, before the native lease is released on
/// drop. Use [`Self::abort`] to request that copied storage not be written
/// back; JNI cannot undo writes when the JVM returned pinned backing storage.
pub struct PrimitiveArrayElements<'env, T> {
    env: &'env JniEnv,
    array: jni::jarray,
    elements: NonNull<T>,
    length: usize,
    is_copy: bool,
    release: unsafe extern "system" fn(*mut jni::JNIEnv, jni::jarray, *mut T, jni::jint),
    active: bool,
}

impl<'env, T> PrimitiveArrayElements<'env, T> {
    fn new(
        env: &'env JniEnv,
        array: jni::jarray,
        elements: NonNull<T>,
        length: usize,
        is_copy: bool,
        release: unsafe extern "system" fn(*mut jni::JNIEnv, jni::jarray, *mut T, jni::jint),
    ) -> Self {
        Self {
            env,
            array,
            elements,
            length,
            is_copy,
            release,
            active: true,
        }
    }

    /// Whether the JVM returned a copy rather than pinned backing storage.
    pub fn is_copy(&self) -> bool {
        self.is_copy
    }

    /// Return the leased native pointer without transferring ownership.
    pub fn as_ptr(&self) -> *mut T {
        self.elements.as_ptr()
    }

    /// Copy pending modifications back while retaining the lease.
    ///
    /// The guard still releases the lease on drop. Most callers should simply
    /// mutate the slice and allow normal drop to copy back and release once.
    pub fn commit(&mut self) {
        if self.active {
            unsafe {
                (self.release)(
                    self.env.env,
                    self.array,
                    self.elements.as_ptr(),
                    jni::JNI_COMMIT,
                )
            }
        }
    }

    /// Copy pending changes back and release the lease immediately.
    pub fn close(mut self) {
        self.release_with(0);
    }

    /// Release immediately and request that copied storage not be written back.
    ///
    /// This cannot roll back writes when [`Self::is_copy`] is false because
    /// those writes may already have modified pinned Java-array storage.
    pub fn abort(mut self) {
        self.release_with(jni::JNI_ABORT);
    }

    fn release_with(&mut self, mode: jni::jint) {
        if self.active {
            self.active = false;
            unsafe { (self.release)(self.env.env, self.array, self.elements.as_ptr(), mode) }
        }
    }
}

impl<T> Deref for PrimitiveArrayElements<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.elements.as_ptr(), self.length) }
    }
}

impl<T> DerefMut for PrimitiveArrayElements<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::slice::from_raw_parts_mut(self.elements.as_ptr(), self.length) }
    }
}

impl<T> Drop for PrimitiveArrayElements<'_, T> {
    fn drop(&mut self) {
        self.release_with(0);
    }
}

/// Owning critical-region lease over a Java string's UTF-16 code units.
///
/// The guard is allocation-free and releases the region exactly once on drop.
/// JNI calls, blocking operations, and arbitrary native work are forbidden
/// while this critical lease is held.
pub struct StringCritical<'env> {
    env: &'env JniEnv,
    string: jni::jstring,
    characters: NonNull<jni::jchar>,
    length: usize,
    is_copy: bool,
    release: unsafe extern "system" fn(*mut jni::JNIEnv, jni::jstring, *const jni::jchar),
    active: bool,
}

impl StringCritical<'_> {
    /// Whether the JVM returned a copy rather than pinned backing storage.
    pub fn is_copy(&self) -> bool {
        self.is_copy
    }

    /// Return the leased native pointer without transferring ownership.
    pub fn as_ptr(&self) -> *const jni::jchar {
        self.characters.as_ptr()
    }

    /// Release the critical region immediately.
    pub fn close(mut self) {
        self.release_once();
    }

    fn release_once(&mut self) {
        if self.active {
            self.active = false;
            unsafe { (self.release)(self.env.env, self.string, self.characters.as_ptr()) }
        }
    }
}

impl Deref for StringCritical<'_> {
    type Target = [jni::jchar];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.characters.as_ptr(), self.length) }
    }
}

impl Drop for StringCritical<'_> {
    fn drop(&mut self) {
        self.release_once();
    }
}

/// Owning JNI critical-region lease over a primitive array.
///
/// JNI does not report the element type through this operation, so the guard
/// exposes an opaque native pointer and the array's element count. The caller
/// must interpret the storage according to the Java array's actual type.
pub struct PrimitiveArrayCritical<'env> {
    env: &'env JniEnv,
    array: jni::jarray,
    elements: NonNull<std::ffi::c_void>,
    element_count: usize,
    is_copy: bool,
    release:
        unsafe extern "system" fn(*mut jni::JNIEnv, jni::jarray, *mut std::ffi::c_void, jni::jint),
    active: bool,
}

impl PrimitiveArrayCritical<'_> {
    /// Number of Java array elements represented by this opaque lease.
    pub fn element_count(&self) -> usize {
        self.element_count
    }

    /// Whether the JVM returned a copy rather than pinned backing storage.
    pub fn is_copy(&self) -> bool {
        self.is_copy
    }

    /// Return the opaque leased native pointer without transferring ownership.
    pub fn as_ptr(&self) -> *mut std::ffi::c_void {
        self.elements.as_ptr()
    }

    /// Copy pending changes back and release the critical region immediately.
    pub fn close(mut self) {
        self.release_with(0);
    }

    /// Release immediately and request that copied storage not be written back.
    ///
    /// This cannot roll back writes when [`Self::is_copy`] is false because
    /// those writes may already have modified pinned Java-array storage.
    pub fn abort(mut self) {
        self.release_with(jni::JNI_ABORT);
    }

    fn release_with(&mut self, mode: jni::jint) {
        if self.active {
            self.active = false;
            unsafe { (self.release)(self.env.env, self.array, self.elements.as_ptr(), mode) }
        }
    }
}

impl Drop for PrimitiveArrayCritical<'_> {
    fn drop(&mut self) {
        self.release_with(0);
    }
}

/// Owning JNI local-reference frame.
///
/// The frame is popped exactly once on drop unless [`Self::pop`] or
/// [`Self::close`] pops it explicitly. Local references created in this frame
/// must not be used after the guard is consumed or dropped.
pub struct LocalFrame<'env> {
    env: &'env JniEnv,
    active: bool,
}

impl LocalFrame<'_> {
    /// Pop the frame and preserve one local reference in the previous frame.
    ///
    /// # Safety
    ///
    /// `result` must be null or a local reference belonging to this frame on
    /// the current JNI thread. No reference created in the frame may be used
    /// after this call except the returned promoted reference.
    pub unsafe fn pop(mut self, result: jni::jobject) -> jni::jobject {
        self.active = false;
        unsafe { self.env.pop_local_frame_raw(result) }
    }

    /// Pop the frame without preserving a local reference.
    pub fn close(mut self) {
        self.active = false;
        unsafe {
            self.env.pop_local_frame_raw(ptr::null_mut());
        }
    }
}

impl Drop for LocalFrame<'_> {
    fn drop(&mut self) {
        if self.active {
            self.active = false;
            unsafe {
                self.env.pop_local_frame_raw(ptr::null_mut());
            }
        }
    }
}

/// Entered Java object monitor, exited automatically on drop.
///
/// The guard is tied to the current thread's `JniEnv`; it is neither `Send`
/// nor `Sync`. Use [`Self::exit`] when the JNI status must be observed.
pub struct JavaMonitorGuard<'env> {
    env: &'env JniEnv,
    object: jni::jobject,
    active: bool,
}

impl JavaMonitorGuard<'_> {
    /// Exit the monitor immediately instead of waiting for drop.
    ///
    /// A failed explicit exit leaves the guard active so its destructor makes
    /// one best-effort retry, matching the crate's other owning guards.
    pub fn exit(mut self) -> Result<(), jni::jint> {
        let result = unsafe { self.env.monitor_exit_raw(self.object) };
        if result.is_ok() {
            self.active = false;
        }
        result
    }
}

impl Drop for JavaMonitorGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            self.active = false;
            let _ = unsafe { self.env.monitor_exit_raw(self.object) };
        }
    }
}

impl fmt::Display for JniVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} requires JNI version {:#010x}, active JVM reports {:#010x}",
            self.feature, self.required, self.actual
        )
    }
}

impl std::error::Error for JniVersionError {}

/// Safe wrapper around a JNI environment pointer.
///
/// This struct provides ergonomic access to JNI functions with proper
/// error handling and Rust-friendly types.
///
/// # Thread Safety
///
/// A `JniEnv` is tied to a specific thread and cannot be sent across threads.
/// Each JVM thread has its own JNI environment.
pub struct JniEnv {
    env: *mut jni::JNIEnv,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl JniEnv {
    /// Creates a JniEnv wrapper from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure the pointer is valid and comes from the current thread.
    pub unsafe fn from_raw(env: *mut jni::JNIEnv) -> Self {
        JniEnv {
            env,
            _not_send_sync: PhantomData,
        }
    }

    /// Returns the raw JNI environment pointer.
    pub fn raw(&self) -> *mut jni::JNIEnv {
        self.env
    }

    fn function_table_ptr(&self) -> *const jni::JNINativeInterface_ {
        debug_assert!(!self.env.is_null());
        let table = unsafe { *self.env };
        debug_assert!(!table.is_null());
        table
    }

    fn read_function_slot<F: Copy>(&self, slot: *const F) -> F {
        // The raw constructor requires a valid environment. The caller macro
        // forms `slot` from that checked contract and reads no adjacent bytes.
        unsafe { slot.read() }
    }

    /// Returns the JavaVM for this environment.
    pub fn get_java_vm(&self) -> Result<*mut jni::JavaVM, jni::jint> {
        let mut vm: *mut jni::JavaVM = ptr::null_mut();
        unsafe {
            let get_java_vm = jni_function!(self, GetJavaVM);
            let result = get_java_vm(self.env, &mut vm);
            if result == 0 { Ok(vm) } else { Err(result) }
        }
    }

    // =========================================================================
    // Version
    // =========================================================================

    /// Returns the JNI version.
    pub fn get_version(&self) -> jni::jint {
        unsafe {
            let get_version = jni_function!(self, GetVersion);
            get_version(self.env)
        }
    }

    /// Returns whether the active JNI function table includes `required`.
    pub fn supports_version(&self, required: jni::jint) -> bool {
        self.get_version() >= required
    }

    /// Returns whether the active JNI table contains an additive feature.
    pub fn supports_feature(&self, feature: JniFeature) -> bool {
        self.supports_version(feature.required_version())
    }

    fn require_version(
        &self,
        feature: &'static str,
        required: jni::jint,
    ) -> Result<(), JniVersionError> {
        let actual = self.get_version();
        if actual >= required {
            Ok(())
        } else {
            Err(JniVersionError {
                feature,
                required,
                actual,
            })
        }
    }

    fn require_feature(&self, feature: JniFeature) -> Result<(), JniVersionError> {
        self.require_version(feature.operation(), feature.required_version())
    }

    /// Return the module that defines `class` (JDK 9+).
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current
    /// thread, and operation for the duration required by the JNI
    /// specification.
    pub unsafe fn get_module(&self, class: jni::jclass) -> Result<jni::jobject, JniVersionError> {
        self.require_feature(JniFeature::Modules)?;
        unsafe {
            let get_module = jni_function!(self, GetModule);
            Ok(get_module(self.env, class))
        }
    }

    /// Report whether `object` is a virtual thread (JDK 19+).
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current
    /// thread, and operation for the duration required by the JNI
    /// specification.
    pub unsafe fn is_virtual_thread(&self, object: jni::jobject) -> Result<bool, JniVersionError> {
        self.require_feature(JniFeature::VirtualThreads)?;
        unsafe {
            let is_virtual_thread = jni_function!(self, IsVirtualThread);
            Ok(is_virtual_thread(self.env, object) != jni::JNI_FALSE)
        }
    }

    /// Return the modified UTF-8 byte length without the legacy `jsize` limit
    /// (JDK 24+).
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current
    /// thread, and operation for the duration required by the JNI
    /// specification.
    pub unsafe fn get_string_utf_length_as_long(
        &self,
        string: jni::jstring,
    ) -> Result<jni::jlong, JniVersionError> {
        self.require_feature(JniFeature::ModifiedUtf8LongLength)?;
        unsafe {
            let get_length = jni_function!(self, GetStringUTFLengthAsLong);
            Ok(get_length(self.env, string))
        }
    }

    /// Reports whether an object has identity under the JDK 28 value-object model.
    ///
    /// The appended JNI table slot is read only after `GetVersion` confirms that
    /// the active table is from JDK 28 or newer.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current
    /// thread, and operation for the duration required by the JNI
    /// specification.
    pub unsafe fn has_identity(&self, object: jni::jobject) -> Result<bool, JniVersionError> {
        self.require_feature(JniFeature::ValueObjectIdentity)?;
        unsafe {
            let has_identity = jni_function!(self, HasIdentity);
            Ok(has_identity(self.env, object) != jni::JNI_FALSE)
        }
    }

    // =========================================================================
    // Class Operations
    // =========================================================================

    /// Finds a class by its fully qualified name.
    ///
    /// The name should use '/' as package separator (e.g., "java/lang/String").
    pub fn find_class(&self, name: &str) -> Option<jni::jclass> {
        let c_name = mutf8::encode_cstring(name);
        self.find_class_cstr(&c_name)
    }

    /// Finds a class without allocating a temporary C string.
    ///
    /// `name` must already contain NUL-terminated Java Modified UTF-8. This is
    /// the preferred form for static ASCII names, for example
    /// `env.find_class_cstr(c"java/lang/String")`.
    pub fn find_class_cstr(&self, name: &CStr) -> Option<jni::jclass> {
        mutf8::validate(name.to_bytes()).ok()?;
        unsafe {
            let vtable = *self.env;
            let cls = ((*vtable).FindClass)(self.env, name.as_ptr());
            if cls.is_null() { None } else { Some(cls) }
        }
    }

    /// Defines a class from raw classfile bytes.
    ///
    /// `name` must be the internal JVM class name, such as `com/example/Helper`.
    /// The returned class is a local reference and must be deleted by the caller.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn define_class(
        &self,
        name: &str,
        loader: jni::jobject,
        bytes: &[u8],
    ) -> Option<jni::jclass> {
        if bytes.len() > jni::jsize::MAX as usize {
            return None;
        }
        let c_name = mutf8::encode_cstring(name);
        // SAFETY: Forwarded from this function's loader contract.
        unsafe { self.define_class_cstr(&c_name, loader, bytes) }
    }

    /// Defines a class without allocating a temporary C string for its name.
    /// # Safety
    ///
    /// `loader` must be null or a valid loader reference for this environment.
    pub unsafe fn define_class_cstr(
        &self,
        name: &CStr,
        loader: jni::jobject,
        bytes: &[u8],
    ) -> Option<jni::jclass> {
        if bytes.len() > jni::jsize::MAX as usize || mutf8::validate(name.to_bytes()).is_err() {
            return None;
        }
        unsafe {
            let vtable = *self.env;
            let cls = ((*vtable).DefineClass)(
                self.env,
                name.as_ptr(),
                loader,
                bytes.as_ptr() as *const jni::jbyte,
                bytes.len() as jni::jsize,
            );
            if cls.is_null() { None } else { Some(cls) }
        }
    }

    /// Convert a reflected `java.lang.reflect.Method` or `Constructor` to its
    /// JNI method identifier.
    /// # Safety
    ///
    /// `method` must be a live reflected method or constructor object from
    /// this VM and current JNI thread.
    pub unsafe fn from_reflected_method(&self, method: jni::jobject) -> jni::jmethodID {
        let convert = jni_function!(self, FromReflectedMethod);
        unsafe { convert(self.env, method) }
    }

    /// Convert a reflected `java.lang.reflect.Field` to its JNI field ID.
    /// # Safety
    ///
    /// `field` must be a live reflected field object from this VM and current
    /// JNI thread.
    pub unsafe fn from_reflected_field(&self, field: jni::jobject) -> jni::jfieldID {
        let convert = jni_function!(self, FromReflectedField);
        unsafe { convert(self.env, field) }
    }

    /// Convert a JNI method ID to a reflected method or constructor object.
    /// # Safety
    ///
    /// `class` and `method` must be live, compatible handles from this VM.
    pub unsafe fn to_reflected_method(
        &self,
        class: jni::jclass,
        method: jni::jmethodID,
        is_static: bool,
    ) -> Option<jni::jobject> {
        let convert = jni_function!(self, ToReflectedMethod);
        let reflected = unsafe {
            convert(
                self.env,
                class,
                method,
                if is_static {
                    jni::JNI_TRUE
                } else {
                    jni::JNI_FALSE
                },
            )
        };
        (!reflected.is_null()).then_some(reflected)
    }

    /// Convert a JNI field ID to a reflected field object.
    /// # Safety
    ///
    /// `class` and `field` must be live, compatible handles from this VM.
    pub unsafe fn to_reflected_field(
        &self,
        class: jni::jclass,
        field: jni::jfieldID,
        is_static: bool,
    ) -> Option<jni::jobject> {
        let convert = jni_function!(self, ToReflectedField);
        let reflected = unsafe {
            convert(
                self.env,
                class,
                field,
                if is_static {
                    jni::JNI_TRUE
                } else {
                    jni::JNI_FALSE
                },
            )
        };
        (!reflected.is_null()).then_some(reflected)
    }

    /// Gets the superclass of a class.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_superclass(&self, cls: jni::jclass) -> Option<jni::jclass> {
        unsafe {
            let vtable = *self.env;
            let super_cls = ((*vtable).GetSuperclass)(self.env, cls);
            if super_cls.is_null() {
                None
            } else {
                Some(super_cls)
            }
        }
    }

    /// Checks if `cls1` can be assigned to `cls2`.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn is_assignable_from(&self, cls1: jni::jclass, cls2: jni::jclass) -> bool {
        unsafe {
            let vtable = *self.env;
            ((*vtable).IsAssignableFrom)(self.env, cls1, cls2) != 0
        }
    }

    /// Gets the class of an object.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_object_class(&self, obj: jni::jobject) -> jni::jclass {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetObjectClass)(self.env, obj)
        }
    }

    /// Checks if an object is an instance of a class.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn is_instance_of(&self, obj: jni::jobject, cls: jni::jclass) -> bool {
        unsafe {
            let vtable = *self.env;
            ((*vtable).IsInstanceOf)(self.env, obj, cls) != 0
        }
    }

    // =========================================================================
    // ClassLoader and JPMS Helpers
    // =========================================================================

    /// Returns `ClassLoader.getParent()` for a classloader object.
    ///
    /// A null return means either the parent is the bootstrap loader or the
    /// lookup/call failed. Check/clear pending exceptions if you need to
    /// distinguish those cases. The returned object is a local reference.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn class_loader_parent(&self, loader: jni::jobject) -> Option<jni::jobject> {
        if loader.is_null() {
            return None;
        }
        // SAFETY: `loader` is covered by this function's contract; all other
        // handles below are local references created by this environment.
        unsafe {
            let class_loader_class = self.find_class_cstr(c"java/lang/ClassLoader")?;
            let Some(method) = self.get_method_id_cstr(
                class_loader_class,
                c"getParent",
                c"()Ljava/lang/ClassLoader;",
            ) else {
                self.delete_local_ref(class_loader_class);
                return None;
            };
            let parent = self.call_object_method(loader, method, &[]);
            self.delete_local_ref(class_loader_class);
            if parent.is_null() { None } else { Some(parent) }
        }
    }

    /// Returns `ClassLoader.getSystemClassLoader()`.
    ///
    /// The returned object is a local reference.
    pub fn system_class_loader(&self) -> Option<jni::jobject> {
        let class_loader_class = self.find_class_cstr(c"java/lang/ClassLoader")?;
        // Every handle below was produced by this environment in this call.
        let loader = unsafe {
            let Some(method) = self.get_static_method_id_cstr(
                class_loader_class,
                c"getSystemClassLoader",
                c"()Ljava/lang/ClassLoader;",
            ) else {
                self.delete_local_ref(class_loader_class);
                return None;
            };
            let loader = self.call_static_object_method(class_loader_class, method, &[]);
            self.delete_local_ref(class_loader_class);
            loader
        };
        if loader.is_null() { None } else { Some(loader) }
    }

    /// Returns `Module.getName()`.
    ///
    /// Unnamed modules return `None`. The helper also returns `None` if the
    /// reflection lookup/call fails.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn module_name(&self, module: jni::jobject) -> Option<String> {
        if module.is_null() {
            return None;
        }
        // SAFETY: `module` is covered by this function's contract; all other
        // handles below are local references created by this environment.
        unsafe {
            let module_class = self.get_object_class(module);
            let Some(method) =
                self.get_method_id_cstr(module_class, c"getName", c"()Ljava/lang/String;")
            else {
                self.delete_local_ref(module_class);
                return None;
            };
            let name_obj = self.call_object_method(module, method, &[]);
            self.delete_local_ref(module_class);
            if name_obj.is_null() {
                return None;
            }
            let name = self.get_string_utf(name_obj as jni::jstring);
            self.delete_local_ref(name_obj);
            name
        }
    }

    /// Returns `Module.getPackages()` as dotted Java package names.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn module_packages(&self, module: jni::jobject) -> Option<Vec<String>> {
        if module.is_null() {
            return None;
        }
        // SAFETY: `module` is covered by this function's contract; all other
        // handles below are local references created by this environment.
        unsafe {
            let module_class = self.get_object_class(module);
            let Some(method) =
                self.get_method_id_cstr(module_class, c"getPackages", c"()Ljava/util/Set;")
            else {
                self.delete_local_ref(module_class);
                return None;
            };
            let package_set = self.call_object_method(module, method, &[]);
            self.delete_local_ref(module_class);
            if package_set.is_null() {
                return Some(Vec::new());
            }
            let set_class = self.get_object_class(package_set);
            let Some(to_array) =
                self.get_method_id_cstr(set_class, c"toArray", c"()[Ljava/lang/Object;")
            else {
                self.delete_local_ref(set_class);
                self.delete_local_ref(package_set);
                return None;
            };
            let array = self.call_object_method(package_set, to_array, &[]) as jni::jobjectArray;
            self.delete_local_ref(set_class);
            self.delete_local_ref(package_set);
            if array.is_null() {
                return Some(Vec::new());
            }
            let len = self.get_array_length(array);
            let mut packages = Vec::new();
            for index in 0..len {
                let element = self.get_object_array_element(array, index);
                if !element.is_null() {
                    if let Some(package_name) = self.get_string_utf(element as jni::jstring) {
                        packages.push(package_name);
                    }
                    self.delete_local_ref(element);
                }
            }
            self.delete_local_ref(array);
            Some(packages)
        }
    }

    /// Returns `Module.getClassLoader()`.
    ///
    /// A null return means the module is associated with the bootstrap loader or
    /// the lookup/call failed. The returned object is a local reference.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn module_class_loader(&self, module: jni::jobject) -> Option<jni::jobject> {
        if module.is_null() {
            return None;
        }
        // SAFETY: `module` is covered by this function's contract; all other
        // handles below are local references created by this environment.
        unsafe {
            let module_class = self.get_object_class(module);
            let Some(method) = self.get_method_id_cstr(
                module_class,
                c"getClassLoader",
                c"()Ljava/lang/ClassLoader;",
            ) else {
                self.delete_local_ref(module_class);
                return None;
            };
            let loader = self.call_object_method(module, method, &[]);
            self.delete_local_ref(module_class);
            if loader.is_null() { None } else { Some(loader) }
        }
    }

    /// Returns `Module.canRead(other)`.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn module_can_read(&self, module: jni::jobject, other: jni::jobject) -> bool {
        if module.is_null() || other.is_null() {
            return false;
        }
        // SAFETY: Both module handles are covered by this function's contract;
        // all other handles are local references from this environment.
        unsafe {
            let module_class = self.get_object_class(module);
            let Some(method) =
                self.get_method_id_cstr(module_class, c"canRead", c"(Ljava/lang/Module;)Z")
            else {
                self.delete_local_ref(module_class);
                return false;
            };
            let args = [jni::jvalue { l: other }];
            let can_read = self.call_boolean_method(module, method, &args);
            self.delete_local_ref(module_class);
            can_read
        }
    }

    /// Returns `Module.isExported(package_name, other)`.
    ///
    /// `package_name` must use dotted Java package syntax.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn module_is_exported_to(
        &self,
        module: jni::jobject,
        package_name: &str,
        other: jni::jobject,
    ) -> bool {
        // SAFETY: Forwarded from this function's handle contract.
        unsafe { self.module_package_access(module, package_name, other, c"isExported") }
    }

    /// Returns `Module.isOpen(package_name, other)`.
    ///
    /// `package_name` must use dotted Java package syntax.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn module_is_open_to(
        &self,
        module: jni::jobject,
        package_name: &str,
        other: jni::jobject,
    ) -> bool {
        // SAFETY: Forwarded from this function's handle contract.
        unsafe { self.module_package_access(module, package_name, other, c"isOpen") }
    }

    unsafe fn module_package_access(
        &self,
        module: jni::jobject,
        package_name: &str,
        other: jni::jobject,
        method_name: &CStr,
    ) -> bool {
        if module.is_null() || other.is_null() {
            return false;
        }
        // SAFETY: Both module handles are covered by this function's contract;
        // all other handles are local references from this environment.
        unsafe {
            let module_class = self.get_object_class(module);
            let Some(method) = self.get_method_id_cstr(
                module_class,
                method_name,
                c"(Ljava/lang/String;Ljava/lang/Module;)Z",
            ) else {
                self.delete_local_ref(module_class);
                return false;
            };
            let Some(package) = self.new_string_utf(package_name) else {
                self.delete_local_ref(module_class);
                return false;
            };
            let args = [jni::jvalue { l: package }, jni::jvalue { l: other }];
            let result = self.call_boolean_method(module, method, &args);
            self.delete_local_ref(package);
            self.delete_local_ref(module_class);
            result
        }
    }

    // =========================================================================
    // Exception Handling
    // =========================================================================

    /// Checks if an exception is pending.
    pub fn exception_check(&self) -> bool {
        unsafe {
            let vtable = *self.env;
            ((*vtable).ExceptionCheck)(self.env) != 0
        }
    }

    /// Clears any pending exception.
    pub fn exception_clear(&self) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).ExceptionClear)(self.env);
        }
    }

    /// Prints the pending exception and stack trace to stderr.
    pub fn exception_describe(&self) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).ExceptionDescribe)(self.env);
        }
    }

    /// Gets the pending exception (if any).
    pub fn exception_occurred(&self) -> Option<jni::jthrowable> {
        unsafe {
            let vtable = *self.env;
            let exc = ((*vtable).ExceptionOccurred)(self.env);
            if exc.is_null() { None } else { Some(exc) }
        }
    }

    /// Report a fatal JVM error and abort the process.
    ///
    /// JNI specifies that `FatalError` does not return. The wrapper aborts if
    /// a non-conforming VM returns unexpectedly.
    pub fn fatal_error(&self, message: &str) -> ! {
        let message = mutf8::encode_cstring(message);
        let fatal = jni_function!(self, FatalError);
        unsafe { fatal(self.env, message.as_ptr()) };
        std::process::abort()
    }

    /// Throws an exception.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn throw(&self, obj: jni::jthrowable) -> Result<(), jni::jint> {
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).Throw)(self.env, obj);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    /// Throws a new exception of the specified class with the given message.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn throw_new(&self, cls: jni::jclass, msg: &str) -> Result<(), jni::jint> {
        let c_msg = mutf8::encode_cstring(msg);
        // SAFETY: Forwarded from this function's class-handle contract.
        unsafe { self.throw_new_cstr(cls, &c_msg) }
    }

    /// Throws a new exception without allocating a temporary C string.
    /// # Safety
    ///
    /// `cls` must be a valid exception class for this environment and `msg`
    /// must contain NUL-terminated Java Modified UTF-8.
    pub unsafe fn throw_new_cstr(&self, cls: jni::jclass, msg: &CStr) -> Result<(), jni::jint> {
        if mutf8::validate(msg.to_bytes()).is_err() {
            return Err(jni::JNI_EINVAL);
        }
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).ThrowNew)(self.env, cls, msg.as_ptr());
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    // =========================================================================
    // String Operations
    // =========================================================================

    /// Creates a new Java string from a Rust string.
    pub fn new_string_utf(&self, s: &str) -> Option<jni::jstring> {
        let c_str = mutf8::encode_cstring(s);
        self.new_string_utf_cstr(&c_str)
    }

    /// Creates a Java Modified UTF-8 string without a temporary allocation.
    ///
    /// `value` must already be encoded as NUL-terminated Java Modified UTF-8,
    /// not ordinary UTF-8. ASCII literals satisfy both encodings.
    pub fn new_string_utf_cstr(&self, value: &CStr) -> Option<jni::jstring> {
        mutf8::validate(value.to_bytes()).ok()?;
        unsafe {
            let vtable = *self.env;
            let jstr = ((*vtable).NewStringUTF)(self.env, value.as_ptr());
            if jstr.is_null() { None } else { Some(jstr) }
        }
    }

    /// Creates a new Java string from a Rust string using UTF-16.
    pub fn new_string(&self, s: &str) -> Option<jni::jstring> {
        let utf16: Vec<jni::jchar> = s.encode_utf16().collect();
        self.new_string_utf16(&utf16)
    }

    /// Creates a Java string from pre-encoded UTF-16 without a Rust allocation.
    pub fn new_string_utf16(&self, utf16: &[jni::jchar]) -> Option<jni::jstring> {
        if utf16.len() > jni::jsize::MAX as usize {
            return None;
        }
        unsafe {
            let vtable = *self.env;
            let jstr = ((*vtable).NewString)(self.env, utf16.as_ptr(), utf16.len() as jni::jsize);
            if jstr.is_null() { None } else { Some(jstr) }
        }
    }

    /// Gets a Rust string from a Java string.
    ///
    /// Returns `None` if the string is null or contains invalid modified UTF-8.
    /// For full-fidelity Unicode (including embedded nulls), use
    /// [`Self::get_string`].
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_string_utf(&self, s: jni::jstring) -> Option<String> {
        if s.is_null() {
            return None;
        }
        unsafe {
            let vtable = *self.env;
            let chars = ((*vtable).GetStringUTFChars)(self.env, s, ptr::null_mut());
            if chars.is_null() {
                return None;
            }
            let result = mutf8::decode_cstr(CStr::from_ptr(chars)).ok();
            ((*vtable).ReleaseStringUTFChars)(self.env, s, chars);
            result
        }
    }

    /// Gets the exact UTF-16 code units from a Java string.
    ///
    /// Unlike Rust strings, Java strings may contain unpaired surrogates. This
    /// method preserves them without replacement.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_string_utf16(&self, s: jni::jstring) -> Option<Vec<jni::jchar>> {
        if s.is_null() {
            return None;
        }
        unsafe {
            let vtable = *self.env;
            let chars = ((*vtable).GetStringChars)(self.env, s, ptr::null_mut());
            if chars.is_null() {
                return None;
            }
            let raw_len = ((*vtable).GetStringLength)(self.env, s);
            let result = usize::try_from(raw_len)
                .ok()
                .map(|len| std::slice::from_raw_parts(chars, len).to_vec());
            ((*vtable).ReleaseStringChars)(self.env, s, chars);
            result
        }
    }

    /// Gets a Rust string from a Java string using UTF-16.
    ///
    /// Returns `None` if the string is null or contains unpaired UTF-16
    /// surrogates. Use [`Self::get_string_utf16`] for exact Java-string data or
    /// [`Self::get_string_lossy`] when replacement is acceptable.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_string(&self, s: jni::jstring) -> Option<String> {
        let utf16 = unsafe { self.get_string_utf16(s) }?;
        String::from_utf16(&utf16).ok()
    }

    /// Gets a Rust string from Java UTF-16, replacing unpaired surrogates.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_string_lossy(&self, s: jni::jstring) -> Option<String> {
        let utf16 = unsafe { self.get_string_utf16(s) }?;
        Some(String::from_utf16_lossy(&utf16))
    }

    /// Gets the UTF-8 length of a Java string.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_string_utf_length(&self, s: jni::jstring) -> jni::jsize {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetStringUTFLength)(self.env, s)
        }
    }

    /// Gets the length of a Java string (in UTF-16 code units).
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_string_length(&self, s: jni::jstring) -> jni::jsize {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetStringLength)(self.env, s)
        }
    }

    /// Copy a UTF-16 region of a Java string into caller-owned storage.
    /// # Safety
    ///
    /// `string` and the requested range must be valid, and `length` must not
    /// exceed the writable length of `buffer`.
    pub unsafe fn get_string_region(
        &self,
        string: jni::jstring,
        start: jni::jsize,
        length: jni::jsize,
        buffer: &mut [jni::jchar],
    ) {
        let get = jni_function!(self, GetStringRegion);
        unsafe { get(self.env, string, start, length, buffer.as_mut_ptr()) }
    }

    /// Copy a Modified UTF-8 region of a Java string into caller-owned bytes.
    /// # Safety
    ///
    /// `string` and the requested UTF-16 range must be valid. `buffer` must be
    /// large enough for the complete Modified UTF-8 encoding; no terminator is
    /// appended.
    pub unsafe fn get_string_utf_region(
        &self,
        string: jni::jstring,
        start: jni::jsize,
        length: jni::jsize,
        buffer: &mut [u8],
    ) {
        let get = jni_function!(self, GetStringUTFRegion);
        unsafe { get(self.env, string, start, length, buffer.as_mut_ptr().cast()) }
    }

    /// Acquire a JVM-critical UTF-16 string pointer.
    /// # Safety
    ///
    /// `string` must be live. The returned guard releases exactly once, and the
    /// caller must obey JNI's no-blocking/no-arbitrary-JNI restrictions while
    /// it is held.
    pub unsafe fn get_string_critical(&self, string: jni::jstring) -> Option<StringCritical<'_>> {
        let raw_length = unsafe { self.get_string_length(string) };
        let length = usize::try_from(raw_length).ok()?;
        let mut is_copy = jni::JNI_FALSE;
        let get = jni_function!(self, GetStringCritical);
        let release = jni_function!(self, ReleaseStringCritical);
        let characters = unsafe { get(self.env, string, &mut is_copy) };
        Some(StringCritical {
            env: self,
            string,
            characters: NonNull::new(characters.cast_mut())?,
            length,
            is_copy: is_copy != jni::JNI_FALSE,
            release,
            active: true,
        })
    }

    // =========================================================================
    // Method IDs
    // =========================================================================

    /// Gets the method ID for an instance method.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_method_id(
        &self,
        cls: jni::jclass,
        name: &str,
        sig: &str,
    ) -> Option<jni::jmethodID> {
        let c_name = mutf8::encode_cstring(name);
        let c_sig = mutf8::encode_cstring(sig);
        // SAFETY: Forwarded from this function's class-handle contract.
        unsafe { self.get_method_id_cstr(cls, &c_name, &c_sig) }
    }

    /// Gets an instance method ID without allocating name/signature strings.
    /// # Safety
    ///
    /// `cls` must be valid and `name` and `sig` must be NUL-terminated Java
    /// Modified UTF-8.
    pub unsafe fn get_method_id_cstr(
        &self,
        cls: jni::jclass,
        name: &CStr,
        sig: &CStr,
    ) -> Option<jni::jmethodID> {
        if mutf8::validate(name.to_bytes()).is_err() || mutf8::validate(sig.to_bytes()).is_err() {
            return None;
        }
        unsafe {
            let vtable = *self.env;
            let mid = ((*vtable).GetMethodID)(self.env, cls, name.as_ptr(), sig.as_ptr());
            if mid.is_null() { None } else { Some(mid) }
        }
    }

    /// Gets the method ID for a static method.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_static_method_id(
        &self,
        cls: jni::jclass,
        name: &str,
        sig: &str,
    ) -> Option<jni::jmethodID> {
        let c_name = mutf8::encode_cstring(name);
        let c_sig = mutf8::encode_cstring(sig);
        // SAFETY: Forwarded from this function's class-handle contract.
        unsafe { self.get_static_method_id_cstr(cls, &c_name, &c_sig) }
    }

    /// Gets a static method ID without allocating name/signature strings.
    /// # Safety
    ///
    /// `cls` must be valid and `name` and `sig` must be NUL-terminated Java
    /// Modified UTF-8.
    pub unsafe fn get_static_method_id_cstr(
        &self,
        cls: jni::jclass,
        name: &CStr,
        sig: &CStr,
    ) -> Option<jni::jmethodID> {
        if mutf8::validate(name.to_bytes()).is_err() || mutf8::validate(sig.to_bytes()).is_err() {
            return None;
        }
        unsafe {
            let vtable = *self.env;
            let mid = ((*vtable).GetStaticMethodID)(self.env, cls, name.as_ptr(), sig.as_ptr());
            if mid.is_null() { None } else { Some(mid) }
        }
    }

    // =========================================================================
    // Field IDs
    // =========================================================================

    /// Gets the field ID for an instance field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_field_id(
        &self,
        cls: jni::jclass,
        name: &str,
        sig: &str,
    ) -> Option<jni::jfieldID> {
        let c_name = mutf8::encode_cstring(name);
        let c_sig = mutf8::encode_cstring(sig);
        // SAFETY: Forwarded from this function's class-handle contract.
        unsafe { self.get_field_id_cstr(cls, &c_name, &c_sig) }
    }

    /// Gets an instance field ID without allocating name/signature strings.
    /// # Safety
    ///
    /// `cls` must be valid and `name` and `sig` must be NUL-terminated Java
    /// Modified UTF-8.
    pub unsafe fn get_field_id_cstr(
        &self,
        cls: jni::jclass,
        name: &CStr,
        sig: &CStr,
    ) -> Option<jni::jfieldID> {
        if mutf8::validate(name.to_bytes()).is_err() || mutf8::validate(sig.to_bytes()).is_err() {
            return None;
        }
        unsafe {
            let vtable = *self.env;
            let fid = ((*vtable).GetFieldID)(self.env, cls, name.as_ptr(), sig.as_ptr());
            if fid.is_null() { None } else { Some(fid) }
        }
    }

    /// Gets the field ID for a static field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_static_field_id(
        &self,
        cls: jni::jclass,
        name: &str,
        sig: &str,
    ) -> Option<jni::jfieldID> {
        let c_name = mutf8::encode_cstring(name);
        let c_sig = mutf8::encode_cstring(sig);
        // SAFETY: Forwarded from this function's class-handle contract.
        unsafe { self.get_static_field_id_cstr(cls, &c_name, &c_sig) }
    }

    /// Gets a static field ID without allocating name/signature strings.
    /// # Safety
    ///
    /// `cls` must be valid and `name` and `sig` must be NUL-terminated Java
    /// Modified UTF-8.
    pub unsafe fn get_static_field_id_cstr(
        &self,
        cls: jni::jclass,
        name: &CStr,
        sig: &CStr,
    ) -> Option<jni::jfieldID> {
        if mutf8::validate(name.to_bytes()).is_err() || mutf8::validate(sig.to_bytes()).is_err() {
            return None;
        }
        unsafe {
            let vtable = *self.env;
            let fid = ((*vtable).GetStaticFieldID)(self.env, cls, name.as_ptr(), sig.as_ptr());
            if fid.is_null() { None } else { Some(fid) }
        }
    }

    // =========================================================================
    // Object Operations
    // =========================================================================

    /// Allocates a new object without calling any constructor.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn alloc_object(&self, cls: jni::jclass) -> Option<jni::jobject> {
        unsafe {
            let vtable = *self.env;
            let obj = ((*vtable).AllocObject)(self.env, cls);
            if obj.is_null() { None } else { Some(obj) }
        }
    }

    /// Creates a new object by calling the specified constructor.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn new_object(
        &self,
        cls: jni::jclass,
        method_id: jni::jmethodID,
        args: &[jni::jvalue],
    ) -> Option<jni::jobject> {
        unsafe {
            let vtable = *self.env;
            let obj = ((*vtable).NewObjectA)(self.env, cls, method_id, args.as_ptr());
            if obj.is_null() { None } else { Some(obj) }
        }
    }

    /// Checks if two references refer to the same object.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn is_same_object(&self, ref1: jni::jobject, ref2: jni::jobject) -> bool {
        unsafe {
            let vtable = *self.env;
            ((*vtable).IsSameObject)(self.env, ref1, ref2) != 0
        }
    }

    // =========================================================================
    // Reference Management
    // =========================================================================

    /// Creates a new global reference to an object.
    ///
    /// Global references must be explicitly deleted with `delete_global_ref`.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn new_global_ref(&self, obj: jni::jobject) -> jni::jobject {
        unsafe {
            let new_global_ref = jni_function!(self, NewGlobalRef);
            new_global_ref(self.env, obj)
        }
    }

    /// Deletes a global reference.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn delete_global_ref(&self, obj: jni::jobject) {
        unsafe {
            let delete_global_ref = jni_function!(self, DeleteGlobalRef);
            delete_global_ref(self.env, obj);
        }
    }

    /// Creates a new local reference to an object.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn new_local_ref(&self, obj: jni::jobject) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).NewLocalRef)(self.env, obj)
        }
    }

    /// Deletes a local reference.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn delete_local_ref(&self, obj: jni::jobject) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).DeleteLocalRef)(self.env, obj);
        }
    }

    /// Creates a new weak global reference.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn new_weak_global_ref(&self, obj: jni::jobject) -> jni::jweak {
        unsafe {
            let new_weak_global_ref = jni_function!(self, NewWeakGlobalRef);
            new_weak_global_ref(self.env, obj)
        }
    }

    /// Deletes a weak global reference.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn delete_weak_global_ref(&self, obj: jni::jweak) {
        unsafe {
            let delete_weak_global_ref = jni_function!(self, DeleteWeakGlobalRef);
            delete_weak_global_ref(self.env, obj);
        }
    }

    /// Ensures capacity for the given number of local references.
    pub fn ensure_local_capacity(&self, capacity: jni::jint) -> Result<(), jni::jint> {
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).EnsureLocalCapacity)(self.env, capacity);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    /// Push a local-reference frame that is popped automatically on drop.
    pub fn push_local_frame(&self, capacity: jni::jint) -> Result<LocalFrame<'_>, jni::jint> {
        unsafe { self.push_local_frame_raw(capacity)? };
        Ok(LocalFrame {
            env: self,
            active: true,
        })
    }

    /// Push a manually managed local-reference frame.
    /// # Safety
    ///
    /// A successful call must be matched by exactly one
    /// [`Self::pop_local_frame_raw`] on the current JNI thread. Prefer
    /// [`Self::push_local_frame`].
    pub unsafe fn push_local_frame_raw(&self, capacity: jni::jint) -> Result<(), jni::jint> {
        let push = jni_function!(self, PushLocalFrame);
        let result = unsafe { push(self.env, capacity) };
        if result == jni::JNI_OK {
            Ok(())
        } else {
            Err(result)
        }
    }

    /// Pop a manually managed local-reference frame.
    /// # Safety
    ///
    /// A matching frame must be active on the current JNI thread. `result`
    /// must be null or a local reference from that frame. Prefer
    /// [`LocalFrame::pop`] or [`LocalFrame::close`].
    pub unsafe fn pop_local_frame_raw(&self, result: jni::jobject) -> jni::jobject {
        let pop = jni_function!(self, PopLocalFrame);
        unsafe { pop(self.env, result) }
    }

    // =========================================================================
    // Array Operations
    // =========================================================================

    /// Gets the length of an array.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_array_length(&self, array: jni::jarray) -> jni::jsize {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetArrayLength)(self.env, array)
        }
    }

    /// Creates a new object array.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn new_object_array(
        &self,
        length: jni::jsize,
        cls: jni::jclass,
        init: jni::jobject,
    ) -> Option<jni::jobjectArray> {
        unsafe {
            let vtable = *self.env;
            let arr = ((*vtable).NewObjectArray)(self.env, length, cls, init);
            if arr.is_null() { None } else { Some(arr) }
        }
    }

    /// Gets an element from an object array.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_object_array_element(
        &self,
        array: jni::jobjectArray,
        index: jni::jsize,
    ) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetObjectArrayElement)(self.env, array, index)
        }
    }

    /// Sets an element in an object array.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn set_object_array_element(
        &self,
        array: jni::jobjectArray,
        index: jni::jsize,
        value: jni::jobject,
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetObjectArrayElement)(self.env, array, index, value);
        }
    }

    jni_new_primitive_array!(new_boolean_array, NewBooleanArray, jni::jbooleanArray);
    jni_new_primitive_array!(new_char_array, NewCharArray, jni::jcharArray);
    jni_new_primitive_array!(new_short_array, NewShortArray, jni::jshortArray);
    jni_new_primitive_array!(new_float_array, NewFloatArray, jni::jfloatArray);
    jni_new_primitive_array!(new_double_array, NewDoubleArray, jni::jdoubleArray);

    jni_primitive_array_elements!(
        get_boolean_array_elements,
        GetBooleanArrayElements,
        ReleaseBooleanArrayElements,
        jni::jbooleanArray,
        jni::jboolean
    );
    jni_primitive_array_elements!(
        get_byte_array_elements,
        GetByteArrayElements,
        ReleaseByteArrayElements,
        jni::jbyteArray,
        jni::jbyte
    );
    jni_primitive_array_elements!(
        get_char_array_elements,
        GetCharArrayElements,
        ReleaseCharArrayElements,
        jni::jcharArray,
        jni::jchar
    );
    jni_primitive_array_elements!(
        get_short_array_elements,
        GetShortArrayElements,
        ReleaseShortArrayElements,
        jni::jshortArray,
        jni::jshort
    );
    jni_primitive_array_elements!(
        get_int_array_elements,
        GetIntArrayElements,
        ReleaseIntArrayElements,
        jni::jintArray,
        jni::jint
    );
    jni_primitive_array_elements!(
        get_long_array_elements,
        GetLongArrayElements,
        ReleaseLongArrayElements,
        jni::jlongArray,
        jni::jlong
    );
    jni_primitive_array_elements!(
        get_float_array_elements,
        GetFloatArrayElements,
        ReleaseFloatArrayElements,
        jni::jfloatArray,
        jni::jfloat
    );
    jni_primitive_array_elements!(
        get_double_array_elements,
        GetDoubleArrayElements,
        ReleaseDoubleArrayElements,
        jni::jdoubleArray,
        jni::jdouble
    );

    jni_primitive_array_region!(
        get_boolean_array_region,
        GetBooleanArrayRegion,
        set_boolean_array_region,
        SetBooleanArrayRegion,
        jni::jbooleanArray,
        jni::jboolean
    );
    jni_primitive_array_region!(
        get_char_array_region,
        GetCharArrayRegion,
        set_char_array_region,
        SetCharArrayRegion,
        jni::jcharArray,
        jni::jchar
    );
    jni_primitive_array_region!(
        get_short_array_region,
        GetShortArrayRegion,
        set_short_array_region,
        SetShortArrayRegion,
        jni::jshortArray,
        jni::jshort
    );
    jni_primitive_array_region!(
        get_float_array_region,
        GetFloatArrayRegion,
        set_float_array_region,
        SetFloatArrayRegion,
        jni::jfloatArray,
        jni::jfloat
    );
    jni_primitive_array_region!(
        get_double_array_region,
        GetDoubleArrayRegion,
        set_double_array_region,
        SetDoubleArrayRegion,
        jni::jdoubleArray,
        jni::jdouble
    );

    /// Creates a new byte array.
    pub fn new_byte_array(&self, length: jni::jsize) -> Option<jni::jbyteArray> {
        unsafe {
            let vtable = *self.env;
            let arr = ((*vtable).NewByteArray)(self.env, length);
            if arr.is_null() { None } else { Some(arr) }
        }
    }

    /// Gets a region of a byte array.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_byte_array_region(
        &self,
        array: jni::jbyteArray,
        start: jni::jsize,
        len: jni::jsize,
        buf: &mut [jni::jbyte],
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetByteArrayRegion)(self.env, array, start, len, buf.as_mut_ptr());
        }
    }

    /// Sets a region of a byte array.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn set_byte_array_region(
        &self,
        array: jni::jbyteArray,
        start: jni::jsize,
        len: jni::jsize,
        buf: &[jni::jbyte],
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetByteArrayRegion)(self.env, array, start, len, buf.as_ptr());
        }
    }

    /// Creates a new int array.
    pub fn new_int_array(&self, length: jni::jsize) -> Option<jni::jintArray> {
        unsafe {
            let vtable = *self.env;
            let arr = ((*vtable).NewIntArray)(self.env, length);
            if arr.is_null() { None } else { Some(arr) }
        }
    }

    /// Gets a region of an int array.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_int_array_region(
        &self,
        array: jni::jintArray,
        start: jni::jsize,
        len: jni::jsize,
        buf: &mut [jni::jint],
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetIntArrayRegion)(self.env, array, start, len, buf.as_mut_ptr());
        }
    }

    /// Sets a region of an int array.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn set_int_array_region(
        &self,
        array: jni::jintArray,
        start: jni::jsize,
        len: jni::jsize,
        buf: &[jni::jint],
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetIntArrayRegion)(self.env, array, start, len, buf.as_ptr());
        }
    }

    /// Creates a new long array.
    pub fn new_long_array(&self, length: jni::jsize) -> Option<jni::jlongArray> {
        unsafe {
            let vtable = *self.env;
            let arr = ((*vtable).NewLongArray)(self.env, length);
            if arr.is_null() { None } else { Some(arr) }
        }
    }

    /// Gets a region of a long array.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_long_array_region(
        &self,
        array: jni::jlongArray,
        start: jni::jsize,
        len: jni::jsize,
        buf: &mut [jni::jlong],
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetLongArrayRegion)(self.env, array, start, len, buf.as_mut_ptr());
        }
    }

    /// Sets a region of a long array.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn set_long_array_region(
        &self,
        array: jni::jlongArray,
        start: jni::jsize,
        len: jni::jsize,
        buf: &[jni::jlong],
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetLongArrayRegion)(self.env, array, start, len, buf.as_ptr());
        }
    }

    /// Acquire a JVM-critical primitive-array pointer.
    /// # Safety
    ///
    /// `array` must be a live primitive array. The returned guard releases
    /// exactly once, and the caller must obey JNI's no-blocking/no-arbitrary-
    /// JNI restrictions while it is held.
    pub unsafe fn get_primitive_array_critical(
        &self,
        array: jni::jarray,
    ) -> Option<PrimitiveArrayCritical<'_>> {
        let raw_length = unsafe { self.get_array_length(array) };
        let element_count = usize::try_from(raw_length).ok()?;
        let mut is_copy = jni::JNI_FALSE;
        let get = jni_function!(self, GetPrimitiveArrayCritical);
        let release = jni_function!(self, ReleasePrimitiveArrayCritical);
        let elements = unsafe { get(self.env, array, &mut is_copy) };
        Some(PrimitiveArrayCritical {
            env: self,
            array,
            elements: NonNull::new(elements)?,
            element_count,
            is_copy: is_copy != jni::JNI_FALSE,
            release,
            active: true,
        })
    }

    // =========================================================================
    // Method Calls
    // =========================================================================

    /// Calls a void instance method.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn call_void_method(
        &self,
        obj: jni::jobject,
        method_id: jni::jmethodID,
        args: &[jni::jvalue],
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallVoidMethodA)(self.env, obj, method_id, args.as_ptr());
        }
    }

    /// Calls an int instance method.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn call_int_method(
        &self,
        obj: jni::jobject,
        method_id: jni::jmethodID,
        args: &[jni::jvalue],
    ) -> jni::jint {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallIntMethodA)(self.env, obj, method_id, args.as_ptr())
        }
    }

    /// Calls a long instance method.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn call_long_method(
        &self,
        obj: jni::jobject,
        method_id: jni::jmethodID,
        args: &[jni::jvalue],
    ) -> jni::jlong {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallLongMethodA)(self.env, obj, method_id, args.as_ptr())
        }
    }

    /// Calls a boolean instance method.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn call_boolean_method(
        &self,
        obj: jni::jobject,
        method_id: jni::jmethodID,
        args: &[jni::jvalue],
    ) -> bool {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallBooleanMethodA)(self.env, obj, method_id, args.as_ptr()) != 0
        }
    }

    /// Calls an object instance method.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn call_object_method(
        &self,
        obj: jni::jobject,
        method_id: jni::jmethodID,
        args: &[jni::jvalue],
    ) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallObjectMethodA)(self.env, obj, method_id, args.as_ptr())
        }
    }

    jni_instance_call_a!(call_byte_method, CallByteMethodA, jni::jbyte);
    jni_instance_call_a!(call_char_method, CallCharMethodA, jni::jchar);
    jni_instance_call_a!(call_short_method, CallShortMethodA, jni::jshort);
    jni_instance_call_a!(call_float_method, CallFloatMethodA, jni::jfloat);
    jni_instance_call_a!(call_double_method, CallDoubleMethodA, jni::jdouble);

    jni_nonvirtual_call_a!(
        call_nonvirtual_object_method,
        CallNonvirtualObjectMethodA,
        jni::jobject
    );
    jni_nonvirtual_bool_call_a!(call_nonvirtual_boolean_method, CallNonvirtualBooleanMethodA);
    jni_nonvirtual_call_a!(
        call_nonvirtual_byte_method,
        CallNonvirtualByteMethodA,
        jni::jbyte
    );
    jni_nonvirtual_call_a!(
        call_nonvirtual_char_method,
        CallNonvirtualCharMethodA,
        jni::jchar
    );
    jni_nonvirtual_call_a!(
        call_nonvirtual_short_method,
        CallNonvirtualShortMethodA,
        jni::jshort
    );
    jni_nonvirtual_call_a!(
        call_nonvirtual_int_method,
        CallNonvirtualIntMethodA,
        jni::jint
    );
    jni_nonvirtual_call_a!(
        call_nonvirtual_long_method,
        CallNonvirtualLongMethodA,
        jni::jlong
    );
    jni_nonvirtual_call_a!(
        call_nonvirtual_float_method,
        CallNonvirtualFloatMethodA,
        jni::jfloat
    );
    jni_nonvirtual_call_a!(
        call_nonvirtual_double_method,
        CallNonvirtualDoubleMethodA,
        jni::jdouble
    );
    jni_nonvirtual_call_a!(call_nonvirtual_void_method, CallNonvirtualVoidMethodA, ());

    /// Calls a void static method.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn call_static_void_method(
        &self,
        cls: jni::jclass,
        method_id: jni::jmethodID,
        args: &[jni::jvalue],
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallStaticVoidMethodA)(self.env, cls, method_id, args.as_ptr());
        }
    }

    /// Calls an int static method.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn call_static_int_method(
        &self,
        cls: jni::jclass,
        method_id: jni::jmethodID,
        args: &[jni::jvalue],
    ) -> jni::jint {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallStaticIntMethodA)(self.env, cls, method_id, args.as_ptr())
        }
    }

    /// Calls an object static method.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn call_static_object_method(
        &self,
        cls: jni::jclass,
        method_id: jni::jmethodID,
        args: &[jni::jvalue],
    ) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallStaticObjectMethodA)(self.env, cls, method_id, args.as_ptr())
        }
    }

    jni_static_bool_call_a!(call_static_boolean_method, CallStaticBooleanMethodA);
    jni_static_call_a!(call_static_byte_method, CallStaticByteMethodA, jni::jbyte);
    jni_static_call_a!(call_static_char_method, CallStaticCharMethodA, jni::jchar);
    jni_static_call_a!(
        call_static_short_method,
        CallStaticShortMethodA,
        jni::jshort
    );
    jni_static_call_a!(call_static_long_method, CallStaticLongMethodA, jni::jlong);
    jni_static_call_a!(
        call_static_float_method,
        CallStaticFloatMethodA,
        jni::jfloat
    );
    jni_static_call_a!(
        call_static_double_method,
        CallStaticDoubleMethodA,
        jni::jdouble
    );

    // =========================================================================
    // Field Access
    // =========================================================================

    /// Gets an object instance field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_object_field(
        &self,
        obj: jni::jobject,
        field_id: jni::jfieldID,
    ) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetObjectField)(self.env, obj, field_id)
        }
    }

    /// Gets an int instance field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_int_field(&self, obj: jni::jobject, field_id: jni::jfieldID) -> jni::jint {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetIntField)(self.env, obj, field_id)
        }
    }

    /// Gets a long instance field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_long_field(&self, obj: jni::jobject, field_id: jni::jfieldID) -> jni::jlong {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetLongField)(self.env, obj, field_id)
        }
    }

    jni_get_bool_field!(get_boolean_field, GetBooleanField, jobject);
    jni_get_field!(get_byte_field, GetByteField, jobject, jni::jbyte);
    jni_get_field!(get_char_field, GetCharField, jobject, jni::jchar);
    jni_get_field!(get_short_field, GetShortField, jobject, jni::jshort);
    jni_get_field!(get_float_field, GetFloatField, jobject, jni::jfloat);
    jni_get_field!(get_double_field, GetDoubleField, jobject, jni::jdouble);

    /// Sets an object instance field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current
    /// thread, and operation. `field_id` must not designate a final field:
    /// mutating final fields through JNI is undefined and is increasingly
    /// rejected by newer Java releases.
    pub unsafe fn set_object_field(
        &self,
        obj: jni::jobject,
        field_id: jni::jfieldID,
        value: jni::jobject,
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetObjectField)(self.env, obj, field_id, value);
        }
    }

    /// Sets an int instance field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current
    /// thread, and operation. `field_id` must not designate a final field:
    /// mutating final fields through JNI is undefined and is increasingly
    /// rejected by newer Java releases.
    pub unsafe fn set_int_field(
        &self,
        obj: jni::jobject,
        field_id: jni::jfieldID,
        value: jni::jint,
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetIntField)(self.env, obj, field_id, value);
        }
    }

    /// Sets a long instance field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current
    /// thread, and operation. `field_id` must not designate a final field:
    /// mutating final fields through JNI is undefined and is increasingly
    /// rejected by newer Java releases.
    pub unsafe fn set_long_field(
        &self,
        obj: jni::jobject,
        field_id: jni::jfieldID,
        value: jni::jlong,
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetLongField)(self.env, obj, field_id, value);
        }
    }

    jni_set_bool_field!(set_boolean_field, SetBooleanField, jobject);
    jni_set_field!(set_byte_field, SetByteField, jobject, jni::jbyte);
    jni_set_field!(set_char_field, SetCharField, jobject, jni::jchar);
    jni_set_field!(set_short_field, SetShortField, jobject, jni::jshort);
    jni_set_field!(set_float_field, SetFloatField, jobject, jni::jfloat);
    jni_set_field!(set_double_field, SetDoubleField, jobject, jni::jdouble);

    /// Gets a static object field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_static_object_field(
        &self,
        cls: jni::jclass,
        field_id: jni::jfieldID,
    ) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetStaticObjectField)(self.env, cls, field_id)
        }
    }

    /// Gets a static int field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn get_static_int_field(
        &self,
        cls: jni::jclass,
        field_id: jni::jfieldID,
    ) -> jni::jint {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetStaticIntField)(self.env, cls, field_id)
        }
    }

    jni_get_bool_field!(get_static_boolean_field, GetStaticBooleanField, jclass);
    jni_get_field!(
        get_static_byte_field,
        GetStaticByteField,
        jclass,
        jni::jbyte
    );
    jni_get_field!(
        get_static_char_field,
        GetStaticCharField,
        jclass,
        jni::jchar
    );
    jni_get_field!(
        get_static_short_field,
        GetStaticShortField,
        jclass,
        jni::jshort
    );
    jni_get_field!(
        get_static_long_field,
        GetStaticLongField,
        jclass,
        jni::jlong
    );
    jni_get_field!(
        get_static_float_field,
        GetStaticFloatField,
        jclass,
        jni::jfloat
    );
    jni_get_field!(
        get_static_double_field,
        GetStaticDoubleField,
        jclass,
        jni::jdouble
    );

    /// Sets a static object field.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current
    /// thread, and operation. `field_id` must not designate a final field:
    /// mutating final fields through JNI is undefined and is increasingly
    /// rejected by newer Java releases.
    pub unsafe fn set_static_object_field(
        &self,
        cls: jni::jclass,
        field_id: jni::jfieldID,
        value: jni::jobject,
    ) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetStaticObjectField)(self.env, cls, field_id, value);
        }
    }

    jni_set_bool_field!(set_static_boolean_field, SetStaticBooleanField, jclass);
    jni_set_field!(
        set_static_byte_field,
        SetStaticByteField,
        jclass,
        jni::jbyte
    );
    jni_set_field!(
        set_static_char_field,
        SetStaticCharField,
        jclass,
        jni::jchar
    );
    jni_set_field!(
        set_static_short_field,
        SetStaticShortField,
        jclass,
        jni::jshort
    );
    jni_set_field!(set_static_int_field, SetStaticIntField, jclass, jni::jint);
    jni_set_field!(
        set_static_long_field,
        SetStaticLongField,
        jclass,
        jni::jlong
    );
    jni_set_field!(
        set_static_float_field,
        SetStaticFloatField,
        jclass,
        jni::jfloat
    );
    jni_set_field!(
        set_static_double_field,
        SetStaticDoubleField,
        jclass,
        jni::jdouble
    );

    // =========================================================================
    // Monitors
    // =========================================================================

    /// Enter an object's monitor and return a guard that exits it on drop.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn monitor_enter(
        &self,
        obj: jni::jobject,
    ) -> Result<JavaMonitorGuard<'_>, jni::jint> {
        unsafe { self.monitor_enter_raw(obj)? };
        Ok(JavaMonitorGuard {
            env: self,
            object: obj,
            active: true,
        })
    }

    /// Enter an object's monitor without creating an owning guard.
    /// # Safety
    ///
    /// The object must be valid for this environment and the successful entry
    /// must be matched by exactly one [`Self::monitor_exit_raw`] on this
    /// thread. Prefer [`Self::monitor_enter`].
    pub unsafe fn monitor_enter_raw(&self, obj: jni::jobject) -> Result<(), jni::jint> {
        let enter = jni_function!(self, MonitorEnter);
        let result = unsafe { enter(self.env, obj) };
        if result == jni::JNI_OK {
            Ok(())
        } else {
            Err(result)
        }
    }

    /// Exit a manually managed object monitor.
    /// # Safety
    ///
    /// The current thread must own the monitor through a matching successful
    /// entry. Prefer [`JavaMonitorGuard::exit`].
    pub unsafe fn monitor_exit_raw(&self, obj: jni::jobject) -> Result<(), jni::jint> {
        let exit = jni_function!(self, MonitorExit);
        let result = unsafe { exit(self.env, obj) };
        if result == jni::JNI_OK {
            Ok(())
        } else {
            Err(result)
        }
    }

    /// Wrap native memory in a Java direct `ByteBuffer`.
    /// # Safety
    ///
    /// `address..address+capacity` must remain valid and suitably aligned for
    /// every Java access until the returned buffer becomes unreachable.
    pub unsafe fn new_direct_byte_buffer(
        &self,
        address: *mut std::ffi::c_void,
        capacity: jni::jlong,
    ) -> Option<jni::jobject> {
        let create = jni_function!(self, NewDirectByteBuffer);
        let buffer = unsafe { create(self.env, address, capacity) };
        (!buffer.is_null()).then_some(buffer)
    }

    /// Return the native address backing a direct `ByteBuffer`.
    /// # Safety
    ///
    /// `buffer` must be a live direct-buffer reference from this VM and
    /// current JNI thread.
    pub unsafe fn get_direct_buffer_address(&self, buffer: jni::jobject) -> *mut std::ffi::c_void {
        let get = jni_function!(self, GetDirectBufferAddress);
        unsafe { get(self.env, buffer) }
    }

    /// Return the capacity of a direct `ByteBuffer`, or the JVM's negative
    /// sentinel when the object is not a supported direct buffer.
    /// # Safety
    ///
    /// `buffer` must be a live reference from this VM and current JNI thread.
    pub unsafe fn get_direct_buffer_capacity(&self, buffer: jni::jobject) -> jni::jlong {
        let get = jni_function!(self, GetDirectBufferCapacity);
        unsafe { get(self.env, buffer) }
    }

    /// Classify a JNI reference as local, global, weak-global, or invalid.
    /// # Safety
    ///
    /// `object` must be null or a reference value that may be inspected by
    /// this VM and current JNI thread.
    pub unsafe fn get_object_ref_type(&self, object: jni::jobject) -> jni::jobjectRefType {
        let get = jni_function!(self, GetObjectRefType);
        unsafe { get(self.env, object) }
    }

    // =========================================================================
    // Native Method Registration
    // =========================================================================

    /// Registers native methods for a class.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn register_natives(
        &self,
        cls: jni::jclass,
        methods: &[jni::JNINativeMethod],
    ) -> Result<(), jni::jint> {
        let method_count = jni::jint::try_from(methods.len()).map_err(|_| jni::JNI_EINVAL)?;
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).RegisterNatives)(self.env, cls, methods.as_ptr(), method_count);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    /// Unregisters all native methods for a class.
    /// # Safety
    ///
    /// Every JNI handle argument must be valid for this environment, current thread, and operation for the duration required by the JNI specification.
    pub unsafe fn unregister_natives(&self, cls: jni::jclass) -> Result<(), jni::jint> {
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).UnregisterNatives)(self.env, cls);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }
}

// =========================================================================
// Reference Guards (RAII wrappers)
// =========================================================================

/// A guard that automatically deletes a local reference when dropped.
///
/// # Example
///
/// ```rust,no_run
/// use jvmti_bindings::prelude::*;
///
/// fn inspect_string_class(env: &JniEnv) {
///     let Some(raw_class) = env.find_class("java/lang/String") else { return };
///     let class = unsafe { LocalRef::from_raw(env, raw_class) };
///     // class is automatically deleted when it goes out of scope
/// }
/// ```
pub struct LocalRef<'a> {
    env: &'a JniEnv,
    obj: jni::jobject,
}

impl<'a> LocalRef<'a> {
    /// Takes ownership of an existing JNI local reference.
    ///
    /// # Safety
    ///
    /// `obj` must be null or a live local reference owned by the current JNI
    /// frame and thread. No other owner may delete it after this call.
    pub unsafe fn from_raw(env: &'a JniEnv, obj: jni::jobject) -> Self {
        LocalRef { env, obj }
    }

    /// Returns the underlying jobject.
    pub fn get(&self) -> jni::jobject {
        self.obj
    }

    /// Releases the reference without deleting it.
    pub fn into_inner(self) -> jni::jobject {
        let obj = self.obj;
        std::mem::forget(self);
        obj
    }
}

impl Drop for LocalRef<'_> {
    fn drop(&mut self) {
        if !self.obj.is_null() {
            // LocalRef's unsafe constructor established this ownership.
            unsafe { self.env.delete_local_ref(self.obj) };
        }
    }
}

unsafe fn delete_vm_reference(
    vm: *mut jni::JavaVM,
    obj: jni::jobject,
    weak: bool,
) -> Result<(), jni::jint> {
    if obj.is_null() {
        return Ok(());
    }
    if vm.is_null() {
        return Err(jni::JNI_EINVAL);
    }

    unsafe {
        let table = vm.read();
        if table.is_null() {
            return Err(jni::JNI_EINVAL);
        }
        let get_env_fn = (&raw const (*table).GetEnv).read();
        let attach_fn = (&raw const (*table).AttachCurrentThread).read();
        let detach_fn = (&raw const (*table).DetachCurrentThread).read();

        let mut env_ptr: *mut std::ffi::c_void = ptr::null_mut();
        let res = get_env_fn(vm, &mut env_ptr, jni::JNI_VERSION_1_6);

        if res == jni::JNI_OK && !env_ptr.is_null() {
            let env = JniEnv::from_raw(env_ptr as *mut jni::JNIEnv);
            if weak {
                env.delete_weak_global_ref(obj);
            } else {
                env.delete_global_ref(obj);
            }
            return Ok(());
        }

        if res == jni::JNI_EDETACHED {
            let mut attach_env: *mut std::ffi::c_void = ptr::null_mut();
            let attach_result = attach_fn(vm, &mut attach_env, ptr::null_mut());
            if attach_result == jni::JNI_OK && !attach_env.is_null() {
                let env = JniEnv::from_raw(attach_env as *mut jni::JNIEnv);
                if weak {
                    env.delete_weak_global_ref(obj);
                } else {
                    env.delete_global_ref(obj);
                }
                // Reference deletion succeeded. Detach failure is unrelated to
                // reference ownership and must not trigger a second deletion.
                let _ = detach_fn(vm);
                return Ok(());
            }
            return Err(if attach_result == jni::JNI_OK {
                jni::JNI_ERR
            } else {
                attach_result
            });
        }
        Err(res)
    }
}

/// A guard that automatically deletes a global reference when dropped.
///
/// # Example
///
/// ```rust,no_run
/// use jvmti_bindings::prelude::*;
///
/// fn promote_string_class(env: &JniEnv) -> Result<(), jni::jint> {
///     let Some(raw_class) = env.find_class("java/lang/String") else { return Ok(()) };
///     let local_class = unsafe { LocalRef::from_raw(env, raw_class) };
///     let global_class = unsafe { GlobalRef::new(env, local_class.get()) }?;
///     // global_class can be used across JNI calls and is deleted on drop.
///     Ok(())
/// }
/// ```
pub struct GlobalRef {
    vm: *mut jni::JavaVM,
    obj: jni::jobject,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl GlobalRef {
    /// Creates a new GlobalRef by creating a global reference from a local reference.
    ///
    /// # Safety
    ///
    /// `local_obj` must be null or a live reference belonging to the same VM as
    /// `env` and valid on the current JNI thread. The VM must remain alive until
    /// this guard is dropped, and no other owner may delete the global reference
    /// created by this call.
    pub unsafe fn new(env: &JniEnv, local_obj: jni::jobject) -> Result<Self, jni::jint> {
        // Resolve the VM before creating the reference. If GetJavaVM fails,
        // there is no newly-created global reference that Drop cannot release.
        let vm = env.get_java_vm()?;
        // SAFETY: the constructor's contract establishes the raw reference
        // invariants required by NewGlobalRef.
        let global = unsafe { env.new_global_ref(local_obj) };
        if !local_obj.is_null() && global.is_null() {
            return Err(jni::JNI_ENOMEM);
        }
        Ok(GlobalRef {
            vm,
            obj: global,
            _not_send_sync: PhantomData,
        })
    }

    /// Returns the underlying global reference.
    pub fn get(&self) -> jni::jobject {
        self.obj
    }

    /// Deletes the reference and reports cleanup failures.
    ///
    /// Prefer this at explicit lifecycle boundaries when a best-effort `Drop`
    /// is not sufficient. On failure, `Drop` makes one final cleanup attempt.
    pub fn close(mut self) -> Result<(), jni::jint> {
        // SAFETY: construction records the VM that owns this reference.
        let result = unsafe { delete_vm_reference(self.vm, self.obj, false) };
        if result.is_ok() {
            self.obj = ptr::null_mut();
        }
        result
    }
}

impl Drop for GlobalRef {
    fn drop(&mut self) {
        // SAFETY: construction records the VM that owns this global reference.
        let _ = unsafe { delete_vm_reference(self.vm, self.obj, false) };
    }
}

/// An owning JNI weak global reference.
///
/// The reference is deleted automatically on drop. The referenced Java object
/// may still be reclaimed; use `JniEnv::new_local_ref` and test the result for
/// null before accessing it.
pub struct WeakGlobalRef {
    vm: *mut jni::JavaVM,
    obj: jni::jweak,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl WeakGlobalRef {
    /// Creates an owning weak global reference.
    ///
    /// # Safety
    ///
    /// `local_obj` must be null or a live reference belonging to the same VM as
    /// `env` and valid on the current JNI thread. The VM must remain alive until
    /// this guard is dropped.
    pub unsafe fn new(env: &JniEnv, local_obj: jni::jobject) -> Result<Self, jni::jint> {
        let vm = env.get_java_vm()?;
        // SAFETY: forwarded from this constructor's reference contract.
        let weak = unsafe { env.new_weak_global_ref(local_obj) };
        if !local_obj.is_null() && weak.is_null() {
            return Err(jni::JNI_ENOMEM);
        }
        Ok(Self {
            vm,
            obj: weak,
            _not_send_sync: PhantomData,
        })
    }

    /// Returns the underlying weak global reference.
    pub fn get(&self) -> jni::jweak {
        self.obj
    }

    /// Deletes the weak reference and reports cleanup failures.
    pub fn close(mut self) -> Result<(), jni::jint> {
        // SAFETY: construction records the VM that owns this reference.
        let result = unsafe { delete_vm_reference(self.vm, self.obj, true) };
        if result.is_ok() {
            self.obj = ptr::null_mut();
        }
        result
    }
}

impl Drop for WeakGlobalRef {
    fn drop(&mut self) {
        // SAFETY: construction records the VM that owns this weak reference.
        let _ = unsafe { delete_vm_reference(self.vm, self.obj, true) };
    }
}

// GlobalRef and WeakGlobalRef are not Send or Sync because their lifecycle
// operations require a valid thread-local JNIEnv.
